use serde::Serialize;
use tekir_core::TextTransformResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizeMode {
    Basic,
    Turkish,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizeOutput {
    pub normalized: String,
    pub mode: NormalizeMode,
    pub output_len_bytes: usize,
}

pub type NormalizeResponse = TextTransformResponse<NormalizeOutput>;

#[derive(Debug, Default, Clone, Copy)]
pub struct NormalizeService;

impl NormalizeService {
    pub fn normalize(&self, input: impl Into<String>, mode: NormalizeMode) -> NormalizeResponse {
        let input = input.into();
        let normalized = normalize_text(&input, mode);

        TextTransformResponse::new(
            input,
            NormalizeOutput {
                output_len_bytes: normalized.len(),
                normalized,
                mode: mode,
            },
        )
    }
}

fn normalize_text(input: &str, mode: NormalizeMode) -> String {
    let cleaned = strip_invisible_chars(input);
    let normalized_punctuation = normalize_unicode_punctuation(&cleaned);
    let normalized_quotes = normalize_quotes(&normalized_punctuation);
    let collapsed = collapse_whitespace(&normalized_quotes);
    let spacing_fixed = normalize_token_spacing(&collapsed);

    match mode {
        NormalizeMode::Basic => spacing_fixed,
        NormalizeMode::Turkish => turkish_lowercase(&spacing_fixed),
    }
}

fn strip_invisible_chars(input: &str) -> String {
    input
        .chars()
        .filter(|ch| {
            !matches!(
                ch,
                '\u{00AD}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}'
            )
        })
        .collect()
}

fn normalize_unicode_punctuation(input: &str) -> String {
    input
        .chars()
        .flat_map(|ch| match ch {
            '\u{2026}' => "...".chars().collect::<Vec<_>>(),
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => {
                vec!['-']
            }
            _ => vec![ch],
        })
        .collect()
}

fn normalize_quotes(input: &str) -> String {
    input
        .chars()
        .map(|ch| match ch {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '\u{2032}' | '\u{0060}' | '\u{00B4}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            _ => ch,
        })
        .collect()
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_token_spacing(input: &str) -> String {
    let tokens = input
        .split(' ')
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let mut normalized = Vec::new();
    let mut index = 0usize;

    while index < tokens.len() {
        let token = tokens[index];

        if token == "'" && can_join_apostrophe(&normalized, tokens.get(index + 1).copied()) {
            if let Some(previous) = normalized.last_mut() {
                previous.push('\'');
            }
            index += 1;
            continue;
        }

        if should_attach_to_previous(token, &normalized) {
            if let Some(previous) = normalized.last_mut() {
                previous.push_str(token);
            }
            index += 1;
            continue;
        }

        normalized.push(token.to_string());
        index += 1;
    }

    normalized.join(" ")
}

fn can_join_apostrophe(previous_tokens: &[String], next_token: Option<&str>) -> bool {
    previous_tokens
        .last()
        .is_some_and(|previous| ends_with_letter(previous))
        && next_token.is_some_and(starts_with_letter)
}

fn should_attach_to_previous(token: &str, normalized: &[String]) -> bool {
    normalized.last().is_some() && {
        is_closing_punctuation(token)
            || (token.starts_with('\'') && ends_previous_with_letter(normalized))
            || (starts_with_letter(token) && previous_ends_with_apostrophe(normalized))
    }
}

fn ends_previous_with_letter(normalized: &[String]) -> bool {
    normalized
        .last()
        .is_some_and(|token| ends_with_letter(token))
}

fn previous_ends_with_apostrophe(normalized: &[String]) -> bool {
    normalized.last().is_some_and(|token| token.ends_with('\''))
}

fn starts_with_letter(input: &str) -> bool {
    input.chars().next().is_some_and(char::is_alphabetic)
}

fn ends_with_letter(input: &str) -> bool {
    input.chars().last().is_some_and(char::is_alphabetic)
}

fn is_closing_punctuation(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|ch| matches!(ch, ',' | '.' | '!' | '?' | ':' | ';' | ')' | ']' | '}'))
}

fn turkish_lowercase(input: &str) -> String {
    let mut output = String::with_capacity(input.len());

    for ch in input.chars() {
        match ch {
            'I' => output.push('ı'),
            'İ' => output.push('i'),
            _ => output.extend(ch.to_lowercase()),
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{NormalizeMode, NormalizeService};

    #[test]
    fn basic_mode_collapses_whitespace_and_normalizes_quotes() {
        let service = NormalizeService;
        let response =
            service.normalize("  Merhaba\tDünya\u{2019}dan  selam  ", NormalizeMode::Basic);

        assert_eq!(response.output.normalized, "Merhaba Dünya'dan selam");
    }

    #[test]
    fn turkish_mode_uses_turkish_aware_lowercasing() {
        let service = NormalizeService;
        let response = service.normalize("I İZMİR ISTANBUL", NormalizeMode::Turkish);

        assert_eq!(response.output.normalized, "ı izmir ıstanbul");
    }

    #[test]
    fn basic_mode_strips_invisible_chars_and_normalizes_unicode_punctuation() {
        let service = NormalizeService;
        let response = service.normalize(
            "Mer\u{200B}haba\u{00AD}...\u{2026} dünya\u{2014}dan",
            NormalizeMode::Basic,
        );

        assert_eq!(response.output.normalized, "Merhaba...... dünya-dan");
    }

    #[test]
    fn basic_mode_repairs_apostrophe_suffix_spacing() {
        let service = NormalizeService;
        let response = service.normalize(
            "Ankara \u{2019} ya gittik , kitap \u{2019} ları aldık .",
            NormalizeMode::Basic,
        );

        assert_eq!(
            response.output.normalized,
            "Ankara'ya gittik, kitap'ları aldık."
        );
    }
}
