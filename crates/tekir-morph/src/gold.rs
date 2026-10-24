//! Shared gold-standard cases for analysis, generation, and preference.
//!
//! Entries are referenced by lemma and resolved against the seed lexicon at
//! test time. Analysis cases assert containment (the surface may have more
//! readings); generation cases assert the exact surface.

use crate::state::{Case, Number, Possessive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldAnalysisCase {
    pub surface: &'static str,
    pub expected_gloss: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldGenerationCase {
    pub lemma: &'static str,
    pub number: Number,
    pub possessive: Possessive,
    pub case: Case,
    pub expected_surface: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldPreferenceCase {
    pub surface: &'static str,
    pub expected_best_gloss: &'static str,
}

pub const GOLD_ANALYSIS_CASES: &[GoldAnalysisCase] = &[
    GoldAnalysisCase {
        surface: "evi",
        expected_gloss: "ev+Noun+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "eve",
        expected_gloss: "ev+Noun+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "evde",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc",
    },
    GoldAnalysisCase {
        surface: "evden",
        expected_gloss: "ev+Noun+A3sg+Pnon+Abl",
    },
    GoldAnalysisCase {
        surface: "evler",
        expected_gloss: "ev+Noun+A3pl+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "evlere",
        expected_gloss: "ev+Noun+A3pl+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "evlerin",
        expected_gloss: "ev+Noun+A3pl+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "evimiz",
        expected_gloss: "ev+Noun+A3sg+P1pl+Nom",
    },
    GoldAnalysisCase {
        surface: "eviniz",
        expected_gloss: "ev+Noun+A3sg+P2pl+Nom",
    },
    GoldAnalysisCase {
        surface: "evinizde",
        expected_gloss: "ev+Noun+A3sg+P2pl+Loc",
    },
    GoldAnalysisCase {
        surface: "evine",
        expected_gloss: "ev+Noun+A3sg+P3sg+Dat",
    },
    GoldAnalysisCase {
        surface: "evinde",
        expected_gloss: "ev+Noun+A3sg+P3sg+Loc",
    },
    GoldAnalysisCase {
        surface: "evinin",
        expected_gloss: "ev+Noun+A3sg+P3sg+Gen",
    },
    GoldAnalysisCase {
        surface: "ağacı",
        expected_gloss: "ağaç+Noun+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "ağaca",
        expected_gloss: "ağaç+Noun+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "ağaçta",
        expected_gloss: "ağaç+Noun+A3sg+Pnon+Loc",
    },
    GoldAnalysisCase {
        surface: "ağaçtan",
        expected_gloss: "ağaç+Noun+A3sg+Pnon+Abl",
    },
    GoldAnalysisCase {
        surface: "ağaçların",
        expected_gloss: "ağaç+Noun+A3pl+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "ağaçları",
        expected_gloss: "ağaç+Noun+A3pl+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "çocuğa",
        expected_gloss: "çocuk+Noun+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "çocuğun",
        expected_gloss: "çocuk+Noun+A3sg+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "çocukta",
        expected_gloss: "çocuk+Noun+A3sg+Pnon+Loc",
    },
    GoldAnalysisCase {
        surface: "çocuktan",
        expected_gloss: "çocuk+Noun+A3sg+Pnon+Abl",
    },
    GoldAnalysisCase {
        surface: "çocukları",
        expected_gloss: "çocuk+Noun+A3sg+P3pl+Nom",
    },
    GoldAnalysisCase {
        surface: "çocuklarımız",
        expected_gloss: "çocuk+Noun+A3pl+P1pl+Nom",
    },
    GoldAnalysisCase {
        surface: "çocuklarının",
        expected_gloss: "çocuk+Noun+A3pl+P3pl+Gen",
    },
    GoldAnalysisCase {
        surface: "kitabı",
        expected_gloss: "kitap+Noun+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "kitaba",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "kitapta",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Loc",
    },
    GoldAnalysisCase {
        surface: "kitaptan",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Abl",
    },
    GoldAnalysisCase {
        surface: "kitabın",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "kitabımız",
        expected_gloss: "kitap+Noun+A3sg+P1pl+Nom",
    },
    GoldAnalysisCase {
        surface: "kitaplarımız",
        expected_gloss: "kitap+Noun+A3pl+P1pl+Nom",
    },
    GoldAnalysisCase {
        surface: "kitaplarınız",
        expected_gloss: "kitap+Noun+A3pl+P2pl+Nom",
    },
    GoldAnalysisCase {
        surface: "kitapları",
        expected_gloss: "kitap+Noun+A3sg+P3pl+Nom",
    },
    GoldAnalysisCase {
        surface: "burnu",
        expected_gloss: "burun+Noun+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "burnumuzda",
        expected_gloss: "burun+Noun+A3sg+P1pl+Loc",
    },
    GoldAnalysisCase {
        surface: "burunda",
        expected_gloss: "burun+Noun+A3sg+Pnon+Loc",
    },
    GoldAnalysisCase {
        surface: "ağzım",
        expected_gloss: "ağız+Noun+A3sg+P1sg+Nom",
    },
    GoldAnalysisCase {
        surface: "şehrin",
        expected_gloss: "şehir+Noun+A3sg+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "vakti",
        expected_gloss: "vakit+Noun+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "hakkı",
        expected_gloss: "hak+Noun+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "haklar",
        expected_gloss: "hak+Noun+A3pl+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "tıbba",
        expected_gloss: "tıp+Noun+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "suyu",
        expected_gloss: "su+Noun+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "suyun",
        expected_gloss: "su+Noun+A3sg+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "sudan",
        expected_gloss: "su+Noun+A3sg+Pnon+Abl",
    },
    GoldAnalysisCase {
        surface: "saati",
        expected_gloss: "saat+Noun+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "saatler",
        expected_gloss: "saat+Noun+A3pl+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "saatte",
        expected_gloss: "saat+Noun+A3sg+Pnon+Loc",
    },
    GoldAnalysisCase {
        surface: "alkolü",
        expected_gloss: "alkol+Noun+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "rengi",
        expected_gloss: "renk+Noun+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "kalbinde",
        expected_gloss: "kalp+Noun+A3sg+P3sg+Loc",
    },
    GoldAnalysisCase {
        surface: "masasında",
        expected_gloss: "masa+Noun+A3sg+P3sg+Loc",
    },
    GoldAnalysisCase {
        surface: "annesinin",
        expected_gloss: "anne+Noun+A3sg+P3sg+Gen",
    },
    GoldAnalysisCase {
        surface: "gözlerinde",
        expected_gloss: "göz+Noun+A3pl+P3sg+Loc",
    },
    GoldAnalysisCase {
        surface: "kitapçı",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Nom^DB+Noun+Agt+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "kapıcı",
        expected_gloss: "kapı+Noun+A3sg+Pnon+Nom^DB+Noun+Agt+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "denizcilik",
        expected_gloss: "deniz+Noun+A3sg+Pnon+Nom^DB+Noun+Agt+A3sg+Pnon+Nom^DB+Noun+Ness+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "kitaplığa",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Nom^DB+Noun+Ness+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "gözlüğü",
        expected_gloss: "göz+Noun+A3sg+Pnon+Nom^DB+Noun+Ness+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "evcik",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Noun+Dim+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "ağaççık",
        expected_gloss: "ağaç+Noun+A3sg+Pnon+Nom^DB+Noun+Dim+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "evsiz",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Adj+Without",
    },
    GoldAnalysisCase {
        surface: "susuz",
        expected_gloss: "su+Noun+A3sg+Pnon+Nom^DB+Adj+Without",
    },
    GoldAnalysisCase {
        surface: "evli",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Adj+With",
    },
    GoldAnalysisCase {
        surface: "sulu",
        expected_gloss: "su+Noun+A3sg+Pnon+Nom^DB+Adj+With",
    },
    GoldAnalysisCase {
        surface: "evsizlik",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Adj+Without^DB+Noun+Ness+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "evsizlere",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Adj+Without^DB+Noun+Zero+A3pl+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "evdeki",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel",
    },
    GoldAnalysisCase {
        surface: "evinki",
        expected_gloss: "ev+Noun+A3sg+Pnon+Gen^DB+Adj+Rel",
    },
    GoldAnalysisCase {
        surface: "evimdekiler",
        expected_gloss: "ev+Noun+A3sg+P1sg+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "evdekine",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3sg+Pnon+Dat",
    },
    GoldAnalysisCase {
        surface: "masadakilerin",
        expected_gloss: "masa+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Gen",
    },
    GoldAnalysisCase {
        surface: "masadakilerinki",
        expected_gloss: "masa+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Noun+Zero+A3pl+Pnon+Gen^DB+Adj+Rel",
    },
    GoldAnalysisCase {
        surface: "evdeydi",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "evdeymiş",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Narr+A3sg",
    },
    GoldAnalysisCase {
        surface: "evdeyken",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero^DB+Adverb+While",
    },
    GoldAnalysisCase {
        surface: "evdeymişçesine",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Narr^DB+Adverb+AsIf",
    },
    GoldAnalysisCase {
        surface: "evdeyim",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Pres+A1sg",
    },
    GoldAnalysisCase {
        surface: "evdesiniz",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Pres+A2pl",
    },
    GoldAnalysisCase {
        surface: "evdeler",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Pres+A3pl",
    },
    GoldAnalysisCase {
        surface: "evdedirler",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Pres+A3pl+Cop",
    },
    GoldAnalysisCase {
        surface: "okuldaydık",
        expected_gloss: "okul+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Past+A1pl",
    },
    GoldAnalysisCase {
        surface: "kitaptı",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Nom^DB+Verb+Zero+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "kitaptır",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Nom^DB+Verb+Zero+Pres+A3sg+Cop",
    },
    GoldAnalysisCase {
        surface: "evdeyseniz",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Cond+A2pl",
    },
    GoldAnalysisCase {
        surface: "evsizdi",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Adj+Without^DB+Verb+Zero+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "evdekiydi",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Adj+Rel^DB+Verb+Zero+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "sudaymış",
        expected_gloss: "su+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Narr+A3sg",
    },
    GoldAnalysisCase {
        surface: "evimdeydin",
        expected_gloss: "ev+Noun+A3sg+P1sg+Loc^DB+Verb+Zero+Past+A2sg",
    },
    GoldAnalysisCase {
        surface: "evdeymiştir",
        expected_gloss: "ev+Noun+A3sg+Pnon+Loc^DB+Verb+Zero+Narr+A3sg+Cop",
    },
    GoldAnalysisCase {
        surface: "yaptım",
        expected_gloss: "yap+Verb+Pos+Past+A1sg",
    },
    GoldAnalysisCase {
        surface: "gidiyor",
        expected_gloss: "git+Verb+Pos+Prog1+A3sg",
    },
    GoldAnalysisCase {
        surface: "başlıyor",
        expected_gloss: "başla+Verb+Pos+Prog1+A3sg",
    },
    GoldAnalysisCase {
        surface: "yapmıyor",
        expected_gloss: "yap+Verb+Neg+Prog1+A3sg",
    },
    GoldAnalysisCase {
        surface: "yapamıyor",
        expected_gloss: "yap+Verb+Able+Neg+Prog1+A3sg",
    },
    GoldAnalysisCase {
        surface: "gelirim",
        expected_gloss: "gel+Verb+Pos+Aor+A1sg",
    },
    GoldAnalysisCase {
        surface: "okur",
        expected_gloss: "oku+Verb+Pos+Aor+A3sg",
    },
    GoldAnalysisCase {
        surface: "sever",
        expected_gloss: "sev+Verb+Pos+Aor+A3sg",
    },
    GoldAnalysisCase {
        surface: "çalışır",
        expected_gloss: "çalış+Verb+Pos+Aor+A3sg",
    },
    GoldAnalysisCase {
        surface: "yapabilirim",
        expected_gloss: "yap+Verb+Able+Pos+Aor+A1sg",
    },
    GoldAnalysisCase {
        surface: "yapamaz",
        expected_gloss: "yap+Verb+Able+Neg+Aor+A3sg",
    },
    GoldAnalysisCase {
        surface: "yapmam",
        expected_gloss: "yap+Verb+Neg+Aor+A1sg",
    },
    GoldAnalysisCase {
        surface: "yapmayız",
        expected_gloss: "yap+Verb+Neg+Aor+A1pl",
    },
    GoldAnalysisCase {
        surface: "yapmazlardı",
        expected_gloss: "yap+Verb+Neg+Aor+Past+A3pl",
    },
    GoldAnalysisCase {
        surface: "yaptırıldı",
        expected_gloss: "yap+Verb^DB+Verb+Caus^DB+Verb+Pass+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "okuttu",
        expected_gloss: "oku+Verb^DB+Verb+Caus+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "içirdi",
        expected_gloss: "iç+Verb^DB+Verb+Caus+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "gitti",
        expected_gloss: "git+Verb+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "gider",
        expected_gloss: "git+Verb+Pos+Aor+A3sg",
    },
    GoldAnalysisCase {
        surface: "yediler",
        expected_gloss: "ye+Verb+Pos+Past+A3pl",
    },
    GoldAnalysisCase {
        surface: "yiyor",
        expected_gloss: "ye+Verb+Pos+Prog1+A3sg",
    },
    GoldAnalysisCase {
        surface: "yapıyorduk",
        expected_gloss: "yap+Verb+Pos+Prog1+Past+A1pl",
    },
    GoldAnalysisCase {
        surface: "yapıyorlardı",
        expected_gloss: "yap+Verb+Pos+Prog1+Past+A3pl",
    },
    GoldAnalysisCase {
        surface: "yapmışsa",
        expected_gloss: "yap+Verb+Pos+Narr+Cond+A3sg",
    },
    GoldAnalysisCase {
        surface: "yapsaydın",
        expected_gloss: "yap+Verb+Pos+Cond+Past+A2sg",
    },
    GoldAnalysisCase {
        surface: "yapsınlar",
        expected_gloss: "yap+Verb+Pos+Imp+A3pl",
    },
    GoldAnalysisCase {
        surface: "yapın",
        expected_gloss: "yap+Verb+Pos+Imp+A2pl",
    },
    GoldAnalysisCase {
        surface: "yapalım",
        expected_gloss: "yap+Verb+Pos+Opt+A1pl",
    },
    GoldAnalysisCase {
        surface: "yapmalıyız",
        expected_gloss: "yap+Verb+Pos+Neces+A1pl",
    },
    GoldAnalysisCase {
        surface: "yapacağım",
        expected_gloss: "yap+Verb+Pos+Fut+A1sg",
    },
    GoldAnalysisCase {
        surface: "yapacaklar",
        expected_gloss: "yap+Verb+Pos+Fut+A3pl",
    },
    GoldAnalysisCase {
        surface: "yaptıklarımız",
        expected_gloss: "yap+Verb+Pos^DB+Noun+PastPart+A3pl+P1pl+Nom",
    },
    GoldAnalysisCase {
        surface: "yaptıktan",
        expected_gloss: "yap+Verb+Pos^DB+Noun+PastPart+A3sg+Pnon+Abl",
    },
    GoldAnalysisCase {
        surface: "yapması",
        expected_gloss: "yap+Verb+Pos^DB+Noun+Inf2+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "bekleyiş",
        expected_gloss: "bekle+Verb+Pos^DB+Noun+Inf3+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "yapmayan",
        expected_gloss: "yap+Verb+Neg^DB+Adj+PresPart",
    },
    GoldAnalysisCase {
        surface: "yaparken",
        expected_gloss: "yap+Verb+Pos+Aor^DB+Adverb+While",
    },
    GoldAnalysisCase {
        surface: "koşarcasına",
        expected_gloss: "koş+Verb+Pos+Aor^DB+Adverb+AsIf",
    },
    GoldAnalysisCase {
        surface: "görülür",
        expected_gloss: "gör+Verb^DB+Verb+Pass+Pos+Aor+A3sg",
    },
    GoldAnalysisCase {
        surface: "alındı",
        expected_gloss: "al+Verb^DB+Verb+Pass+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "düşünüyormuş",
        expected_gloss: "düşün+Verb+Pos+Prog1+Narr+A3sg",
    },
    GoldAnalysisCase {
        surface: "güzel",
        expected_gloss: "güzel+Adj",
    },
    GoldAnalysisCase {
        surface: "gençti",
        expected_gloss: "genç+Adj^DB+Verb+Zero+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "güzeller",
        expected_gloss: "güzel+Adj^DB+Noun+Zero+A3pl+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "güzellik",
        expected_gloss: "güzel+Adj^DB+Noun+Ness+A3sg+Pnon+Nom",
    },
    GoldAnalysisCase {
        surface: "büyüğü",
        expected_gloss: "büyük+Adj^DB+Noun+Zero+A3sg+Pnon+Acc",
    },
    GoldAnalysisCase {
        surface: "bitpazarı",
        expected_gloss: "bitpazarı+Noun+A3sg+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "bitpazarında",
        expected_gloss: "bitpazarı+Noun+A3sg+P3sg+Loc",
    },
    GoldAnalysisCase {
        surface: "buzdolabım",
        expected_gloss: "buzdolabı+Noun+A3sg+P1sg+Nom",
    },
    GoldAnalysisCase {
        surface: "buzdolapları",
        expected_gloss: "buzdolabı+Noun+A3pl+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "hanımelleri",
        expected_gloss: "hanımeli+Noun+A3pl+P3sg+Nom",
    },
    GoldAnalysisCase {
        surface: "kitapla",
        expected_gloss: "kitap+Noun+A3sg+Pnon+Ins",
    },
    GoldAnalysisCase {
        surface: "arabasıyla",
        expected_gloss: "araba+Noun+A3sg+P3sg+Ins",
    },
    GoldAnalysisCase {
        surface: "suyla",
        expected_gloss: "su+Noun+A3sg+Pnon+Ins",
    },
    GoldAnalysisCase {
        surface: "güzelleşti",
        expected_gloss: "güzel+Adj^DB+Verb+Become+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "çocuklaştı",
        expected_gloss: "çocuk+Noun+A3sg+Pnon+Nom^DB+Verb+Become+Pos+Past+A3sg",
    },
    GoldAnalysisCase {
        surface: "evlendik",
        expected_gloss: "ev+Noun+A3sg+Pnon+Nom^DB+Verb+Acquire+Pos+Past+A1pl",
    },
    GoldAnalysisCase {
        surface: "oynuyor",
        expected_gloss: "oyna+Verb+Pos+Prog1+A3sg",
    },
    GoldAnalysisCase {
        surface: "okumuyor",
        expected_gloss: "oku+Verb+Neg+Prog1+A3sg",
    },
];

/// Derived surface forms used by the regeneration property test: every
/// analysis of each surface must regenerate byte-identically.
pub const GOLD_DERIVED_SURFACES: &[&str] = &[
    "kitapçı",
    "kitapçılar",
    "kitapçılık",
    "kitapçılığı",
    "kapıcı",
    "kapıcısı",
    "denizcilik",
    "kitaplığa",
    "kitaplıkta",
    "gözlüğü",
    "gözlükçü",
    "evcik",
    "ağaççık",
    "ağaççığı",
    "evsiz",
    "evsizlik",
    "evsizliği",
    "evsizlere",
    "evliler",
    "susuz",
    "sulu",
    "sucu",
    "evdeki",
    "evinki",
    "evdekiler",
    "evdekilerin",
    "evdekine",
    "evdekinde",
    "evdekini",
    "evimdekiler",
    "masadakilerinki",
    "masadakilerinkinde",
    "çocuklarımızdaki",
    "haklılık",
    "saatlik",
    "vakitsiz",
    "evdeydi",
    "evdeydim",
    "evdeydiniz",
    "evdeydiler",
    "evdeymiş",
    "evdeymişsin",
    "evdeymişiz",
    "evdeymiştir",
    "evdeymişken",
    "evdeymişçesine",
    "evdeyken",
    "evdeyim",
    "evdeyiz",
    "evdesin",
    "evdeler",
    "evdedir",
    "evdedirler",
    "evdeyse",
    "evdeysek",
    "evdeyseler",
    "okuldaydık",
    "kitaptı",
    "kitaptır",
    "evsizdi",
    "evsizmiş",
    "evdekiydi",
    "evdekilerdeydi",
    "sudaymış",
    "evimdeydin",
    "kitaplıktaydı",
    "saatteyken",
    "yaptım",
    "gidiyor",
    "gidiyordu",
    "başlıyor",
    "yapmıyor",
    "yapamıyor",
    "gelirim",
    "okur",
    "sever",
    "çalışır",
    "yapabilirim",
    "yapabilirdik",
    "yapamaz",
    "yapmam",
    "yapmayız",
    "yapmazlardı",
    "yaptırıldı",
    "yaptırttı",
    "okuttu",
    "içirdi",
    "oturttu",
    "gitti",
    "gider",
    "yediler",
    "yiyor",
    "yiyordu",
    "yapıyorduk",
    "yapıyorlardı",
    "yapmışsa",
    "yapsaydın",
    "yapsalardı",
    "yapsınlar",
    "yapın",
    "yapsın",
    "yapalım",
    "yapasın",
    "yapmalıyız",
    "yapacağım",
    "yapacaklar",
    "yapacaktı",
    "yaptıklarımız",
    "yaptıktan",
    "yapması",
    "yapmayı",
    "bekleyiş",
    "yapan",
    "yapanlar",
    "yapmayan",
    "yaparken",
    "yapmışçasına",
    "koşarcasına",
    "yapıp",
    "yaparak",
    "gitmeden",
    "yapmaktan",
    "yapmakta",
    "yapmaktaydı",
    "görülür",
    "alındı",
    "düşünüyormuş",
    "güzel",
    "güzeldi",
    "güzeller",
    "güzellik",
    "güzelliği",
    "büyüğü",
    "gençti",
    "gençken",
    "uzaktaki",
    "bitpazarı",
    "bitpazarında",
    "bitpazarındaki",
    "buzdolabım",
    "buzdolabında",
    "buzdolapları",
    "hanımelleri",
    "kitapla",
    "arabayla",
    "arabasıyla",
    "evlerle",
    "suyla",
    "evdekiyle",
    "güzelleşti",
    "güzelleşmiş",
    "çocuklaştı",
    "evlendik",
    "evlendirdi",
    "evlenmeden",
    "oynuyor",
    "oynuyordu",
    "okumuyor",
];

pub const GOLD_GENERATION_CASES: &[GoldGenerationCase] = &[
    GoldGenerationCase {
        lemma: "ev",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Dative,
        expected_surface: "eve",
    },
    GoldGenerationCase {
        lemma: "ev",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Locative,
        expected_surface: "evde",
    },
    GoldGenerationCase {
        lemma: "ev",
        number: Number::Plural,
        possessive: Possessive::None,
        case: Case::Genitive,
        expected_surface: "evlerin",
    },
    GoldGenerationCase {
        lemma: "ev",
        number: Number::Singular,
        possessive: Possessive::SecondPlural,
        case: Case::Locative,
        expected_surface: "evinizde",
    },
    GoldGenerationCase {
        lemma: "ev",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Dative,
        expected_surface: "evine",
    },
    GoldGenerationCase {
        lemma: "ev",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Locative,
        expected_surface: "evinde",
    },
    GoldGenerationCase {
        lemma: "kitap",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Nominative,
        expected_surface: "kitabı",
    },
    GoldGenerationCase {
        lemma: "kitap",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Locative,
        expected_surface: "kitapta",
    },
    GoldGenerationCase {
        lemma: "kitap",
        number: Number::Plural,
        possessive: Possessive::SecondPlural,
        case: Case::Nominative,
        expected_surface: "kitaplarınız",
    },
    GoldGenerationCase {
        lemma: "kitap",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Genitive,
        expected_surface: "kitabın",
    },
    GoldGenerationCase {
        lemma: "ağaç",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Ablative,
        expected_surface: "ağaçtan",
    },
    GoldGenerationCase {
        lemma: "ağaç",
        number: Number::Plural,
        possessive: Possessive::None,
        case: Case::Accusative,
        expected_surface: "ağaçları",
    },
    GoldGenerationCase {
        lemma: "çocuk",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Locative,
        expected_surface: "çocukta",
    },
    GoldGenerationCase {
        lemma: "çocuk",
        number: Number::Plural,
        possessive: Possessive::FirstPlural,
        case: Case::Nominative,
        expected_surface: "çocuklarımız",
    },
    GoldGenerationCase {
        lemma: "çocuk",
        number: Number::Plural,
        possessive: Possessive::ThirdPlural,
        case: Case::Genitive,
        expected_surface: "çocuklarının",
    },
    GoldGenerationCase {
        lemma: "çocuk",
        number: Number::Singular,
        possessive: Possessive::ThirdPlural,
        case: Case::Nominative,
        expected_surface: "çocukları",
    },
    GoldGenerationCase {
        lemma: "burun",
        number: Number::Singular,
        possessive: Possessive::FirstSingular,
        case: Case::Nominative,
        expected_surface: "burnum",
    },
    GoldGenerationCase {
        lemma: "burun",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Locative,
        expected_surface: "burunda",
    },
    GoldGenerationCase {
        lemma: "burun",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Dative,
        expected_surface: "burnuna",
    },
    GoldGenerationCase {
        lemma: "ağız",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Nominative,
        expected_surface: "ağzı",
    },
    GoldGenerationCase {
        lemma: "şehir",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Dative,
        expected_surface: "şehre",
    },
    GoldGenerationCase {
        lemma: "akıl",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Dative,
        expected_surface: "akla",
    },
    GoldGenerationCase {
        lemma: "gönül",
        number: Number::Singular,
        possessive: Possessive::SecondSingular,
        case: Case::Nominative,
        expected_surface: "gönlün",
    },
    GoldGenerationCase {
        lemma: "vakit",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Nominative,
        expected_surface: "vakti",
    },
    GoldGenerationCase {
        lemma: "hak",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Dative,
        expected_surface: "hakka",
    },
    GoldGenerationCase {
        lemma: "hak",
        number: Number::Plural,
        possessive: Possessive::None,
        case: Case::Nominative,
        expected_surface: "haklar",
    },
    GoldGenerationCase {
        lemma: "tıp",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Dative,
        expected_surface: "tıbba",
    },
    GoldGenerationCase {
        lemma: "su",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Nominative,
        expected_surface: "suyu",
    },
    GoldGenerationCase {
        lemma: "su",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Genitive,
        expected_surface: "suyun",
    },
    GoldGenerationCase {
        lemma: "su",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Locative,
        expected_surface: "suda",
    },
    GoldGenerationCase {
        lemma: "saat",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Accusative,
        expected_surface: "saati",
    },
    GoldGenerationCase {
        lemma: "saat",
        number: Number::Plural,
        possessive: Possessive::None,
        case: Case::Nominative,
        expected_surface: "saatler",
    },
    GoldGenerationCase {
        lemma: "saat",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Locative,
        expected_surface: "saatte",
    },
    GoldGenerationCase {
        lemma: "renk",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Genitive,
        expected_surface: "rengin",
    },
    GoldGenerationCase {
        lemma: "kalp",
        number: Number::Singular,
        possessive: Possessive::FirstSingular,
        case: Case::Nominative,
        expected_surface: "kalbim",
    },
    GoldGenerationCase {
        lemma: "alkol",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Accusative,
        expected_surface: "alkolü",
    },
    GoldGenerationCase {
        lemma: "masa",
        number: Number::Singular,
        possessive: Possessive::ThirdSingular,
        case: Case::Locative,
        expected_surface: "masasında",
    },
    GoldGenerationCase {
        lemma: "erk",
        number: Number::Singular,
        possessive: Possessive::None,
        case: Case::Accusative,
        expected_surface: "erki",
    },
];

pub const GOLD_PREFERENCE_CASES: &[GoldPreferenceCase] = &[
    GoldPreferenceCase {
        surface: "evi",
        expected_best_gloss: "ev+Noun+A3sg+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "evin",
        expected_best_gloss: "ev+Noun+A3sg+Pnon+Gen",
    },
    GoldPreferenceCase {
        surface: "evleri",
        expected_best_gloss: "ev+Noun+A3pl+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "kitapları",
        expected_best_gloss: "kitap+Noun+A3pl+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "çocukları",
        expected_best_gloss: "çocuk+Noun+A3pl+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "ağaçları",
        expected_best_gloss: "ağaç+Noun+A3pl+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "ağacı",
        expected_best_gloss: "ağaç+Noun+A3sg+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "kitabın",
        expected_best_gloss: "kitap+Noun+A3sg+Pnon+Gen",
    },
    GoldPreferenceCase {
        surface: "çocuğun",
        expected_best_gloss: "çocuk+Noun+A3sg+Pnon+Gen",
    },
    GoldPreferenceCase {
        surface: "suyu",
        expected_best_gloss: "su+Noun+A3sg+Pnon+Acc",
    },
    GoldPreferenceCase {
        surface: "saati",
        expected_best_gloss: "saat+Noun+A3sg+Pnon+Acc",
    },
];
