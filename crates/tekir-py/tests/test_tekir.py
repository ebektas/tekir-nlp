"""Smoke tests for the Python bindings: `maturin develop` then `pytest`."""

import tekir


def test_segment_and_analyze():
    assert tekir.segment("kitaplarımızdan") == ["kitap", "lar", "ımız", "dan"]
    best = tekir.analyze("evler")[0]
    assert best["lexical_form"] == "ev+Noun+A3pl+Pnon+Nom"
    assert best["morphemes"] == ["ev", "ler"]


def test_pretokenizer_is_lossless():
    text = "Kitaplarımızdan biri, Ankara'dakilere 2024'te gönderildi.\n\n"
    pieces = tekir.Pretokenizer().pretokenize(text)
    assert "".join(pieces) == text
    assert pieces[:4] == ["Kitap", "lar", "ımız", "dan"]


def test_tokenizer_roundtrip():
    lines = ["Evlerimizde kitaplar var.", "Kitaplarımızdan biri evdeydi."] * 50
    tok = tekir.train_tokenizer(lines, vocab_size=400)
    text = "Kitaplarımız evlerde."
    assert tok.decode(tok.encode(text)) == text


def test_prep():
    assert tekir.restore("cok guzel") == "çok güzel"
    assert tekir.redact_pii("TC 10000000146") == "TC [TCKN]"
    assert tekir.detect_pii("çğ 10000000146") == [("tckn", 3, 14)]
    assert tekir.dedup(["a b c d e f", "a b c d e f", "x y z"]) == [True, False, True]
    assert tekir.quality("Lorem ipsum dolor sit amet.")["passes"] is False
