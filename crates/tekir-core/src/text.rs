/// Turkish-aware lowercasing (`İ -> i`, `I -> ı`).
pub fn turkish_lowercase(input: &str) -> String {
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

/// Whether `input` is already lowercase in the Turkish sense, so
/// [`turkish_lowercase`] would return it unchanged.
pub fn is_turkish_lowercase(input: &str) -> bool {
    input.chars().all(|ch| !ch.is_uppercase() && ch != 'İ')
}

#[cfg(test)]
mod tests {
    use super::{is_turkish_lowercase, turkish_lowercase};

    #[test]
    fn lowercases_turkish_dotted_and_dotless() {
        assert_eq!(turkish_lowercase("İSTANBUL'DA"), "istanbul'da");
        assert_eq!(turkish_lowercase("IŞIK"), "ışık");
        assert!(is_turkish_lowercase("ışık"));
        assert!(!is_turkish_lowercase("Işık"));
    }
}
