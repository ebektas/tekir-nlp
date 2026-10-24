//! Rule-based Turkish PII detection and redaction.
//!
//! One left-to-right pass over the bytes with hand-written matchers (no
//! regex engine, no allocation besides the output `Vec`). Candidates are
//! only tried at token boundaries, and every numeric kind is validated by
//! its checksum (TCKN check digits, IBAN mod-97, Luhn) or by numbering-plan
//! rules (phones), which is what keeps false positives on ordinary numbers
//! low. Spans are byte offsets into the input, sorted and non-overlapping.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PiiKind {
    /// T.C. Kimlik Numarası: 11 digits with two check digits.
    Tckn,
    /// Turkish IBAN: `TR` + 24 digits, optionally in groups of four.
    Iban,
    /// Payment card number: 13-19 digits, Luhn-valid.
    CreditCard,
    /// Turkish phone number (mobile `5xx` or landline), with `+90`/`0`
    /// prefixes and common separators.
    Phone,
    Email,
    Ipv4,
}

impl PiiKind {
    /// A default redaction placeholder such as `[TCKN]`.
    pub fn placeholder(self) -> &'static str {
        match self {
            PiiKind::Tckn => "[TCKN]",
            PiiKind::Iban => "[IBAN]",
            PiiKind::CreditCard => "[CREDIT_CARD]",
            PiiKind::Phone => "[PHONE]",
            PiiKind::Email => "[EMAIL]",
            PiiKind::Ipv4 => "[IPV4]",
        }
    }
}

/// A detected PII span: `text[start_byte..end_byte]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PiiSpan {
    pub kind: PiiKind,
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Detects all PII spans in `text`, in order.
pub fn detect(text: &str) -> Vec<PiiSpan> {
    let mut spans = Vec::new();
    detect_into(text, &mut spans);
    spans
}

/// Appends all PII spans in `text` to `out` (reuse `out` across documents
/// to avoid reallocating).
pub fn detect_into(text: &str, out: &mut Vec<PiiSpan>) {
    let bytes = text.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        if is_candidate_start(bytes, index)
            && left_boundary(bytes, index)
            && let Some((kind, end)) = match_at(bytes, index)
        {
            out.push(PiiSpan {
                kind,
                start_byte: index,
                end_byte: end,
            });
            index = end;
            continue;
        }
        // Skip the rest of the word: matches only start at boundaries. Word
        // bytes include all non-ASCII bytes, so `index` stays on a char
        // boundary.
        if is_word_byte(byte) {
            while index < bytes.len() && is_word_byte(bytes[index]) {
                index += 1;
            }
        } else {
            index += 1;
        }
    }
}

/// Detects PII in `text` and replaces each span with `replace(kind)`.
///
/// ```
/// use tekir_prep::pii::{PiiKind, redact};
///
/// let out = redact("Ara: 0532 123 45 67", PiiKind::placeholder);
/// assert_eq!(out, "Ara: [PHONE]");
/// ```
pub fn redact<F, R>(text: &str, replace: F) -> String
where
    F: FnMut(PiiKind) -> R,
    R: AsRef<str>,
{
    redact_spans(text, &detect(text), replace)
}

/// Replaces the given spans (sorted, non-overlapping, as returned by
/// [`detect`], possibly filtered) with `replace(kind)`.
pub fn redact_spans<F, R>(text: &str, spans: &[PiiSpan], mut replace: F) -> String
where
    F: FnMut(PiiKind) -> R,
    R: AsRef<str>,
{
    let mut output = String::with_capacity(text.len());
    let mut last = 0;
    for span in spans {
        debug_assert!(span.start_byte >= last, "spans must be sorted");
        output.push_str(&text[last..span.start_byte]);
        output.push_str(replace(span.kind).as_ref());
        last = span.end_byte;
    }
    output.push_str(&text[last..]);
    output
}

fn match_at(bytes: &[u8], index: usize) -> Option<(PiiKind, usize)> {
    let byte = bytes[index];
    if byte.is_ascii_alphanumeric()
        && let Some(end) = match_email(bytes, index)
    {
        return Some((PiiKind::Email, end));
    }
    if matches!(byte, b'T' | b't')
        && let Some(end) = match_iban(bytes, index)
    {
        return Some((PiiKind::Iban, end));
    }
    if byte.is_ascii_digit()
        && let Some(end) = match_ipv4(bytes, index)
    {
        return Some((PiiKind::Ipv4, end));
    }
    if matches!(byte, b'0'..=b'9' | b'+' | b'(') {
        return match_number(bytes, index);
    }
    None
}

/// ASCII alphanumerics and every non-ASCII byte (Turkish letters etc.).
fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte >= 0x80
}

fn is_candidate_start(bytes: &[u8], index: usize) -> bool {
    match bytes[index] {
        b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' => true,
        b'+' | b'(' => bytes.get(index + 1).is_some_and(u8::is_ascii_digit),
        _ => false,
    }
}

/// Not glued to a preceding word, address, or decimal number.
fn left_boundary(bytes: &[u8], index: usize) -> bool {
    let Some(&prev) = index.checked_sub(1).and_then(|i| bytes.get(i)) else {
        return true;
    };
    if is_word_byte(prev) || prev == b'@' {
        return false;
    }
    let numeric_joiner = matches!(prev, b'.' | b',' | b'-' | b'/');
    !(numeric_joiner && index >= 2 && bytes[index - 2].is_ascii_digit())
}

/// Not followed by more word, address, or number.
fn right_boundary(bytes: &[u8], end: usize) -> bool {
    let Some(&next) = bytes.get(end) else {
        return true;
    };
    if is_word_byte(next) || next == b'@' {
        return false;
    }
    let numeric_joiner = matches!(next, b'.' | b',' | b'-' | b'/');
    !(numeric_joiner && bytes.get(end + 1).is_some_and(u8::is_ascii_digit))
}

fn is_email_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-')
}

fn match_email(bytes: &[u8], start: usize) -> Option<usize> {
    if start > 0 && is_email_local_byte(bytes[start - 1]) {
        return None;
    }
    let mut at = start;
    while at < bytes.len() && at - start <= 64 && is_email_local_byte(bytes[at]) {
        at += 1;
    }
    if bytes.get(at) != Some(&b'@') || bytes[at - 1] == b'.' {
        return None;
    }

    let mut cursor = at + 1;
    let mut labels = 0;
    let mut last_label;
    loop {
        let label_start = cursor;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'-')
        {
            cursor += 1;
        }
        if cursor == label_start || bytes[label_start] == b'-' || bytes[cursor - 1] == b'-' {
            return None;
        }
        labels += 1;
        last_label = (label_start, cursor);
        let continues = bytes.get(cursor) == Some(&b'.')
            && bytes.get(cursor + 1).is_some_and(u8::is_ascii_alphanumeric);
        if !continues {
            break;
        }
        cursor += 1;
    }

    let tld = &bytes[last_label.0..last_label.1];
    let valid = labels >= 2 && tld.len() >= 2 && tld.iter().all(u8::is_ascii_alphabetic);
    (valid && right_boundary(bytes, cursor)).then_some(cursor)
}

/// `TR` + 24 digits; a single space is allowed at each group-of-four
/// boundary (`TR33 0006 1005 ...`). Validated with ISO 7064 mod-97.
fn match_iban(bytes: &[u8], start: usize) -> Option<usize> {
    if !matches!(bytes.get(start + 1), Some(b'R' | b'r')) {
        return None;
    }
    let mut digits = [0u8; 24];
    let mut cursor = start + 2;
    for (index, digit) in digits.iter_mut().enumerate() {
        // `index + 2` is the character position within the IBAN.
        if (index + 2) % 4 == 0
            && bytes.get(cursor) == Some(&b' ')
            && bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit)
        {
            cursor += 1;
        }
        let byte = *bytes.get(cursor).filter(|byte| byte.is_ascii_digit())?;
        *digit = byte - b'0';
        cursor += 1;
    }
    (right_boundary(bytes, cursor) && iban_checksum_ok(&digits)).then_some(cursor)
}

fn iban_checksum_ok(digits: &[u8; 24]) -> bool {
    // Move "TR" + check digits to the end; T = 29, R = 27.
    let rearranged = digits[2..].iter().chain(&[2, 9, 2, 7]).chain(&digits[..2]);
    rearranged.fold(0u32, |rem, &digit| (rem * 10 + u32::from(digit)) % 97) == 1
}

fn match_ipv4(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start;
    for octet in 0..4 {
        if octet > 0 {
            if bytes.get(cursor) != Some(&b'.') {
                return None;
            }
            cursor += 1;
        }
        let octet_start = cursor;
        let mut value = 0u32;
        while cursor < bytes.len() && bytes[cursor].is_ascii_digit() && cursor - octet_start < 3 {
            value = value * 10 + u32::from(bytes[cursor] - b'0');
            cursor += 1;
        }
        let len = cursor - octet_start;
        let leading_zero = len > 1 && bytes[octet_start] == b'0';
        let too_long = bytes.get(cursor).is_some_and(u8::is_ascii_digit);
        if len == 0 || leading_zero || too_long || value > 255 {
            return None;
        }
    }
    right_boundary(bytes, cursor).then_some(cursor)
}

const MAX_DIGITS: usize = 24;
const MAX_GROUPS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Sep {
    #[default]
    None,
    Space,
    Dash,
    Other,
}

#[derive(Debug, Clone, Copy, Default)]
struct Group {
    /// Cumulative digit count at the end of this group.
    digit_end: usize,
    byte_end: usize,
    sep_before: Sep,
}

/// A run of digit groups joined by short separators, collected on the
/// stack. Each prefix of whole groups is a candidate number.
struct DigitRun {
    plus: bool,
    digits: [u8; MAX_DIGITS],
    groups: [Group; MAX_GROUPS],
    group_count: usize,
}

impl DigitRun {
    fn collect(bytes: &[u8], start: usize) -> Self {
        let mut run = DigitRun {
            plus: false,
            digits: [0; MAX_DIGITS],
            groups: [Group::default(); MAX_GROUPS],
            group_count: 0,
        };
        let mut cursor = start;
        let mut sep = Sep::None;
        if bytes[cursor] == b'+' {
            run.plus = true;
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'(') {
            sep = Sep::Other;
            cursor += 1;
        }
        let mut count = 0;

        while run.group_count < MAX_GROUPS {
            let group_start = cursor;
            while cursor < bytes.len() && bytes[cursor].is_ascii_digit() && count < MAX_DIGITS {
                run.digits[count] = bytes[cursor] - b'0';
                count += 1;
                cursor += 1;
            }
            if cursor == group_start || bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                // Empty, or too many digits overall: drop this group.
                break;
            }
            run.groups[run.group_count] = Group {
                digit_end: count,
                byte_end: cursor,
                sep_before: sep,
            };
            run.group_count += 1;

            let sep_start = cursor;
            while cursor < bytes.len()
                && cursor - sep_start < 2
                && matches!(bytes[cursor], b' ' | b'-' | b'.' | b'/' | b'(' | b')')
            {
                cursor += 1;
            }
            if cursor == sep_start || !bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
                break;
            }
            sep = match &bytes[sep_start..cursor] {
                b" " => Sep::Space,
                b"-" => Sep::Dash,
                _ => Sep::Other,
            };
        }
        run
    }
}

/// TCKN, card, or phone, trying the longest group prefix first.
fn match_number(bytes: &[u8], start: usize) -> Option<(PiiKind, usize)> {
    let run = DigitRun::collect(bytes, start);
    for count in (1..=run.group_count).rev() {
        let groups = &run.groups[..count];
        let last = groups[count - 1];
        if !right_boundary(bytes, last.byte_end) {
            continue;
        }
        let digits = &run.digits[..last.digit_end];
        let plain = !run.plus && groups[0].sep_before == Sep::None;
        let kind = if plain && count == 1 && tckn_valid(digits) {
            PiiKind::Tckn
        } else if plain && card_valid(groups, digits) {
            PiiKind::CreditCard
        } else if turkish_phone(run.plus, digits) {
            PiiKind::Phone
        } else {
            continue;
        };
        return Some((kind, last.byte_end));
    }
    None
}

/// Both TCKN check digits; the first digit is non-zero.
pub fn tckn_valid(digits: &[u8]) -> bool {
    if digits.len() != 11 || digits[0] == 0 || digits.iter().any(|&d| d > 9) {
        return false;
    }
    let d = |i: usize| i32::from(digits[i]);
    let odd = d(0) + d(2) + d(4) + d(6) + d(8);
    let even = d(1) + d(3) + d(5) + d(7);
    let tenth = (odd * 7 - even).rem_euclid(10);
    let eleventh = (0..10).map(d).sum::<i32>() % 10;
    d(9) == tenth && d(10) == eleventh
}

/// Luhn checksum over decimal digits.
pub fn luhn_valid(digits: &[u8]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(index, &digit)| {
            let digit = u32::from(digit);
            if index % 2 == 1 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                digit
            }
        })
        .sum();
    !digits.is_empty() && sum % 10 == 0
}

/// 13-19 digits from a real issuer range (2-6), Luhn-valid; separated
/// forms must use one separator (space or dash) in a standard grouping
/// (4-4-4-..., Amex 4-6-5, Diners 4-6-4).
fn card_valid(groups: &[Group], digits: &[u8]) -> bool {
    if !(13..=19).contains(&digits.len()) || !(2..=6).contains(&digits[0]) {
        return false;
    }
    if groups.len() > 1 {
        let sep = groups[1].sep_before;
        if !matches!(sep, Sep::Space | Sep::Dash) || groups[1..].iter().any(|g| g.sep_before != sep)
        {
            return false;
        }
        let mut sizes = [0usize; MAX_GROUPS];
        let mut previous = 0;
        for (size, group) in sizes.iter_mut().zip(groups) {
            *size = group.digit_end - previous;
            previous = group.digit_end;
        }
        let sizes = &sizes[..groups.len()];
        let (last, rest) = sizes.split_last().expect("at least two groups");
        let fours = rest.iter().all(|&size| size == 4) && (1..=4).contains(last);
        if !(fours || sizes == [4, 6, 5] || sizes == [4, 6, 4]) {
            return false;
        }
    }
    luhn_valid(digits)
}

/// `+90` / `0090` / `0` + a 10-digit national number, or a bare 10-digit
/// mobile number. National numbers start with a landline area code
/// (2xx-4xx), a mobile prefix (5xx), or 800/850.
fn turkish_phone(plus: bool, digits: &[u8]) -> bool {
    let national = if plus {
        match digits.strip_prefix(&[9, 0]) {
            Some(rest) => rest,
            None => return false,
        }
    } else if let Some(rest) = digits.strip_prefix(&[0, 0, 9, 0]) {
        rest
    } else if let Some(rest) = digits.strip_prefix(&[0]) {
        rest
    } else if digits.first() == Some(&5) {
        digits
    } else {
        return false;
    };
    national.len() == 10
        && match national[0] {
            2..=5 => true,
            8 => matches!(&national[1..3], [0, 0] | [5, 0]),
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::{PiiKind, detect, luhn_valid, redact, tckn_valid};

    fn kinds(text: &str) -> Vec<(PiiKind, &str)> {
        detect(text)
            .into_iter()
            .map(|span| (span.kind, &text[span.start_byte..span.end_byte]))
            .collect()
    }

    fn digits(text: &str) -> Vec<u8> {
        text.bytes().map(|byte| byte - b'0').collect()
    }

    #[test]
    fn tckn_checksums() {
        assert!(tckn_valid(&digits("10000000146")));
        assert!(!tckn_valid(&digits("10000000147")));
        assert!(!tckn_valid(&digits("10000000156")));
        assert!(!tckn_valid(&digits("01000000146")));
        assert!(!tckn_valid(&digits("1000000014")));
    }

    #[test]
    fn detects_valid_tckn_only() {
        assert_eq!(
            kinds("TC kimlik no: 10000000146."),
            [(PiiKind::Tckn, "10000000146")]
        );
        assert!(kinds("Sipariş 10000000147 kargoda").is_empty());
        assert!(kinds("x10000000146").is_empty());
        assert!(kinds("100000001460").is_empty());
    }

    #[test]
    fn detects_iban_with_and_without_spaces() {
        assert_eq!(
            kinds("IBAN: TR33 0006 1005 1978 6457 8413 26 hesabına"),
            [(PiiKind::Iban, "TR33 0006 1005 1978 6457 8413 26")]
        );
        assert_eq!(
            kinds("TR330006100519786457841326"),
            [(PiiKind::Iban, "TR330006100519786457841326")]
        );
        // Wrong check digits.
        assert!(kinds("TR34 0006 1005 1978 6457 8413 26").is_empty());
        // Too short.
        assert!(kinds("TR33 0006 1005 1978 6457 8413 2").is_empty());
    }

    #[test]
    fn luhn() {
        assert!(luhn_valid(&digits("4111111111111111")));
        assert!(luhn_valid(&digits("378282246310005")));
        assert!(!luhn_valid(&digits("4111111111111112")));
    }

    #[test]
    fn detects_credit_cards() {
        assert_eq!(
            kinds("kart 4111 1111 1111 1111 son"),
            [(PiiKind::CreditCard, "4111 1111 1111 1111")]
        );
        assert_eq!(
            kinds("5500-0000-0000-0004"),
            [(PiiKind::CreditCard, "5500-0000-0000-0004")]
        );
        assert_eq!(
            kinds("amex 3782 822463 10005"),
            [(PiiKind::CreditCard, "3782 822463 10005")]
        );
        assert_eq!(
            kinds("4111111111111111"),
            [(PiiKind::CreditCard, "4111111111111111")]
        );
        assert!(kinds("4111 1111 1111 1112").is_empty());
        // Mixed separators and odd groupings are not card formats.
        assert!(kinds("4111-1111 1111-1111").is_empty());
        assert!(kinds("41111 11111 111111").is_empty());
    }

    #[test]
    fn detects_turkish_phones() {
        for phone in [
            "+90 532 123 45 67",
            "+905321234567",
            "0532 123 45 67",
            "0532-123-45-67",
            "0532.123.45.67",
            "05321234567",
            "0 (532) 123 45 67",
            "(0212) 555 12 34",
            "0212 555 1234",
            "+90 (216) 555-12-34",
            "0090 532 123 45 67",
            "0850 222 33 44",
            "532 123 45 67",
        ] {
            let text = format!("Tel: {phone}, ara.");
            assert_eq!(kinds(&text), [(PiiKind::Phone, phone)], "{phone}");
        }
    }

    #[test]
    fn rejects_non_phone_numbers() {
        for text in [
            "+44 20 7946 0958",
            "0123 456 78 90",
            "212 555 12 34",
            "1923 yılında 1938",
            "0532 123 45 6",
            "Fiyat 1.250,00 TL",
        ] {
            assert!(kinds(text).is_empty(), "{text}: {:?}", kinds(text));
        }
    }

    #[test]
    fn trailing_numbers_do_not_hide_a_phone() {
        assert_eq!(
            kinds("0532 123 45 67 5 adet"),
            [(PiiKind::Phone, "0532 123 45 67")]
        );
    }

    #[test]
    fn detects_emails() {
        assert_eq!(
            kinds("Yazın: ali.veli+test@ornek.com.tr."),
            [(PiiKind::Email, "ali.veli+test@ornek.com.tr")]
        );
        assert!(kinds("@kullanici ve a@b").is_empty());
        assert!(kinds("x@y.c").is_empty());
        assert!(kinds("x@-y.com").is_empty());
    }

    #[test]
    fn detects_ipv4() {
        assert_eq!(
            kinds("sunucu 192.168.1.10 kapalı"),
            [(PiiKind::Ipv4, "192.168.1.10")]
        );
        assert!(kinds("256.1.1.1").is_empty());
        assert!(kinds("1.2.3.4.5").is_empty());
        assert!(kinds("01.2.3.4").is_empty());
        assert!(kinds("sürüm 1.2.3").is_empty());
    }

    #[test]
    fn spans_follow_non_ascii_text() {
        let text = "Şükrü'nün numarası: 0532 123 45 67 ğ";
        let spans = detect(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            &text[spans[0].start_byte..spans[0].end_byte],
            "0532 123 45 67"
        );
        assert!(kinds("çağrı0532 123 45 67").is_empty());
    }

    #[test]
    fn redacts_with_callback() {
        let text = "Ali (10000000146) ali@ornek.com, tel 05321234567.";
        assert_eq!(
            redact(text, PiiKind::placeholder),
            "Ali ([TCKN]) [EMAIL], tel [PHONE]."
        );
        assert_eq!(redact(text, |_| "***"), "Ali (***) ***, tel ***.");
        assert_eq!(redact("temiz metin", |_| "x"), "temiz metin");
    }
}
