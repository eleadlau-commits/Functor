use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::check::{self, Failure, Mode, Opts};
use crate::gen::Tables;
use crate::{page_source, shrink};

pub const KNOWN: &str = include_str!("../known.txt");

pub static CURRENT: AtomicU64 = AtomicU64::new(u64::MAX);

#[derive(Clone, Debug)]
pub struct RunOpts {
    pub seed: u64,
    pub from: u64,
    pub pages: u64,
    pub mode: Option<Mode>,
    pub jobs: usize,
    pub out: PathBuf,
    pub hang_ms: u64,
    pub shrink: usize,
    pub depth: usize,
    pub pdf_every: u64,
}

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts {
            seed: 1,
            from: 0,
            pages: 1000,
            mode: None,
            jobs: std::thread::available_parallelism().map_or(2, |n| n.get()),
            out: PathBuf::from("target/stress"),
            hang_ms: 20_000,
            shrink: 12,
            depth: 3,
            pdf_every: 4,
        }
    }
}

pub fn page_mode(mode: Option<Mode>, index: u64) -> Mode {
    match mode {
        Some(m) => m,
        None if index % 4 == 3 => Mode::Garbage,
        None => Mode::Valid,
    }
}

pub fn known() -> HashSet<String> {
    known_limits().into_keys().collect()
}

pub fn known_limits() -> std::collections::HashMap<String, Option<u64>> {
    KNOWN
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| match l.rsplit_once(" <= ") {
            Some((sig, n)) if n.trim().parse::<u64>().is_ok() => (sig.trim().to_string(), n.trim().parse().ok()),
            _ => (l.to_string(), None),
        })
        .collect()
}

pub fn over_limit(limit: Option<u64>, count: usize, pages: u64) -> bool {
    limit.is_some_and(|n| count as u64 > (n * pages).div_ceil(10_000).max(1))
}

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let cur = CURRENT.load(Ordering::SeqCst);
        let line = format!("P {cur}\t{loc}\t{}\n", msg.replace(['\n', '\t'], " "));
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(line.as_bytes());
        let _ = out.flush();
        std::process::abort();
    }));
}

pub fn child(seed: u64, from: u64, to: u64, mode: Option<Mode>, depth: usize, pdf_every: u64) {
    install_panic_hook();
    let t = Tables::new();
    let mut out = std::io::stdout().lock();
    for i in from..to {
        CURRENT.store(i, Ordering::SeqCst);
        let m = page_mode(mode, i);
        let src = page_source(&t, seed, i, m, depth);
        let _ = writeln!(out, "B {i}");
        let _ = out.flush();
        let opts = Opts {
            pdf: pdf_every > 0 && i % pdf_every == 0,
            ..Opts::default()
        };
        let start = Instant::now();
        for f in small_stack(move || check::check(&src, m, opts)) {
            let _ = writeln!(out, "F {i}\t{}\t{}\t{}", f.kind, f.sig, clean(&f.detail));
        }
        let _ = writeln!(out, "D {i} {}", start.elapsed().as_micros());
        let _ = out.flush();
    }
}

pub fn probe_child(mode: Mode) {
    install_panic_hook();
    CURRENT.store(0, Ordering::SeqCst);
    let mut src = String::new();
    let _ = std::io::stdin().read_to_string(&mut src);
    let mut out = std::io::stdout().lock();
    for f in small_stack(move || check::check(&src, mode, Opts::default())) {
        let _ = writeln!(out, "F 0\t{}\t{}\t{}", f.kind, f.sig, clean(&f.detail));
    }
    let _ = writeln!(out, "D 0 0");
    let _ = out.flush();
}

pub const STACK: usize = 1 << 20;

fn small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(f)
        .expect("thread")
        .join()
        .unwrap_or_else(|_| std::process::abort())
}

fn clean(s: &str) -> String {
    let s = s.replace(['\n', '\t', '\r'], " ");
    if s.len() > 300 {
        let mut e = 300;
        while !s.is_char_boundary(e) {
            e -= 1;
        }
        format!("{}…", &s[..e])
    } else {
        s
    }
}

#[derive(Clone, Debug)]
pub struct Record {
    pub index: u64,
    pub mode: Mode,
    pub f: Failure,
}

enum Ev {
    Line(String),
    Eof,
}

fn spawn(
    exe: &Path,
    args: &[String],
    stdin: bool,
) -> std::io::Result<(Child, mpsc::Receiver<Ev>, std::thread::JoinHandle<String>)> {
    let mut c = Command::new(exe)
        .args(args)
        .stdin(if stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let so = c.stdout.take().expect("stdout");
    let mut se = c.stderr.take().expect("stderr");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for l in BufReader::new(so).lines() {
            match l {
                Ok(l) => {
                    if tx.send(Ev::Line(l)).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(Ev::Eof);
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = se.read_to_string(&mut s);
        s
    });
    Ok((c, rx, err))
}

fn crash_failure(
    status: Option<std::process::ExitStatus>,
    panic: Option<(String, String)>,
    stderr: &str,
) -> Failure {
    if let Some((loc, msg)) = panic {
        let loc = loc.rsplit("src/").next().unwrap_or(&loc).to_string();
        return Failure {
            kind: "panic",
            sig: format!("panic: {}", strip_line(&loc)),
            detail: format!("{loc}: {msg}"),
        };
    }
    if stderr.contains("overflowed its stack") {
        return Failure {
            kind: "crash",
            sig: "crash: stack overflow".into(),
            detail: "stack overflow".into(),
        };
    }
    let code = status
        .map(|s| format!("{s}"))
        .unwrap_or_else(|| "killed".into());
    let tail: String = stderr
        .lines()
        .last()
        .unwrap_or("")
        .chars()
        .take(200)
        .collect();
    Failure {
        kind: "crash",
        sig: format!("crash: {}", check::normalize(&code)),
        detail: format!("{code} {tail}"),
    }
}

fn strip_line(loc: &str) -> String {
    loc.split(':').next().unwrap_or(loc).to_string()
}

fn parse_f(rest: &str) -> Option<(u64, Failure)> {
    let mut p = rest.splitn(4, '\t');
    let i = p.next()?.parse().ok()?;
    let kind: &'static str = match p.next()? {
        "slow" => "slow",
        "refused-valid" => "refused-valid",
        "bad-position" => "bad-position",
        "empty-message" => "empty-message",
        "bad-number" => "bad-number",
        "empty-math" => "empty-math",
        "nondeterministic" => "nondeterministic",
        "pdf" => "pdf",
        "collision" => "collision",
        _ => "other",
    };
    let sig = p.next()?.to_string();
    let detail = p.next().unwrap_or("").to_string();
    Some((i, Failure { kind, sig, detail }))
}

fn supervise(
    exe: &Path,
    o: &RunOpts,
    from: u64,
    to: u64,
    records: &mut Vec<Record>,
    times: &mut Vec<(u64, u128)>,
) {
    let mut start = from;
    while start < to {
        let mode = o.mode.map_or("both", Mode::as_str).to_string();
        let args = vec![
            "child".into(),
            "--seed".into(),
            o.seed.to_string(),
            "--from".into(),
            start.to_string(),
            "--to".into(),
            to.to_string(),
            "--mode".into(),
            mode,
            "--depth".into(),
            o.depth.to_string(),
            "--pdf-every".into(),
            o.pdf_every.to_string(),
        ];
        let Ok((mut c, rx, err)) = spawn(exe, &args, false) else {
            return;
        };
        let mut current = start;
        let mut panic = None;
        let mut hung = false;
        let mut done_to = start;
        loop {
            match rx.recv_timeout(Duration::from_millis(o.hang_ms)) {
                Ok(Ev::Line(l)) => {
                    if let Some(r) = l.strip_prefix("B ") {
                        current = r.trim().parse().unwrap_or(current);
                    } else if let Some(r) = l.strip_prefix("F ") {
                        if let Some((i, f)) = parse_f(r) {
                            records.push(Record {
                                index: i,
                                mode: page_mode(o.mode, i),
                                f,
                            });
                        }
                    } else if let Some(r) = l.strip_prefix("D ") {
                        let mut p = r.split_whitespace();
                        let i: u64 = p.next().and_then(|x| x.parse().ok()).unwrap_or(current);
                        let us: u128 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                        times.push((i, us));
                        done_to = i + 1;
                    } else if let Some(r) = l.strip_prefix("P ") {
                        let mut p = r.splitn(3, '\t');
                        let _ = p.next();
                        panic = Some((
                            p.next().unwrap_or("").to_string(),
                            p.next().unwrap_or("").to_string(),
                        ));
                    }
                }
                Ok(Ev::Eof) => break,
                Err(_) => {
                    hung = true;
                    let _ = c.kill();
                    break;
                }
            }
        }
        let status = c.wait().ok();
        let stderr = err.join().unwrap_or_default();
        if done_to >= to {
            break;
        }
        let f = if hung {
            Failure {
                kind: "hang",
                sig: "hang".into(),
                detail: format!("no answer in {} ms", o.hang_ms),
            }
        } else {
            crash_failure(status, panic, &stderr)
        };
        records.push(Record {
            index: current,
            mode: page_mode(o.mode, current),
            f,
        });
        start = current + 1;
    }
}

pub fn probe(exe: &Path, src: &str, mode: Mode, hang_ms: u64) -> Vec<Failure> {
    let args = vec!["probe".to_string(), "--mode".into(), mode.as_str().into()];
    let Ok((mut c, rx, err)) = spawn(exe, &args, true) else {
        return vec![];
    };
    if let Some(mut si) = c.stdin.take() {
        let s = src.to_string();
        std::thread::spawn(move || {
            let _ = si.write_all(s.as_bytes());
        });
    }
    let mut out = Vec::new();
    let mut panic = None;
    let mut done = false;
    let mut hung = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(hang_ms)) {
            Ok(Ev::Line(l)) => {
                if let Some(r) = l.strip_prefix("F ") {
                    if let Some((_, f)) = parse_f(r) {
                        out.push(f);
                    }
                } else if l.starts_with("D ") {
                    done = true;
                } else if let Some(r) = l.strip_prefix("P ") {
                    let mut p = r.splitn(3, '\t');
                    let _ = p.next();
                    panic = Some((
                        p.next().unwrap_or("").to_string(),
                        p.next().unwrap_or("").to_string(),
                    ));
                }
            }
            Ok(Ev::Eof) => break,
            Err(_) => {
                hung = true;
                let _ = c.kill();
                break;
            }
        }
    }
    let status = c.wait().ok();
    let stderr = err.join().unwrap_or_default();
    if hung {
        out.push(Failure {
            kind: "hang",
            sig: "hang".into(),
            detail: String::new(),
        });
    } else if !done {
        out.push(crash_failure(status, panic, &stderr));
    }
    out
}

pub struct Summary {
    pub records: Vec<Record>,
    pub pages: u64,
    pub secs: f64,
    pub slowest: Option<(u64, u128)>,
    pub new_sigs: Vec<String>,
}

pub fn run(o: &RunOpts) -> Summary {
    let exe = std::env::current_exe().expect("exe");
    let start = Instant::now();
    let jobs = o.jobs.max(1) as u64;
    let per = o.pages.div_ceil(jobs);
    let mut handles = Vec::new();
    for j in 0..jobs {
        let a = o.from + j * per;
        let b = (a + per).min(o.from + o.pages);
        if a >= b {
            continue;
        }
        let (exe, o) = (exe.clone(), o.clone());
        handles.push(std::thread::spawn(move || {
            let mut r = Vec::new();
            let mut t = Vec::new();
            supervise(&exe, &o, a, b, &mut r, &mut t);
            (r, t)
        }));
    }
    let mut records = Vec::new();
    let mut times = Vec::new();
    for h in handles {
        if let Ok((r, t)) = h.join() {
            records.extend(r);
            times.extend(t);
        }
    }
    records.sort_by_key(|r| r.index);
    let secs = start.elapsed().as_secs_f64();
    let slowest = times.iter().max_by_key(|t| t.1).copied();
    let limits = known_limits();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in &records {
        *counts.entry(r.f.sig.clone()).or_default() += 1;
    }
    let mut new_sigs: Vec<String> = counts
        .iter()
        .filter(|(s, c)| match limits.get(*s) {
            None => true,
            Some(l) => over_limit(*l, **c, o.pages),
        })
        .map(|(s, _)| s.clone())
        .collect();
    new_sigs.sort();
    new_sigs.dedup();
    Summary {
        records,
        pages: o.pages,
        secs,
        slowest,
        new_sigs,
    }
}

pub fn report(o: &RunOpts, s: &Summary) -> String {
    let exe = std::env::current_exe().expect("exe");
    let t = Tables::new();
    let known = known();
    let _ = std::fs::create_dir_all(o.out.join("failures"));
    let mut by: BTreeMap<String, Vec<&Record>> = BTreeMap::new();
    for r in &s.records {
        by.entry(r.f.sig.clone()).or_default().push(r);
    }
    let mut md = String::new();
    md.push_str(&format!(
        "# Functor stress run\n\nSeed {}, pages {}..{}, depth {}, {} jobs. {} pages in {:.1} s ({:.0} pages/s).\n",
        o.seed,
        o.from,
        o.from + o.pages,
        o.depth,
        o.jobs,
        s.pages,
        s.secs,
        s.pages as f64 / s.secs.max(1e-9)
    ));
    if let Some((i, us)) = s.slowest {
        md.push_str(&format!(
            "Slowest page: {i} ({:.1} ms for all checks).\n",
            us as f64 / 1000.0
        ));
    }
    md.push_str(&format!(
        "\n{} failures, {} kinds, {} new.\n\n",
        s.records.len(),
        by.len(),
        s.new_sigs.len()
    ));
    if by.is_empty() {
        md.push_str("No failures.\n");
        return md;
    }
    md.push_str("| Kind | Pages | First page | Known | Smallest case |\n|:--|--:|--:|:--|:--|\n");
    let mut shrunk = 0;
    let mut order: Vec<(&String, &Vec<&Record>)> = by.iter().collect();
    order.sort_by_key(|(sig, v)| (known.contains(*sig), std::cmp::Reverse(v.len())));
    let mut details = String::new();
    for (sig, recs) in order {
        let first = recs[0];
        let is_known = known.contains(sig);
        let mut case = String::from("-");
        if shrunk < o.shrink {
            shrunk += 1;
            let src = page_source(&t, o.seed, first.index, first.mode, o.depth);
            let mode = first.mode;
            let keep_valid = sig.starts_with("refused-valid");
            let want_detail = strip_line_no(&first.f.detail);
            let mut keep = |c: &str| {
                if mode == Mode::Valid
                    && (!balanced(c)
                        || EMPTY_ARGS.iter().any(|p| c.contains(p) && !src.contains(p)))
                {
                    return false;
                }
                let fs = probe(&exe, c, mode, o.hang_ms);
                let hit = if keep_valid {
                    fs.iter().any(|f| strip_line_no(&f.detail) == want_detail)
                } else {
                    fs.iter().any(|f| &f.sig == sig)
                };
                hit && (keep_valid
                    || mode == Mode::Garbage
                    || !fs.iter().any(|f| f.kind == "refused-valid"))
            };
            let start = if sig.starts_with("collision") {
                formula_case(&src, sig).filter(|m| keep(m))
            } else {
                None
            };
            let small = match start {
                Some(m) => shrink::shrink(&m, &mut keep, 400),
                None if keep(&src) => shrink::shrink(&src, &mut keep, 400),
                None => src.clone(),
            };
            let slug = slug(sig);
            let _ = std::fs::write(o.out.join("failures").join(format!("{slug}.fn")), &small);
            let _ = std::fs::write(o.out.join("failures").join(format!("{slug}.page.fn")), &src);
            case = format!("`failures/{slug}.fn`");
            details.push_str(&format!(
                "## {sig}\n\nMode {}, page {}. {}\n\n```fn\n{}\n```\n\n",
                mode.as_str(),
                first.index,
                first.f.detail,
                small.trim_end()
            ));
        }
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            sig.replace('|', "\\|"),
            recs.len(),
            first.index,
            if s.new_sigs.contains(sig) && is_known {
                "**over limit**"
            } else if is_known {
                "yes"
            } else {
                "**new**"
            },
            case
        ));
    }
    md.push('\n');
    md.push_str(&details);
    md
}

fn formula_case(page: &str, sig: &str) -> Option<String> {
    let found = check::collisions(page);
    let f = found.iter().find(|c| {
        sig.ends_with(&format!(
            "{} ({})",
            c.what,
            if c.display { "display" } else { "inline" }
        ))
    })?;
    let decls: String = page
        .lines()
        .filter(|l| l.starts_with("sym ") || l.starts_with("object ") || l.starts_with("set "))
        .map(|l| format!("{l}\n"))
        .collect();
    let body = if f.display {
        format!("$$\n{}\n$$\n", f.src.trim())
    } else {
        format!("${}$\n", f.src.trim())
    };
    Some(format!("{decls}\n{body}"))
}

fn strip_line_no(d: &str) -> String {
    match d.split_once(": ") {
        Some((head, rest)) if head.starts_with("line ") => rest.to_string(),
        _ => d.to_string(),
    }
}

const EMPTY_ARGS: &[&str] = &[
    "(,", ",)", ", )", "()", ",,", ", ,", "(;", ";)", "; )", ";;", "; ;", "$$$$", "_)", "^)", "_ ",
    "^ ", "_$", "^$",
];

pub fn balanced(s: &str) -> bool {
    let mut stack = Vec::new();
    let mut quote = false;
    let mut dollars = 0;
    for c in s.chars() {
        if c == '"' {
            quote = !quote;
            continue;
        }
        if quote {
            continue;
        }
        match c {
            '$' => dollars += 1,
            '(' | '[' | '{' => stack.push(c),
            ')' | ']' | '}' => {
                if stack.pop().is_none() {
                    return false;
                }
            }
            _ => {}
        }
    }
    stack.is_empty() && !quote && dollars % 2 == 0
}

fn slug(sig: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in sig.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    let head: String = sig
        .chars()
        .take(40)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    format!("{head}-{:06x}", h & 0xFF_FFFF)
}
