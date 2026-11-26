use std::collections::{HashMap, HashSet};

use crate::diag::Diagnostics;
use crate::math::MathEnv;
use crate::trace::{look, rough_look, Form, Trace};

const VECTOR_LOOKS: [&str; 2] = ["arrow", "bold"];

pub fn lint(traces: &[&Trace], env: &MathEnv, diags: &mut Diagnostics) {
    if env.opt("lint", "notation") == "off" {
        return;
    }
    let mut done: HashSet<(usize, usize)> = HashSet::new();
    vector_rule(traces, env, diags, &mut done);
    vector_letters(traces, diags, &mut done);
    same_look(traces, diags, &mut done);
}

fn vector_op(t: &Trace) -> Option<&str> {
    let mut it = t.hand.iter().map(String::as_str).filter(|o| VECTOR_LOOKS.contains(o));
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

fn vector_rule(traces: &[&Trace], env: &MathEnv, diags: &mut Diagnostics, done: &mut HashSet<(usize, usize)>) {
    let rule = env.vec_style.as_str();
    for t in traces.iter().filter(|t| t.key.starts_with("vec ") && matches!(t.form, Form::Object | Form::Kind)) {
        let Some(h) = vector_op(t) else { continue };
        let s = &t.subject;
        if h == rule {
            diags.warning(t.line, t.col, format!("`.{h}` is not necessary here. `{s}` is a vector, and the style rule draws vectors with `.{rule}`."), Some(format!("Remove `.{h}`.")));
        } else {
            diags.warning(
                t.line,
                t.col,
                format!("`{s}` is a vector, and the style rule draws vectors with `.{rule}`. Here `.{h}` is written by hand, so this vector has a second look."),
                Some(format!("Remove `.{h}`, or change the rule for all vectors: `set vec(style: {h})`.")),
            );
        }
        done.insert((t.line, t.col));
    }
}

fn vector_letters(traces: &[&Trace], diags: &mut Diagnostics, done: &mut HashSet<(usize, usize)>) {
    let mut first: HashMap<&str, (&str, usize)> = HashMap::new();
    for t in traces.iter().filter(|t| t.form == Form::Name) {
        let Some(h) = vector_op(t) else { continue };
        if done.contains(&(t.line, t.col)) {
            continue;
        }
        let s = t.subject.as_str();
        match first.get(s) {
            None => {
                first.insert(s, (h, t.line));
            }
            Some(&(f, line)) if f != h => {
                diags.warning(
                    t.line,
                    t.col,
                    format!("`{s}` has two vector looks: `.{h}` here and `.{f}` on line {line}."),
                    Some(format!("Write `object vec: {s}` one time. Then select one look for all vectors with `set vec(style: ...)`.")),
                );
                done.insert((t.line, t.col));
            }
            _ => {}
        }
    }
}

fn fence_body(t: &Trace) -> String {
    let spec = t.recipe.strip_prefix("fences ").and_then(|r| r.strip_suffix(" around the group")).unwrap_or("");
    let body = t.source.strip_suffix(spec).unwrap_or(&t.source).trim();
    body.strip_prefix('(').and_then(|b| b.strip_suffix(')')).unwrap_or(body).trim().to_string()
}

fn suggestion(f: &Trace, t: &Trace) -> String {
    match f.form {
        Form::Kind if f.key.starts_with("vec ") => format!("Write `vec({})`.", f.subject),
        Form::Kind if t.key == "fence" => format!("Write `{}({})`.", f.subject, fence_body(t)),
        Form::Kind => format!("Write it as `{}`.", f.source),
        _ => format!("Write `{}`.", f.subject),
    }
}

fn same_look(traces: &[&Trace], diags: &mut Diagnostics, done: &mut HashSet<(usize, usize)>) {
    let mut rough: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, t) in traces.iter().enumerate() {
        if let Some(l) = rough_look(t) {
            rough.entry(l).or_default().push(i);
        }
    }
    let mut looks: Vec<Option<u64>> = vec![None; traces.len()];
    for idx in rough.values().filter(|v| v.len() > 1 && v.iter().any(|&i| traces[i].form.has_meaning())) {
        for &i in idx {
            looks[i] = look(&traces[i]);
        }
    }
    let mut groups: HashMap<u64, Vec<&Trace>> = HashMap::new();
    for (t, l) in traces.iter().zip(&looks) {
        if let Some(l) = l {
            groups.entry(*l).or_default().push(t);
        }
    }
    for (t, l) in traces.iter().zip(&looks) {
        let Some(group) = l.and_then(|l| groups.get(&l)) else { continue };
        if done.contains(&(t.line, t.col)) {
            continue;
        }
        if t.form.has_meaning() {
            let Some(f) = group.iter().take_while(|f| (f.line, f.col) < (t.line, t.col)).find(|f| f.form.has_meaning() && f.key != t.key) else { continue };
            diags.warning(
                t.line,
                t.col,
                format!("`{}` and `{}` on line {} look the same, but they have different meanings.", t.source, f.source, f.line),
                Some("Give one of them its own look, with a `set` rule or a different recipe.".into()),
            );
        } else {
            let Some(f) = group.iter().find(|f| f.form.has_meaning() && f.key != t.key) else { continue };
            if t.key == "fence" && !f.key.starts_with("kind ") {
                continue;
            }
            diags.warning(
                t.line,
                t.col,
                format!("`{}` looks the same as `{}` on line {}. Only `{}` tells the reader what it means.", t.source, f.source, f.line, f.source),
                Some(suggestion(f, t)),
            );
        }
        done.insert((t.line, t.col));
    }
}
