use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Word,
    ApostrophedWord,
    Number,
    Abbreviation,
    Punctuation,
    Grapheme,
    Line,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Token {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: TokenKind,
}

impl Token {
    pub fn new(
        text: impl Into<String>,
        start_byte: usize,
        end_byte: usize,
        kind: TokenKind,
    ) -> Self {
        Self {
            text: text.into(),
            start_byte,
            end_byte,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SegmentSpan {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: SegmentKind,
}

impl SegmentSpan {
    pub fn new(
        text: impl Into<String>,
        start_byte: usize,
        end_byte: usize,
        kind: SegmentKind,
    ) -> Self {
        Self {
            text: text.into(),
            start_byte,
            end_byte,
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Sentence,
    Heading,
    LeadIn,
}
