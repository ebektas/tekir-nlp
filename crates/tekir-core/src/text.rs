/// Turkish-aware lowercasing (`İ -> i`, `I -> ı`).
pub fn turkish_lowercase(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        for_each_turkish_lower(ch, |lower| output.push(lower));
    }
    output
}

/// Call `f` with each char of the Turkish lowercase form of `ch`, without
/// allocating (for hashing and other hot loops).
#[inline]
pub fn for_each_turkish_lower(ch: char, mut f: impl FnMut(char)) {
    match ch {
        'I' => f('ı'),
        'İ' => f('i'),
        _ if ch.is_ascii() => f(ch.to_ascii_lowercase()),
        _ => ch.to_lowercase().for_each(f),
    }
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
