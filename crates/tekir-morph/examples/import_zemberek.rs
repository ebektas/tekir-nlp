//! Convert the Zemberek-NLP master dictionary into the tekir lexicon TSV.
//!
//! ```bash
//! cargo run --release -p tekir-morph --example import_zemberek -- \
//!     master-dictionary.dict crates/tekir-morph/data
//! ```
//!
//! Writes `zemberek-nouns.tsv`, `zemberek-adjectives.tsv`,
//! `zemberek-verbs.tsv`, and `zemberek-compounds.tsv` into the output
//! directory.
//!
//! Source data: <https://github.com/ahmetaa/zemberek-nlp>
//! (`morphology/src/main/resources/tr/master-dictionary.dict`), licensed
//! under the Apache License 2.0. The conversion replicates Zemberek's own
//! root-attribute inference (`TurkishDictionaryLoader`): nouns with more
//! than one syllable ending in a stop consonant voice by default unless
//! `NoVoicing` or `InverseHarmony` is present; `nk`/`og` endings always
//! voice; monosyllables default to no voicing. Explicit attributes map as:
//! `Voicing` -> voiced `vowel_stem`, `LastVowelDrop` -> vowel-ellipsis
//! stem, `Doubling` -> doubled stem, `InverseHarmony` -> front
//! `harmony_vowel` override. Only noun entries are imported; see
//! docs/morphology-design.md for what is deferred and why.

use std::{collections::BTreeMap, env, fs, process::ExitCode};

use tekir_morph::rules::is_vowel;

#[derive(Default)]
struct Stats {
    lines: usize,
    nouns: usize,
    adjectives: usize,
    verbs: usize,
    compounds: usize,
    duplicates: usize,
    verb_vowel_drop_plain: usize,
    skipped_non_noun: usize,
    skipped_compound: usize,
    skipped_no_suffix: usize,
    skipped_form: usize,
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(input), Some(output_dir)) = (args.next(), args.next()) else {
        eprintln!("usage: import_zemberek <master-dictionary.dict> <output-dir>");
        return ExitCode::FAILURE;
    };

    let text = match fs::read_to_string(&input) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("failed to read {input}: {error}");
            return ExitCode::FAILURE;
        }
    };

    let mut stats = Stats::default();
    // key: (lemma, vowel_stem, harmony_vowel); value: source-attribute note
    let mut noun_rows: BTreeMap<(String, Option<String>, Option<char>), String> = BTreeMap::new();
    let mut adj_rows: BTreeMap<(String, Option<String>, Option<char>), String> = BTreeMap::new();
    // key: (root, vowel_stem, aorist); value: note
    let mut verb_rows: BTreeMap<(String, Option<String>, &'static str), String> = BTreeMap::new();
    // key: (lemma, bare_stem, vowel_stem); value: note
    let mut compound_rows: BTreeMap<(String, String, Option<String>), String> = BTreeMap::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        stats.lines += 1;

        let (word, meta) = match line.split_once(" [") {
            Some((word, meta)) => (word.trim(), meta.trim_end_matches(']').trim()),
            None => (line, ""),
        };

        if !is_plain_lowercase_word(word) {
            stats.skipped_form += 1;
            continue;
        }

        let mut pos: Option<&str> = None;
        let mut attributes: Vec<&str> = Vec::new();
        let mut roots: Option<&str> = None;
        for section in meta.split(';') {
            let section = section.trim();
            if let Some(value) = section.strip_prefix("P:") {
                pos = value.split(',').next().map(str::trim);
            } else if let Some(value) = section.strip_prefix("A:") {
                attributes.extend(value.split(',').map(str::trim));
            } else if let Some(value) = section.strip_prefix("Roots:") {
                roots = Some(value.trim());
            }
        }

        let is_adjective = pos == Some("Adj");
        match pos {
            Some("Noun") | Some("Adj") => {}
            Some(_) => {
                stats.skipped_non_noun += 1;
                continue;
            }
            None => {
                // Zemberek infers verbs from bare -mak/-mek infinitives;
                // nouns like `yemek` carry an explicit [P:Noun] line.
                if let Some(root) = word
                    .strip_suffix("mak")
                    .or_else(|| word.strip_suffix("mek"))
                {
                    if root.chars().count() < 2 {
                        stats.skipped_form += 1;
                        continue;
                    }
                    let has = |name: &str| attributes.contains(&name);
                    let vowel_stem = has("Voicing")
                        .then(|| voice_final_consonant(root))
                        .flatten();
                    let aorist = if has("Aorist_I") {
                        "i"
                    } else if has("Aorist_A") {
                        "a"
                    } else {
                        "-"
                    };
                    if has("LastVowelDrop") {
                        // The pre-passive vowel drop (`çağrıl-`) is not yet
                        // modeled; the verb is imported without it.
                        stats.verb_vowel_drop_plain += 1;
                    }
                    let mut note = if meta.is_empty() {
                        "plain".to_string()
                    } else {
                        meta.replace('\t', " ")
                    };
                    note.push_str("; infinitive ");
                    note.push_str(word);
                    let key = (root.to_string(), vowel_stem, aorist);
                    match verb_rows.entry(key) {
                        std::collections::btree_map::Entry::Occupied(_) => stats.duplicates += 1,
                        std::collections::btree_map::Entry::Vacant(slot) => {
                            slot.insert(note);
                            stats.verbs += 1;
                        }
                    }
                    continue;
                }
            }
        }

        if attributes.contains(&"CompoundP3sg") {
            // Compound with a lexicalized third-person marker: the bare
            // suffixation stem is the concatenated roots, the pre-vowel
            // stem is the citation form minus the marker.
            let Some(roots) = roots else {
                stats.skipped_compound += 1;
                continue;
            };
            let bare: String = roots.split('-').collect();
            let stripped = strip_p3sg_marker(word);
            let Some(stripped) = stripped else {
                stats.skipped_compound += 1;
                continue;
            };
            let first_root = roots.split('-').next().unwrap_or_default();
            if !bare.starts_with(first_root) || !stripped.starts_with(first_root) {
                stats.skipped_compound += 1;
                continue;
            }
            let vowel_stem = (stripped != bare).then_some(stripped);
            let note = meta.replace('\t', " ");
            let key = (word.to_string(), bare, vowel_stem);
            match compound_rows.entry(key) {
                std::collections::btree_map::Entry::Occupied(_) => stats.duplicates += 1,
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(note);
                    stats.compounds += 1;
                }
            }
            continue;
        }
        if attributes.contains(&"NoSuffix") {
            stats.skipped_no_suffix += 1;
            continue;
        }

        let has = |name: &str| attributes.contains(&name);
        let vowel_count = word.chars().filter(|ch| is_vowel(*ch)).count();
        let last_char = word.chars().next_back().unwrap_or(' ');
        let ends_in_stop = matches!(last_char, 'p' | 'ç' | 't' | 'k');

        let mut voicing = has("Voicing");
        let mut inferred_voicing = false;
        if !voicing && !has("NoVoicing") {
            if vowel_count > 1 && ends_in_stop && !has("InverseHarmony") {
                voicing = true;
                inferred_voicing = true;
            }
            if word.ends_with("nk") || word.ends_with("og") {
                voicing = true;
                inferred_voicing = true;
            }
        }

        let mut stem = word.to_string();
        if has("LastVowelDrop")
            && let Some(dropped) = drop_last_vowel(&stem)
        {
            stem = dropped;
        }
        if voicing && let Some(voiced) = voice_final_consonant(&stem) {
            stem = voiced;
        }
        if has("Doubling")
            && let Some(last) = stem.chars().next_back()
        {
            stem.push(last);
        }
        if word == "su" {
            stem = "suy".to_string();
        }
        let vowel_stem = (stem != word).then_some(stem);

        let harmony_vowel =
            has("InverseHarmony").then(|| match word.chars().rev().find(|ch| is_vowel(*ch)) {
                Some('o' | 'u' | 'ö' | 'ü' | 'û') => 'ü',
                _ => 'i',
            });

        let mut note = if meta.is_empty() {
            "plain".to_string()
        } else {
            meta.replace('\t', " ")
        };
        if inferred_voicing {
            note.push_str("; +Voicing(inferred)");
        }
        if word == "su" {
            note.push_str("; special y-stem");
        }

        let key = (word.to_string(), vowel_stem, harmony_vowel);
        let (rows, counter) = if is_adjective {
            (&mut adj_rows, &mut stats.adjectives)
        } else {
            (&mut noun_rows, &mut stats.nouns)
        };
        match rows.entry(key) {
            std::collections::btree_map::Entry::Occupied(_) => stats.duplicates += 1,
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(note);
                *counter += 1;
            }
        }
    }

    let attribution = "# Imported from the Zemberek-NLP master dictionary.\n\
         # Source: https://github.com/ahmetaa/zemberek-nlp\n\
         #   (morphology/src/main/resources/tr/master-dictionary.dict)\n\
         # Source data license: Apache License 2.0 (c) Zemberek-NLP contributors.\n\
         # Converted by tekir-morph's import_zemberek example; the gloss column\n\
         # preserves the source metadata plus inferred attributes.\n";

    let render_nounlike =
        |directive: &str, rows: &BTreeMap<(String, Option<String>, Option<char>), String>| {
            let mut out = String::with_capacity(rows.len() * 24);
            out.push_str(directive);
            out.push_str(attribution);
            out.push_str("# Columns: lemma\tvowel_stem\tharmony_vowel\tgloss\n");
            for ((lemma, vowel_stem, harmony_vowel), note) in rows {
                out.push_str(lemma);
                out.push('\t');
                out.push_str(vowel_stem.as_deref().unwrap_or("-"));
                out.push('\t');
                match harmony_vowel {
                    Some(vowel) => out.push(*vowel),
                    None => out.push('-'),
                }
                out.push('\t');
                out.push_str(note);
                out.push('\n');
            }
            out
        };

    let mut verbs_out = String::with_capacity(verb_rows.len() * 24);
    verbs_out.push_str("#! pos: verb\n");
    verbs_out.push_str(attribution);
    verbs_out.push_str("# Columns: lemma\tvowel_stem\taorist\tcausative\tgloss\n");
    for ((root, vowel_stem, aorist), note) in &verb_rows {
        verbs_out.push_str(root);
        verbs_out.push('\t');
        verbs_out.push_str(vowel_stem.as_deref().unwrap_or("-"));
        verbs_out.push('\t');
        verbs_out.push_str(aorist);
        verbs_out.push_str("\t-\t");
        verbs_out.push_str(note);
        verbs_out.push('\n');
    }

    let mut compounds_out = String::with_capacity(compound_rows.len() * 32);
    compounds_out.push_str("#! pos: compound-noun\n");
    compounds_out.push_str(attribution);
    compounds_out.push_str("# Columns: lemma\tbare_stem\tvowel_stem\tgloss\n");
    for ((lemma, bare, vowel_stem), note) in &compound_rows {
        compounds_out.push_str(lemma);
        compounds_out.push('\t');
        compounds_out.push_str(bare);
        compounds_out.push('\t');
        compounds_out.push_str(vowel_stem.as_deref().unwrap_or("-"));
        compounds_out.push('\t');
        compounds_out.push_str(note);
        compounds_out.push('\n');
    }

    let outputs = [
        ("zemberek-nouns.tsv", render_nounlike("", &noun_rows)),
        (
            "zemberek-adjectives.tsv",
            render_nounlike("#! pos: adj\n", &adj_rows),
        ),
        ("zemberek-verbs.tsv", verbs_out),
        ("zemberek-compounds.tsv", compounds_out),
    ];
    for (name, content) in outputs {
        let path = format!("{output_dir}/{name}");
        if let Err(error) = fs::write(&path, content) {
            eprintln!("failed to write {path}: {error}");
            return ExitCode::FAILURE;
        }
    }

    println!("input lines:            {:>6}", stats.lines);
    println!("imported noun roots:    {:>6}", stats.nouns);
    println!("imported adjectives:    {:>6}", stats.adjectives);
    println!("imported verb roots:    {:>6}", stats.verbs);
    println!("imported compounds:     {:>6}", stats.compounds);
    println!("  duplicate rows:       {:>6}", stats.duplicates);
    println!(
        "  vowel-drop verbs imported without the drop: {}",
        stats.verb_vowel_drop_plain
    );
    println!("skipped:");
    println!("  other POS:            {:>6}", stats.skipped_non_noun);
    println!("  unmappable compounds: {:>6}", stats.skipped_compound);
    println!("  no-suffix roots:      {:>6}", stats.skipped_no_suffix);
    println!("  proper/format:        {:>6}", stats.skipped_form);
    ExitCode::SUCCESS
}

/// Strip the lexicalized third-person marker: `(s)H` after a vowel.
fn strip_p3sg_marker(lemma: &str) -> Option<String> {
    let mut chars: Vec<char> = lemma.chars().collect();
    let last = *chars.last()?;
    if !is_vowel(last) {
        return None;
    }
    chars.pop();
    if chars.len() >= 2 && *chars.last()? == 's' && is_vowel(chars[chars.len() - 2]) {
        chars.pop();
    }
    Some(chars.into_iter().collect())
}

/// Accept only plain lowercase dictionary words: no proper nouns, spaces,
/// hyphens, apostrophes, or non-alphabetic symbols.
fn is_plain_lowercase_word(word: &str) -> bool {
    !word.is_empty()
        && word.chars().all(|ch| ch.is_alphabetic())
        && word.chars().next().is_some_and(|ch| ch.is_lowercase())
}

fn drop_last_vowel(stem: &str) -> Option<String> {
    let (index, _) = stem.char_indices().rfind(|(_, ch)| is_vowel(*ch))?;
    let mut result = String::with_capacity(stem.len());
    for (char_index, ch) in stem.char_indices() {
        if char_index != index {
            result.push(ch);
        }
    }
    Some(result)
}

/// Zemberek's voicing map: `p -> b`, `ç -> c`, `t -> d`, `k -> ğ`
/// (`g` after `n`, as in `renk -> reng`), `g -> ğ` (for `og` endings).
fn voice_final_consonant(stem: &str) -> Option<String> {
    let mut chars: Vec<char> = stem.chars().collect();
    let last = *chars.last()?;
    let before_last = chars.len().checked_sub(2).map(|index| chars[index]);
    let replacement = match last {
        'p' => 'b',
        'ç' => 'c',
        't' => 'd',
        'k' if before_last == Some('n') => 'g',
        'k' => 'ğ',
        'g' => 'ğ',
        _ => return None,
    };
    *chars.last_mut().expect("stem is non-empty") = replacement;
    Some(chars.into_iter().collect())
}
