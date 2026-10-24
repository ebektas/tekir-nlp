//! Lossless pre-segmentation of raw text into pieces, GPT-style: words,
//! digit groups, punctuation runs, and whitespace. Pieces are contiguous
//! byte ranges that cover the input exactly, so joining them restores it.
//!
//! A single space directly before a word, number, or punctuation run is
//! attached to the front of that piece (`" kitap"`); any other whitespace
//! is a piece of its own. Digit runs split into groups of at most three.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieceKind {
    /// Letters, optionally joined by inner apostrophes (`Ankara'da`,
    /// `1990'larda` starts as a number and continues as a word).
    Word,
    Number,
    Punct,
    Space,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub range: Range<usize>,
    pub kind: PieceKind,
    /// Byte length of the attached leading space (0 or 1).
    pub prefix: usize,
}

impl Piece {
    /// The piece's content without its attached leading space.
    pub fn content(&self) -> Range<usize> {
        self.range.start + self.prefix..self.range.end
    }
}

fn is_apostrophe(ch: char) -> bool {
    matches!(ch, '\'' | '’')
}

fn is_letter(ch: char) -> bool {
    ch.is_alphabetic()
}

/// Split `text` into pieces.
pub fn scan(text: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let end_of = |i: usize| chars.get(i).map_or(text.len(), |&(offset, _)| offset);
    let mut i = 0;

    while i < chars.len() {
        let (start, ch) = chars[i];

        if ch.is_whitespace() {
            let mut j = i;
            while j < chars.len() && chars[j].1.is_whitespace() {
                j += 1;
            }
            // Leave a final plain space for the next piece to absorb.
            let absorb = j < chars.len() && chars[j - 1].1 == ' ';
            let space_end = if absorb { j - 1 } else { j };
            if space_end > i {
                pieces.push(Piece {
                    range: start..end_of(space_end),
                    kind: PieceKind::Space,
                    prefix: 0,
                });
            }
            i = space_end;
            if absorb {
                let (kind, next) = scan_content(&chars, i + 1);
                push_content(&mut pieces, text, &chars, i, i + 1, next, kind, 1);
                i = next;
            }
            continue;
        }

        let (kind, next) = scan_content(&chars, i);
        push_content(&mut pieces, text, &chars, i, i, next, kind, 0);
        i = next;
    }

    pieces
}

/// Classify and measure the non-space run starting at char `i`.
fn scan_content(chars: &[(usize, char)], i: usize) -> (PieceKind, usize) {
    let ch = chars[i].1;
    let mut j = i + 1;
    let kind = if is_letter(ch) || ch.is_numeric() {
        let starts_numeric = ch.is_numeric();
        let mut word = !starts_numeric;
        loop {
            match chars.get(j).map(|&(_, c)| c) {
                Some(c) if is_letter(c) || (c.is_numeric() && !word) => j += 1,
                Some(c) if is_letter(c) && starts_numeric => {
                    word = true;
                    j += 1;
                }
                Some(c)
                    if is_apostrophe(c) && chars.get(j + 1).is_some_and(|&(_, n)| is_letter(n)) =>
                {
                    word = true;
                    j += 2;
                }
                _ => break,
            }
        }
        if word {
            PieceKind::Word
        } else {
            PieceKind::Number
        }
    } else {
        while chars
            .get(j)
            .is_some_and(|&(_, c)| !c.is_whitespace() && !is_letter(c) && !c.is_numeric())
        {
            j += 1;
        }
        PieceKind::Punct
    };
    (kind, j)
}

#[allow(clippy::too_many_arguments)]
fn push_content(
    pieces: &mut Vec<Piece>,
    text: &str,
    chars: &[(usize, char)],
    piece_start: usize,
    content_start: usize,
    end: usize,
    kind: PieceKind,
    prefix: usize,
) {
    let end_of = |i: usize| chars.get(i).map_or(text.len(), |&(offset, _)| offset);
    if kind != PieceKind::Number {
        pieces.push(Piece {
            range: chars[piece_start].0..end_of(end),
            kind,
            prefix,
        });
        return;
    }
    // Digit groups of at most three, the first carrying the prefix.
    let mut group_start = piece_start;
    let mut i = content_start;
    while i < end {
        let group_end = (i + 3).min(end);
        pieces.push(Piece {
            range: chars[group_start].0..end_of(group_end),
            kind,
            prefix: if group_start == piece_start {
                prefix
            } else {
                0
            },
        });
        i = group_end;
        group_start = group_end;
    }
}

#[cfg(test)]
mod tests {
    use super::{PieceKind, scan};

    fn texts(text: &str) -> Vec<&str> {
        scan(text)
            .iter()
            .map(|piece| &text[piece.range.clone()])
            .collect()
    }

    #[test]
    fn scans_words_spaces_numbers_and_punctuation() {
        assert_eq!(
            texts("Ankara'dakiler 1990'larda geldi, 12345 kişi!\n\nSon"),
            vec![
                "Ankara'dakiler",
                " 1990'larda",
                " geldi",
                ",",
                " 123",
                "45",
                " kişi",
                "!",
                "\n\n",
                "Son"
            ]
        );
    }

    #[test]
    fn is_lossless_and_marks_prefixes() {
        for text in ["  çift  boşluk ", "\tsekme\n", "a", "", "...", "x  'y' z"] {
            let pieces = scan(text);
            assert_eq!(
                pieces
                    .iter()
                    .map(|p| &text[p.range.clone()])
                    .collect::<String>(),
                text
            );
            for piece in &pieces {
                if piece.prefix == 1 {
                    assert!(text[piece.range.clone()].starts_with(' '));
                }
            }
        }
        let pieces = scan(" kitap");
        assert_eq!(pieces[0].kind, PieceKind::Word);
        assert_eq!(pieces[0].prefix, 1);
    }
}
