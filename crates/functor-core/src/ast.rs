#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emph(Vec<Inline>),
    Code(String),
    Math { src: String, line: usize, col: usize },
    Link { text: Vec<Inline>, url: String },
    Image { alt: String, src: String },
    Ref { label: String, line: usize, col: usize },
    Cite(Vec<String>),
    Note(Vec<Inline>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading { level: usize, content: Vec<Inline>, label: Option<String>, number: Option<String> },
    Para(Vec<Inline>),
    Math { src: String, line: usize, col: usize, label: Option<String>, number: Option<usize> },
    List { ordered: bool, items: Vec<Vec<Inline>> },
    Table { align: Vec<Align>, header: Vec<Vec<Inline>>, rows: Vec<Vec<Vec<Inline>>> },
    Code { lang: String, text: String },
    Env(Env),
    Rule,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Env {
    pub kind: String,
    pub label: Option<String>,
    pub title: Option<String>,
    pub width: Option<String>,
    pub number: Option<usize>,
    pub caption: Option<Vec<Inline>>,
    pub body: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Heading { number: Option<String>, text: String },
    Env { kind: String, number: usize },
    Equation(usize),
}
