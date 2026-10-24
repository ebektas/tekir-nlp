use std::{fmt, io, path::Path};

/// One noun root with its surface alternants and harmony behavior.
///
/// `vowel_stem` is the stem realized before vowel-initial suffixes. That one
/// field encodes final voicing (`kitap`/`kitab`), root vowel ellipsis
/// (`burun`/`burn`), consonant doubling (`hak`/`hakk`), `nk -> ng`
/// (`renk`/`reng`), and y-stems (`su`/`suy`).
///
/// `harmony_vowel` overrides vowel harmony for non-harmonizing loans such as
/// `saat` (-> `saati`) and `alkol` (-> `alkolü`).
/// Root part of speech of a lexicon entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootPos {
    Noun,
    /// Adjective roots share nominal morphotactics via zero nominalization
    /// (`güzeller` = `güzel+Adj^DB+Noun+Zero+A3pl+Pnon+Nom`).
    Adj,
    Verb,
}

/// Lexical aorist allomorph of a verb root. `Rule` applies the default
/// (monosyllabic `+Ar`, polysyllabic `+Hr`, bare `+r` after vowels);
/// `Hr` marks the closed monosyllabic class (`gelir`), `Ar` the
/// polysyllabic `-et` compounds (`affeder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AoristAllomorph {
    Rule,
    Hr,
    Ar,
}

/// Lexical causative allomorph of a verb root. `Rule` applies the default:
/// `+t` after vowels and polysyllabic `l`/`r` stems, `+DHr` otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CausativeAllomorph {
    Rule,
    Hr,
    Ht,
    Ar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexiconEntry {
    pub lemma: String,
    pub bare_stem: String,
    pub vowel_stem: Option<String>,
    pub harmony_vowel: Option<char>,
    pub pos: RootPos,
    pub aorist: AoristAllomorph,
    pub causative: CausativeAllomorph,
    /// Compound noun carrying a lexicalized third-person marker
    /// (`bitpazarı`): inflection requires a possessive.
    pub compound_p3sg: bool,
}

impl LexiconEntry {
    pub fn new(
        lemma: impl Into<String>,
        vowel_stem: Option<String>,
        harmony_vowel: Option<char>,
    ) -> Self {
        let lemma = lemma.into();
        Self {
            bare_stem: lemma.clone(),
            lemma,
            vowel_stem,
            harmony_vowel,
            pos: RootPos::Noun,
            aorist: AoristAllomorph::Rule,
            causative: CausativeAllomorph::Rule,
            compound_p3sg: false,
        }
    }

    pub fn new_verb(
        lemma: impl Into<String>,
        vowel_stem: Option<String>,
        aorist: AoristAllomorph,
        causative: CausativeAllomorph,
    ) -> Self {
        let lemma = lemma.into();
        Self {
            bare_stem: lemma.clone(),
            lemma,
            vowel_stem,
            harmony_vowel: None,
            pos: RootPos::Verb,
            aorist,
            causative,
            compound_p3sg: false,
        }
    }

    /// A compound noun with a lexicalized third-person marker: `lemma` is
    /// the citation form (`bitpazarı`), `bare_stem` the suffixation stem
    /// (`bitpazar`), `vowel_stem` its pre-vowel alternant when different
    /// (`buzdolab`, `aslanağz`).
    pub fn new_compound(
        lemma: impl Into<String>,
        bare_stem: impl Into<String>,
        vowel_stem: Option<String>,
    ) -> Self {
        Self {
            lemma: lemma.into(),
            bare_stem: bare_stem.into(),
            vowel_stem,
            harmony_vowel: None,
            pos: RootPos::Noun,
            aorist: AoristAllomorph::Rule,
            causative: CausativeAllomorph::Rule,
            compound_p3sg: true,
        }
    }

    pub fn stem_for_vowel_initial_suffix(&self) -> &str {
        self.vowel_stem.as_deref().unwrap_or(&self.bare_stem)
    }
}

/// Which stem alternant of an entry a surface match starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemAlternant {
    Bare,
    Vowel,
    /// Vowel-final verb stem with its final vowel elided before `+Hyor`
    /// (`başlıyor` matches via `başl`).
    Dropped,
}

#[derive(Debug)]
pub enum LexiconError {
    Io { path: String, source: io::Error },
    Parse { line: usize, message: String },
}

impl fmt::Display for LexiconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LexiconError::Io { path, source } => {
                write!(f, "failed to read lexicon file {path}: {source}")
            }
            LexiconError::Parse { line, message } => {
                write!(f, "invalid lexicon entry on line {line}: {message}")
            }
        }
    }
}

impl std::error::Error for LexiconError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LexiconError::Io { source, .. } => Some(source),
            LexiconError::Parse { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
struct StemIndexEntry {
    stem: Box<str>,
    entry: u32,
    alternant: StemAlternant,
}

/// Immutable root lexicon plus a sorted stem index for surface-driven lookup.
///
/// Built once, then shared read-only across threads (`Lexicon` is `Send +
/// Sync`); analysis never takes a lock. The index is a contiguous sorted
/// array probed by binary search, chosen for cache locality; it can be
/// swapped for a trie or FST behind the same lookup API when root counts
/// grow.
#[derive(Debug, Clone)]
pub struct Lexicon {
    entries: Vec<LexiconEntry>,
    stem_index: Vec<StemIndexEntry>,
}

const SEED_NOUNS_TSV: &str = include_str!("../data/nouns.tsv");
const SEED_ADJECTIVES_TSV: &str = include_str!("../data/adjectives.tsv");
const SEED_COMPOUNDS_TSV: &str = include_str!("../data/compounds.tsv");
const SEED_VERBS_TSV: &str = include_str!("../data/verbs.tsv");
const ZEMBEREK_NOUNS_TSV: &str = include_str!("../data/zemberek-nouns.tsv");
const ZEMBEREK_ADJECTIVES_TSV: &str = include_str!("../data/zemberek-adjectives.tsv");
const ZEMBEREK_VERBS_TSV: &str = include_str!("../data/zemberek-verbs.tsv");
const ZEMBEREK_COMPOUNDS_TSV: &str = include_str!("../data/zemberek-compounds.tsv");

/// File kind selected by the optional `#! pos: ...` directive; plain noun
/// files need no directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TsvKind {
    Noun,
    Adj,
    Verb,
    CompoundNoun,
}

impl Lexicon {
    /// The built-in seed lexicon embedded in the crate.
    pub fn seed() -> Self {
        let mut entries = Vec::new();
        for text in [SEED_NOUNS_TSV, SEED_ADJECTIVES_TSV, SEED_COMPOUNDS_TSV] {
            entries.extend(parse_tsv_entries(text).expect("embedded seed lexicon must parse"));
        }
        entries.extend(seed_verb_entries());
        Self::from_entries(entries)
    }

    /// The full embedded lexicon: the imported Zemberek noun, adjective,
    /// verb, and compound files (26,798 roots).
    pub fn full() -> Self {
        let mut entries = Vec::new();
        for text in [
            ZEMBEREK_NOUNS_TSV,
            ZEMBEREK_ADJECTIVES_TSV,
            ZEMBEREK_VERBS_TSV,
            ZEMBEREK_COMPOUNDS_TSV,
        ] {
            entries.extend(parse_tsv_entries(text).expect("embedded zemberek lexicon must parse"));
        }
        Self::from_entries(entries)
    }

    /// Load and merge several lexicon TSV files. Seed verb roots are
    /// appended unless one of the files provides verbs itself.
    pub fn from_tsv_paths<I, P>(paths: I) -> Result<Self, LexiconError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut entries = Vec::new();
        for path in paths {
            let path = path.as_ref();
            let text = std::fs::read_to_string(path).map_err(|source| LexiconError::Io {
                path: path.display().to_string(),
                source,
            })?;
            entries.extend(parse_tsv_entries(&text)?);
        }
        if !entries.iter().any(|entry| entry.pos == RootPos::Verb) {
            entries.extend(seed_verb_entries());
        }
        Ok(Self::from_entries(entries))
    }

    pub fn from_tsv_path(path: impl AsRef<Path>) -> Result<Self, LexiconError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| LexiconError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_tsv_str(&text)
    }

    /// Parse one tab-separated lexicon file (kind selected by its
    /// `#! pos: ...` directive; plain noun format without one). Seed verb
    /// roots are appended unless the file provides verbs itself.
    pub fn from_tsv_str(text: &str) -> Result<Self, LexiconError> {
        let mut entries = parse_tsv_entries(text)?;
        if !entries.iter().any(|entry| entry.pos == RootPos::Verb) {
            entries.extend(seed_verb_entries());
        }
        Ok(Self::from_entries(entries))
    }

    fn parse_noun_like(text: &str, pos: RootPos) -> Result<Vec<LexiconEntry>, LexiconError> {
        let mut entries = Vec::new();

        for (line_idx, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut columns = line.split('\t');
            let lemma = columns.next().unwrap_or_default().trim();
            let vowel_stem = optional_column(columns.next());
            let harmony = optional_column(columns.next());

            if lemma.is_empty() {
                return Err(LexiconError::Parse {
                    line: line_idx + 1,
                    message: "lemma column is empty".to_string(),
                });
            }

            let harmony_vowel = match harmony {
                None => None,
                Some(value) => {
                    let mut chars = value.chars();
                    let vowel = chars.next();
                    match (vowel, chars.next()) {
                        (Some(vowel), None) if crate::rules::is_vowel(vowel) => Some(vowel),
                        _ => {
                            return Err(LexiconError::Parse {
                                line: line_idx + 1,
                                message: format!(
                                    "harmony_vowel must be a single Turkish vowel, got {value:?}"
                                ),
                            });
                        }
                    }
                }
            };

            if let Some(stem) = vowel_stem
                && stem == lemma
            {
                return Err(LexiconError::Parse {
                    line: line_idx + 1,
                    message: "vowel_stem equal to lemma should be written as \"-\"".to_string(),
                });
            }

            let mut entry = LexiconEntry::new(lemma, vowel_stem.map(str::to_string), harmony_vowel);
            entry.pos = pos;
            entries.push(entry);
        }

        Ok(entries)
    }

    pub fn from_entries(entries: Vec<LexiconEntry>) -> Self {
        let mut stem_index = Vec::with_capacity(entries.len() * 2);

        for (idx, entry) in entries.iter().enumerate() {
            let entry_id = u32::try_from(idx).expect("lexicon entry count must fit in u32");
            stem_index.push(StemIndexEntry {
                stem: entry.bare_stem.as_str().into(),
                entry: entry_id,
                alternant: StemAlternant::Bare,
            });
            if let Some(vowel_stem) = &entry.vowel_stem {
                stem_index.push(StemIndexEntry {
                    stem: vowel_stem.as_str().into(),
                    entry: entry_id,
                    alternant: StemAlternant::Vowel,
                });
            }
            if entry.pos == RootPos::Verb
                && entry
                    .bare_stem
                    .chars()
                    .next_back()
                    .is_some_and(crate::rules::is_vowel)
            {
                let mut dropped = entry.bare_stem.clone();
                dropped.pop();
                if !dropped.is_empty() {
                    stem_index.push(StemIndexEntry {
                        stem: dropped.into(),
                        entry: entry_id,
                        alternant: StemAlternant::Dropped,
                    });
                }
            }
        }

        stem_index.sort_by(|a, b| a.stem.cmp(&b.stem));

        Self {
            entries,
            stem_index,
        }
    }

    pub fn entries(&self) -> &[LexiconEntry] {
        &self.entries
    }

    pub fn entry(&self, index: u32) -> &LexiconEntry {
        &self.entries[index as usize]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entry_by_lemma(&self, lemma: &str) -> Option<&LexiconEntry> {
        self.entries.iter().find(|entry| entry.lemma == lemma)
    }

    /// Invoke `visit` for every indexed stem that is a prefix of `surface`,
    /// passing the entry id, the matched alternant, and the matched byte
    /// length.
    ///
    /// Allocation-free virtual-trie descent over the sorted index: stems
    /// sharing a prefix are contiguous, so one `[lo, hi)` range narrows as
    /// the prefix extends (each step binary-searches only the surviving
    /// range), and the scan stops as soon as no indexed stem can extend the
    /// current prefix — for most inputs after a handful of characters.
    pub fn for_each_stem_prefix(
        &self,
        surface: &str,
        mut visit: impl FnMut(u32, StemAlternant, usize),
    ) {
        let boundaries = surface
            .char_indices()
            .map(|(offset, _)| offset)
            .skip(1)
            .chain(std::iter::once(surface.len()));

        let mut low = 0;
        let high = self.stem_index.len();

        for end in boundaries {
            let prefix = &surface[..end];
            low += self.stem_index[low..high]
                .partition_point(|indexed| indexed.stem.as_ref() < prefix);
            if low >= high {
                return;
            }

            for indexed in &self.stem_index[low..high] {
                if indexed.stem.as_ref() != prefix {
                    break;
                }
                visit(indexed.entry, indexed.alternant, end);
            }

            if !self.stem_index[low].stem.starts_with(prefix) {
                return;
            }
        }
    }
}

fn optional_column(column: Option<&str>) -> Option<&str> {
    match column.map(str::trim) {
        None | Some("") | Some("-") => None,
        Some(value) => Some(value),
    }
}

/// Dispatch on the optional `#! pos: ...` directive and parse one file.
fn parse_tsv_entries(text: &str) -> Result<Vec<LexiconEntry>, LexiconError> {
    let mut kind = TsvKind::Noun;
    for line in text.lines() {
        let line = line.trim();
        if let Some(directive) = line.strip_prefix("#!") {
            kind = match directive.trim().strip_prefix("pos:").map(str::trim) {
                Some("noun") | None => TsvKind::Noun,
                Some("adj") => TsvKind::Adj,
                Some("verb") => TsvKind::Verb,
                Some("compound-noun") => TsvKind::CompoundNoun,
                Some(other) => {
                    return Err(LexiconError::Parse {
                        line: 1,
                        message: format!("unknown pos directive {other:?}"),
                    });
                }
            };
            break;
        }
        if !line.is_empty() && !line.starts_with('#') {
            break;
        }
    }

    match kind {
        TsvKind::Noun => Lexicon::parse_noun_like(text, RootPos::Noun),
        TsvKind::Adj => Lexicon::parse_noun_like(text, RootPos::Adj),
        TsvKind::Verb => parse_verb_tsv(text),
        TsvKind::CompoundNoun => parse_compound_tsv(text),
    }
}

/// Verb TSV: `lemma  vowel_stem  aorist(-|i|a)  causative(-|ir|it|ar)  gloss`.
fn parse_verb_tsv(text: &str) -> Result<Vec<LexiconEntry>, LexiconError> {
    let mut entries = Vec::new();

    for (line_idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut columns = line.split('\t');
        let lemma = columns.next().unwrap_or_default().trim();
        let vowel_stem = optional_column(columns.next());
        let aorist = optional_column(columns.next());
        let causative = optional_column(columns.next());

        if lemma.is_empty() {
            return Err(LexiconError::Parse {
                line: line_idx + 1,
                message: "lemma column is empty".to_string(),
            });
        }
        let aorist = match aorist {
            None => AoristAllomorph::Rule,
            Some("i") => AoristAllomorph::Hr,
            Some("a") => AoristAllomorph::Ar,
            Some(other) => {
                return Err(LexiconError::Parse {
                    line: line_idx + 1,
                    message: format!("aorist must be -, i, or a, got {other:?}"),
                });
            }
        };
        let causative = match causative {
            None => CausativeAllomorph::Rule,
            Some("ir") => CausativeAllomorph::Hr,
            Some("it") => CausativeAllomorph::Ht,
            Some("ar") => CausativeAllomorph::Ar,
            Some(other) => {
                return Err(LexiconError::Parse {
                    line: line_idx + 1,
                    message: format!("causative must be -, ir, it, or ar, got {other:?}"),
                });
            }
        };

        entries.push(LexiconEntry::new_verb(
            lemma,
            vowel_stem.map(str::to_string),
            aorist,
            causative,
        ));
    }

    Ok(entries)
}

/// Compound TSV: `lemma  bare_stem  vowel_stem  gloss`.
fn parse_compound_tsv(text: &str) -> Result<Vec<LexiconEntry>, LexiconError> {
    let mut entries = Vec::new();

    for (line_idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut columns = line.split('\t');
        let lemma = columns.next().unwrap_or_default().trim();
        let bare_stem = optional_column(columns.next());
        let vowel_stem = optional_column(columns.next());

        let Some(bare_stem) = bare_stem else {
            return Err(LexiconError::Parse {
                line: line_idx + 1,
                message: "compound entries need an explicit bare_stem".to_string(),
            });
        };
        if lemma.is_empty() {
            return Err(LexiconError::Parse {
                line: line_idx + 1,
                message: "lemma column is empty".to_string(),
            });
        }

        entries.push(LexiconEntry::new_compound(
            lemma,
            bare_stem,
            vowel_stem.map(str::to_string),
        ));
    }

    Ok(entries)
}

/// Parse the embedded seed verb lexicon (`data/verbs.tsv`).
fn seed_verb_entries() -> Vec<LexiconEntry> {
    parse_verb_tsv(SEED_VERBS_TSV).expect("embedded seed verbs must parse")
}

#[cfg(test)]
mod tests {
    use crate::lexicon::{Lexicon, LexiconError, StemAlternant};

    #[test]
    fn seed_lexicon_parses_and_indexes_alternants() {
        let lexicon = Lexicon::seed();

        assert!(lexicon.len() >= 30);

        let kitap = lexicon.entry_by_lemma("kitap").expect("kitap should exist");
        assert_eq!(kitap.vowel_stem.as_deref(), Some("kitab"));

        let saat = lexicon.entry_by_lemma("saat").expect("saat should exist");
        assert_eq!(saat.harmony_vowel, Some('i'));
    }

    #[test]
    fn stem_prefix_lookup_finds_matching_alternants_only() {
        let lexicon = Lexicon::seed();

        let mut hits = Vec::new();
        lexicon.for_each_stem_prefix("kitaba", |entry, alternant, len| {
            hits.push((lexicon.entry(entry).lemma.clone(), alternant, len));
        });

        assert!(hits.iter().any(|(lemma, alternant, len)| {
            lemma == "kitap" && *alternant == StemAlternant::Vowel && *len == "kitab".len()
        }));
        assert!(
            !hits
                .iter()
                .any(|(_, alternant, _)| *alternant == StemAlternant::Bare),
            "bare stem kitap is not a prefix of kitaba"
        );
    }

    #[test]
    fn homograph_lemmas_keep_separate_entries() {
        let lexicon = Lexicon::seed();

        let entries: Vec<_> = lexicon
            .entries()
            .iter()
            .filter(|entry| entry.lemma == "şık")
            .collect();

        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn rejects_invalid_harmony_vowel() {
        let error = Lexicon::from_tsv_str("ev\t-\tx\tbad").expect_err("x is not a vowel");
        assert!(matches!(error, LexiconError::Parse { line: 1, .. }));
    }

    #[test]
    fn rejects_redundant_vowel_stem() {
        let error = Lexicon::from_tsv_str("ev\tev\t-\tbad").expect_err("redundant stem");
        assert!(matches!(error, LexiconError::Parse { line: 1, .. }));
    }
}
