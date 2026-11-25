#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Default)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn error(&mut self, line: usize, col: usize, message: impl Into<String>, hint: Option<String>) {
        self.push(Severity::Error, line, col, message.into(), hint);
    }

    pub fn warning(&mut self, line: usize, col: usize, message: impl Into<String>, hint: Option<String>) {
        self.push(Severity::Warning, line, col, message.into(), hint);
    }

    fn push(&mut self, severity: Severity, line: usize, col: usize, message: String, hint: Option<String>) {
        self.items.push(Diagnostic { severity, line, col: col.max(1), message, hint });
    }

    pub fn finish(mut self) -> Vec<Diagnostic> {
        self.items.sort_by_key(|d| (d.line, d.col));
        self.items
    }
}

pub fn did_you_mean<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut best: Option<(usize, &str)> = None;
    for c in candidates {
        let d = edit_distance(word, c);
        if best.map_or(true, |(bd, _)| d < bd) {
            best = Some((d, c));
        }
    }
    let (d, c) = best?;
    let limit = (word.chars().count().max(c.chars().count()) / 3).max(1);
    (d <= limit).then(|| c.to_string())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let w = b.len() + 1;
    let mut d = vec![0usize; (a.len() + 1) * w];
    for i in 0..=a.len() {
        d[i * w] = i;
    }
    for j in 0..=b.len() {
        d[j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut v = (d[(i - 1) * w + j] + 1).min(d[i * w + j - 1] + 1).min(d[(i - 1) * w + j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                v = v.min(d[(i - 2) * w + j - 2] + 1);
            }
            d[i * w + j] = v;
        }
    }
    d[a.len() * w + b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_close_words_only() {
        assert_eq!(did_you_mean("theorm", ["theorem", "figure"]), Some("theorem".into()));
        assert_eq!(did_you_mean("zzz", ["theorem", "figure"]), None);
        assert_eq!(did_you_mean("alpah", ["alpha"]), Some("alpha".into()));
    }
}
