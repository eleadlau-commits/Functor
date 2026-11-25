pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn opt(s: Option<&str>) -> String {
    s.map_or_else(|| "null".into(), string)
}

pub struct Obj(String);

impl Obj {
    pub fn new() -> Self {
        Obj(String::from("{"))
    }

    pub fn raw(mut self, key: &str, value: impl AsRef<str>) -> Self {
        if self.0.len() > 1 {
            self.0.push(',');
        }
        self.0.push_str(&string(key));
        self.0.push(':');
        self.0.push_str(value.as_ref());
        self
    }

    pub fn str(self, key: &str, value: &str) -> Self {
        let v = string(value);
        self.raw(key, v)
    }

    pub fn num(self, key: &str, value: impl std::fmt::Display) -> Self {
        let v = value.to_string();
        self.raw(key, v)
    }

    pub fn end(mut self) -> String {
        self.0.push('}');
        self.0
    }
}

pub fn array(items: impl IntoIterator<Item = String>) -> String {
    let v: Vec<String> = items.into_iter().collect();
    format!("[{}]", v.join(","))
}

#[cfg(test)]
mod tests {
    #[test]
    fn escapes() {
        assert_eq!(super::string("a\"b\\\n\u{1}"), "\"a\\\"b\\\\\\n\\u0001\"");
        let o = super::Obj::new().str("k", "v").num("n", 2).end();
        assert_eq!(o, "{\"k\":\"v\",\"n\":2}");
    }
}
