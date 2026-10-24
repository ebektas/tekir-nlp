use serde::Serialize;
use tekir_core::{TextRequest, TextTransformResponse, TextTransformer, Token, TokenKind};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenizeMode {
    Turkish,
    Whitespace,
    Graphemes,
    Lines,
}

pub type TokenizeRequest = TextRequest<TokenizeMode>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TokenizeOutput {
    pub tokens: Vec<Token>,
    pub mode: TokenizeMode,
    pub token_count: usize,
}

pub type TokenizeResponse = TextTransformResponse<TokenizeOutput>;

#[derive(Debug, Default, Clone, Copy)]
pub struct TokenizeService;

impl TextTransformer for TokenizeService {
    type Options = TokenizeMode;
    type Output = TokenizeOutput;

    fn execute(&self, request: TokenizeRequest) -> TokenizeResponse {
        let tokens = tokenize_text(&request.input, request.options);

        TextTransformResponse::new(
            request.input,
            TokenizeOutput {
                token_count: tokens.len(),
                tokens,
                mode: request.options,
            },
        )
    }
}

impl TokenizeService {
    pub fn tokenize(&self, input: impl Into<String>, mode: TokenizeMode) -> TokenizeResponse {
        self.execute(TokenizeRequest::new(input, mode))
    }
}

fn tokenize_text(input: &str, mode: TokenizeMode) -> Vec<Token> {
    match mode {
        TokenizeMode::Turkish => tokenize_turkish(input),
        TokenizeMode::Whitespace => tokenize_whitespace(input),
        TokenizeMode::Graphemes => tokenize_graphemes(input),
        TokenizeMode::Lines => tokenize_lines(input),
    }
}

fn tokenize_turkish(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_start = 0;

    for (byte_idx, ch) in input.char_indices() {
        if ch.is_whitespace() {
            push_current(&mut tokens, &mut current, &mut current_start, byte_idx);
            continue;
        }

        if current.is_empty() {
            current_start = byte_idx;
        }

        if is_turkish_token_char(ch, current.is_empty()) {
            current.push(ch);
            continue;
        }

        push_current(&mut tokens, &mut current, &mut current_start, byte_idx);
        tokens.push(Token::new(
            ch.to_string(),
            byte_idx,
            byte_idx + ch.len_utf8(),
            TokenKind::Punctuation,
        ));
    }

    push_current(&mut tokens, &mut current, &mut current_start, input.len());
    tokens
}

fn is_turkish_token_char(ch: char, token_is_empty: bool) -> bool {
    ch.is_alphanumeric() || matches!(ch, '_') || (matches!(ch, '\'' | '’') && !token_is_empty)
}

fn push_current(
    tokens: &mut Vec<Token>,
    current: &mut String,
    current_start: &mut usize,
    end_byte: usize,
) {
    if !current.is_empty() {
        let token_text = std::mem::take(current);
        tokens.push(Token::new(
            token_text.clone(),
            *current_start,
            end_byte,
            classify_word_kind(&token_text),
        ));
    }
}

fn classify_word_kind(text: &str) -> TokenKind {
    if text.contains(['\'', '’']) {
        TokenKind::ApostrophedWord
    } else if is_number_like(text) {
        TokenKind::Number
    } else if is_abbreviation_like(text) {
        TokenKind::Abbreviation
    } else {
        TokenKind::Word
    }
}

fn is_number_like(text: &str) -> bool {
    text.chars().all(|ch| ch.is_ascii_digit()) && !text.is_empty()
}

fn is_abbreviation_like(text: &str) -> bool {
    let common = [
        "Dr", "Prof", "Doç", "Sn", "Av", "Arş", "Gör", "bkz", "vb", "vs", "örn", "No", "Mad",
    ];

    common.contains(&text) || (text.chars().all(|ch| ch.is_uppercase()) && text.chars().count() > 1)
}

fn tokenize_whitespace(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current_start = None;

    for (byte_idx, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if let Some(start_byte) = current_start.take() {
                tokens.push(Token::new(
                    &input[start_byte..byte_idx],
                    start_byte,
                    byte_idx,
                    TokenKind::Word,
                ));
            }
        } else if current_start.is_none() {
            current_start = Some(byte_idx);
        }
    }

    if let Some(start_byte) = current_start {
        tokens.push(Token::new(
            &input[start_byte..input.len()],
            start_byte,
            input.len(),
            TokenKind::Word,
        ));
    }

    tokens
}

fn tokenize_graphemes(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();

    for (byte_idx, grapheme) in input.grapheme_indices(true) {
        tokens.push(Token::new(
            grapheme,
            byte_idx,
            byte_idx + grapheme.len(),
            TokenKind::Grapheme,
        ));
    }

    tokens
}

fn tokenize_lines(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut offset = 0;

    for line in input.split('\n') {
        let start_byte = offset;
        let end_byte = start_byte + line.len();
        tokens.push(Token::new(line, start_byte, end_byte, TokenKind::Line));
        offset = end_byte + 1;
    }

    if input.is_empty() {
        tokens.clear();
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::{TokenizeMode, TokenizeRequest, TokenizeService};
    use tekir_core::{TextTransformer, TokenKind};

    #[test]
    fn turkish_mode_keeps_apostrophe_suffixes_and_splits_punctuation() {
        let service = TokenizeService;
        let response = service.execute(TokenizeRequest::new(
            "Ankara'ya gidiyor, değil mi?",
            TokenizeMode::Turkish,
        ));

        assert_eq!(
            response
                .output
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Ankara'ya", "gidiyor", ",", "değil", "mi", "?"]
        );
        assert_eq!(response.output.tokens[0].start_byte, 0);
        assert_eq!(response.output.tokens[0].end_byte, 9);
        assert_eq!(response.output.tokens[0].kind, TokenKind::ApostrophedWord);
        assert_eq!(response.output.tokens[2].kind, TokenKind::Punctuation);
    }

    #[test]
    fn classifies_numbers_and_abbreviations() {
        let service = TokenizeService;
        let response = service.execute(TokenizeRequest::new(
            "Dr. 2026 geldi.",
            TokenizeMode::Turkish,
        ));

        assert_eq!(response.output.tokens[0].kind, TokenKind::Abbreviation);
        assert_eq!(response.output.tokens[2].kind, TokenKind::Number);
    }

    #[test]
    fn tokenizes_by_whitespace() {
        let service = TokenizeService;
        let response = service.execute(TokenizeRequest::new(
            "merhaba guzel dunya",
            TokenizeMode::Whitespace,
        ));

        assert_eq!(
            response
                .output
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            vec!["merhaba", "guzel", "dunya"]
        );
        assert_eq!(response.output.token_count, 3);
    }

    #[test]
    fn tokenizes_by_graphemes() {
        let service = TokenizeService;
        let response = service.execute(TokenizeRequest::new("sa", TokenizeMode::Graphemes));

        assert_eq!(
            response
                .output
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            vec!["s", "a"]
        );
        assert_eq!(response.output.tokens[0].kind, TokenKind::Grapheme);
    }

    #[test]
    fn tokenizes_by_lines() {
        let service = TokenizeService;
        let response = service.execute(TokenizeRequest::new("bir\niki", TokenizeMode::Lines));

        assert_eq!(
            response
                .output
                .tokens
                .iter()
                .map(|token| token.text.as_str())
                .collect::<Vec<_>>(),
            vec!["bir", "iki"]
        );
        assert_eq!(response.output.tokens[1].start_byte, 4);
        assert_eq!(response.output.tokens[1].kind, TokenKind::Line);
    }
}
