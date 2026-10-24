//! Minimal CoNLL-U reader: forms, lemmas, UPOS, and morphological features.

#[derive(Debug, Clone)]
pub struct Token {
    pub form: String,
    pub lemma: String,
    pub upos: String,
    pub feats: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct Sentence {
    pub tokens: Vec<Token>,
}

impl Token {
    pub fn feat(&self, name: &str) -> Option<&str> {
        self.feats
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Parse CoNLL-U text. Multiword-token ranges and empty nodes are skipped
/// (the Turkish Kenet treebank contains neither).
pub fn parse(text: &str) -> Vec<Sentence> {
    let mut sentences = Vec::new();
    let mut current = Sentence::default();

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if !current.tokens.is_empty() {
                sentences.push(std::mem::take(&mut current));
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let mut columns = line.split('\t');
        let id = columns.next().unwrap_or_default();
        if id.contains('-') || id.contains('.') {
            continue;
        }
        let form = columns.next().unwrap_or_default();
        let lemma = columns.next().unwrap_or_default();
        let upos = columns.next().unwrap_or_default();
        let _xpos = columns.next();
        let feats_column = columns.next().unwrap_or("_");

        let feats = if feats_column == "_" {
            Vec::new()
        } else {
            feats_column
                .split('|')
                .filter_map(|pair| {
                    pair.split_once('=')
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                })
                .collect()
        };

        current.tokens.push(Token {
            form: form.to_string(),
            lemma: lemma.to_string(),
            upos: upos.to_string(),
            feats,
        });
    }
    if !current.tokens.is_empty() {
        sentences.push(current);
    }
    sentences
}

pub use tekir_core::turkish_lowercase;

#[cfg(test)]
mod tests {
    #[test]
    fn parses_sentences_and_feats() {
        let text = "# text = Ev .\n1\tEvde\tev\tNOUN\t_\tCase=Loc|Number=Sing\t0\troot\t_\t_\n2\t.\t.\tPUNCT\t_\t_\t1\tpunct\t_\t_\n\n";
        let sentences = super::parse(text);
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].tokens.len(), 2);
        assert_eq!(sentences[0].tokens[0].lemma, "ev");
        assert_eq!(sentences[0].tokens[0].feat("Case"), Some("Loc"));
    }
}
