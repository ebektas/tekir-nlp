use serde::Serialize;
use tekir_core::{
    SegmentKind, SegmentSpan, TextRequest, TextTransformResponse, TextTransformer, TokenKind,
};
use tekir_tokenize::{TokenizeMode, TokenizeService};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentMode {
    Turkish,
}

pub type SegmentRequest = TextRequest<SegmentMode>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SegmentOutput {
    pub segments: Vec<SegmentSpan>,
    pub mode: SegmentMode,
    pub segment_count: usize,
}

pub type SegmentResponse = TextTransformResponse<SegmentOutput>;

#[derive(Debug, Default, Clone, Copy)]
pub struct SegmentService;

impl TextTransformer for SegmentService {
    type Options = SegmentMode;
    type Output = SegmentOutput;

    fn execute(&self, request: SegmentRequest) -> SegmentResponse {
        let segments = split_segments(&request.input, request.options);

        TextTransformResponse::new(
            request.input,
            SegmentOutput {
                segment_count: segments.len(),
                segments,
                mode: request.options,
            },
        )
    }
}

impl SegmentService {
    pub fn split(&self, input: impl Into<String>, mode: SegmentMode) -> SegmentResponse {
        self.execute(SegmentRequest::new(input, mode))
    }
}

fn split_segments(input: &str, mode: SegmentMode) -> Vec<SegmentSpan> {
    match mode {
        SegmentMode::Turkish => split_turkish_segments(input),
    }
}

fn split_turkish_segments(input: &str) -> Vec<SegmentSpan> {
    let mut segments = Vec::new();
    let mut cursor = 0;

    for block in split_paragraph_blocks(input) {
        if is_heading_block(&block.text) {
            segments.push(with_kind(block, SegmentKind::Heading));
            continue;
        }

        let block_sentences = split_block_sentences(input, block.start_byte, block.end_byte);
        if block_sentences.is_empty() {
            segments.push(with_kind(block, SegmentKind::Sentence));
            continue;
        }

        segments.extend(block_sentences);
        cursor = block.end_byte;
    }

    if segments.is_empty() && cursor < input.len() {
        let start = skip_leading_whitespace(input, cursor);
        if start < input.len() {
            push_sentence(input, &mut segments, start, input.len());
        }
    }

    segments
}

fn split_block_sentences(input: &str, start_byte: usize, end_byte: usize) -> Vec<SegmentSpan> {
    let tokenizer = TokenizeService;
    let token_response = tokenizer.tokenize(&input[start_byte..end_byte], TokenizeMode::Turkish);
    let tokens = token_response.output.tokens;

    if tokens.is_empty() {
        return Vec::new();
    }

    let mut sentences = Vec::new();
    let mut sentence_start = skip_leading_whitespace(input, start_byte);

    for (index, token) in tokens.iter().enumerate() {
        if !is_sentence_terminal(token.text.as_str()) {
            continue;
        }

        if should_skip_terminal(index, &tokens) {
            continue;
        }

        let mut sentence_end = start_byte + token.end_byte;
        while let Some(next_char) = input[sentence_end..end_byte].chars().next() {
            if is_trailing_closer(next_char) || next_char.is_whitespace() {
                sentence_end += next_char.len_utf8();
                continue;
            }
            break;
        }

        push_sentence(input, &mut sentences, sentence_start, sentence_end);
        sentence_start = skip_leading_whitespace(input, sentence_end);
    }

    if sentence_start < end_byte {
        push_sentence(input, &mut sentences, sentence_start, end_byte);
    }

    sentences
}

fn split_paragraph_blocks(input: &str) -> Vec<SegmentSpan> {
    let mut blocks = Vec::new();
    let mut block_start = 0;
    let mut separator_start = None;
    let mut newline_run = 0;

    for (byte_idx, ch) in input.char_indices() {
        if ch == '\n' {
            if separator_start.is_none() {
                separator_start = Some(byte_idx);
            }
            newline_run += 1;

            if newline_run >= 2 {
                let end = separator_start.unwrap_or(byte_idx);
                push_span(input, &mut blocks, block_start, end, SegmentKind::Sentence);
            }
            continue;
        }

        if newline_run >= 2 {
            block_start = byte_idx;
        }

        newline_run = 0;
        separator_start = None;
    }

    push_span(
        input,
        &mut blocks,
        block_start,
        input.len(),
        SegmentKind::Sentence,
    );
    blocks
}

fn is_heading_block(text: &str) -> bool {
    if text.contains('\n') {
        return false;
    }

    let trimmed = text.trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\''));
    if trimmed.is_empty() {
        return false;
    }

    let has_lowercase = trimmed.chars().any(char::is_lowercase);
    let has_letter = trimmed.chars().any(char::is_alphabetic);
    let uppercase_ratio = trimmed
        .chars()
        .filter(|ch| ch.is_alphabetic())
        .filter(|ch| ch.is_uppercase())
        .count();
    let letter_count = trimmed.chars().filter(|ch| ch.is_alphabetic()).count();

    has_letter && !has_lowercase && letter_count > 2 && uppercase_ratio == letter_count
}

fn is_lead_in_block(text: &str) -> bool {
    text.ends_with(':')
}

fn should_skip_terminal(index: usize, tokens: &[tekir_core::Token]) -> bool {
    let previous = match index.checked_sub(1).and_then(|prev| tokens.get(prev)) {
        Some(token) => token,
        None => return false,
    };

    if tokens[index].text == "." && matches!(previous.kind, TokenKind::Abbreviation) {
        return true;
    }

    false
}

fn is_sentence_terminal(text: &str) -> bool {
    matches!(text, "." | "!" | "?" | "…")
}

fn is_trailing_closer(ch: char) -> bool {
    matches!(ch, '"' | '\'' | ')' | ']' | '}')
}

fn skip_leading_whitespace(input: &str, start: usize) -> usize {
    let mut offset = start;

    while let Some(ch) = input[offset..].chars().next() {
        if ch.is_whitespace() {
            offset += ch.len_utf8();
            continue;
        }
        break;
    }

    offset
}

fn push_sentence(
    input: &str,
    sentences: &mut Vec<SegmentSpan>,
    start_byte: usize,
    end_byte: usize,
) {
    let text = input[start_byte..end_byte].trim();
    let kind = if is_lead_in_block(text) {
        SegmentKind::LeadIn
    } else {
        SegmentKind::Sentence
    };

    push_span(input, sentences, start_byte, end_byte, kind);
}

fn push_span(
    input: &str,
    sentences: &mut Vec<SegmentSpan>,
    start_byte: usize,
    end_byte: usize,
    kind: SegmentKind,
) {
    if start_byte >= end_byte {
        return;
    }

    let text = input[start_byte..end_byte].trim();
    if text.is_empty() {
        return;
    }

    let trimmed_start = input[start_byte..end_byte]
        .find(text)
        .map(|relative| start_byte + relative)
        .unwrap_or(start_byte);
    let trimmed_end = trimmed_start + text.len();

    sentences.push(SegmentSpan::new(text, trimmed_start, trimmed_end, kind));
}

fn with_kind(mut span: SegmentSpan, kind: SegmentKind) -> SegmentSpan {
    span.kind = kind;
    span
}

#[cfg(test)]
mod tests {
    use super::{SegmentMode, SegmentRequest, SegmentService};
    use tekir_core::{SegmentKind, TextTransformer};

    #[test]
    fn splits_simple_sentences() {
        let service = SegmentService;
        let response = service.execute(SegmentRequest::new(
            "Merhaba dunya. Nasılsın? İyiyim!",
            SegmentMode::Turkish,
        ));

        assert_eq!(
            response
                .output
                .segments
                .iter()
                .map(|sentence| sentence.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Merhaba dunya.", "Nasılsın?", "İyiyim!"]
        );
    }

    #[test]
    fn does_not_split_after_common_abbreviations() {
        let service = SegmentService;
        let response = service.execute(SegmentRequest::new(
            "Dr. Ahmet geldi. Merhaba dedi.",
            SegmentMode::Turkish,
        ));

        assert_eq!(
            response
                .output
                .segments
                .iter()
                .map(|sentence| sentence.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Dr. Ahmet geldi.", "Merhaba dedi."]
        );
    }

    #[test]
    fn keeps_byte_spans_for_sentences() {
        let service = SegmentService;
        let response = service.execute(SegmentRequest::new(
            "Ankara'ya gitti mi? Evet.",
            SegmentMode::Turkish,
        ));

        assert_eq!(response.output.segments[0].start_byte, 0);
        assert_eq!(response.output.segments[0].end_byte, 19);
        assert_eq!(response.output.segments[1].start_byte, 20);
        assert_eq!(response.output.segments[0].kind, SegmentKind::Sentence);
    }

    #[test]
    fn splits_news_headings_and_paragraphs() {
        let service = SegmentService;
        let response = service.execute(SegmentRequest::new(
            "'MAYIS ORTASINI AŞMAYACAK ŞEKİLDE'\n\nBağımsız Maden İş Sendikası Avukatı dedi.\n\nİÇİŞLERİ BAKANLIĞI'NDAN AÇIKLAMA\n\nİçişleri Bakanlığı'ndan açıklama yapıldı.",
            SegmentMode::Turkish,
        ));

        assert_eq!(
            response
                .output
                .segments
                .iter()
                .map(|sentence| sentence.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "'MAYIS ORTASINI AŞMAYACAK ŞEKİLDE'",
                "Bağımsız Maden İş Sendikası Avukatı dedi.",
                "İÇİŞLERİ BAKANLIĞI'NDAN AÇIKLAMA",
                "İçişleri Bakanlığı'ndan açıklama yapıldı."
            ]
        );
        assert_eq!(response.output.segments[0].kind, SegmentKind::Heading);
        assert_eq!(response.output.segments[2].kind, SegmentKind::Heading);
    }

    #[test]
    fn marks_colon_ended_intro_as_lead_in() {
        let service = SegmentService;
        let response = service.execute(SegmentRequest::new(
            "Açıklamada şu ifadelere yer verildi:",
            SegmentMode::Turkish,
        ));

        assert_eq!(response.output.segments[0].kind, SegmentKind::LeadIn);
    }
}
