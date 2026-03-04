#[derive(Debug, PartialEq, Clone)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
    FuzzyFinder,
}

#[derive(Debug, PartialEq, Clone)]
pub enum FuzzyFinderType {
    Files,
    Grep,
}

#[derive(Debug, Clone)]
pub enum YankType {
    Character,
    Line,
}

#[derive(Debug, Clone)]
pub struct YankRegister {
    pub text: Vec<String>,
    pub yank_type: YankType,
}
