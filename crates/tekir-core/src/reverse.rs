use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::service::{TextRequest, TextTransformResponse, TextTransformer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReverseMode {
    Graphemes,
    Chars,
    Words,
    Lines,
}

pub type ReverseRequest = TextRequest<ReverseMode>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReverseOutput {
    pub transformed: String,
    pub mode: ReverseMode,
    pub output_len_bytes: usize,
}

pub type ReverseResponse = TextTransformResponse<ReverseOutput>;

#[derive(Debug, Default, Clone, Copy)]
pub struct ReverseService;

impl TextTransformer for ReverseService {
    type Options = ReverseMode;
    type Output = ReverseOutput;

    fn execute(&self, request: ReverseRequest) -> ReverseResponse {
        let transformed = reverse_text(&request.input, request.options);

        TextTransformResponse::new(
            request.input,
            ReverseOutput {
                transformed: transformed.clone(),
                mode: request.options,
                output_len_bytes: transformed.len(),
            },
        )
    }
}

impl ReverseService {
    pub fn reverse(&self, input: impl Into<String>, mode: ReverseMode) -> ReverseResponse {
        self.execute(ReverseRequest::new(input, mode))
    }
}

fn reverse_text(input: &str, mode: ReverseMode) -> String {
    match mode {
        ReverseMode::Graphemes => input.graphemes(true).rev().collect(),
        ReverseMode::Chars => input.chars().rev().collect(),
        ReverseMode::Words => input.split_whitespace().rev().collect::<Vec<_>>().join(" "),
        ReverseMode::Lines => input.lines().rev().collect::<Vec<_>>().join("\n"),
    }
}

#[cfg(test)]
mod tests {
    use crate::TextTransformer;

    use super::{ReverseMode, ReverseRequest, ReverseService};

    #[test]
    fn reverses_graphemes_without_splitting_combining_marks() {
        let service = ReverseService;
        let response = service.execute(ReverseRequest::new("a\u{0301}b", ReverseMode::Graphemes));

        assert_eq!(response.output.transformed, "ba\u{0301}");
    }

    #[test]
    fn reverses_chars_by_scalar_value() {
        let service = ReverseService;
        let response = service.execute(ReverseRequest::new("Tekir", ReverseMode::Chars));

        assert_eq!(response.output.transformed, "rikeT");
    }

    #[test]
    fn reverses_word_order() {
        let service = ReverseService;
        let response = service.execute(ReverseRequest::new(
            "merhaba guzel dunya",
            ReverseMode::Words,
        ));

        assert_eq!(response.output.transformed, "dunya guzel merhaba");
    }

    #[test]
    fn reverses_line_order() {
        let service = ReverseService;
        let response = service.execute(ReverseRequest::new(
            "satir bir\nsatir iki\nsatir uc",
            ReverseMode::Lines,
        ));

        assert_eq!(
            response.output.transformed,
            "satir uc\nsatir iki\nsatir bir"
        );
    }
}
