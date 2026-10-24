//! Parallel JSONL batch output must be byte-identical to sequential.

use std::io::Write;
use std::process::{Command, Stdio};

fn run_batch(jobs: &str, input: &str) -> Vec<u8> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_tekir-cli"))
        .args([
            "morph",
            "--stdin",
            "--input-format",
            "jsonl",
            "--output",
            "jsonl",
            "--jobs",
            jobs,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("binary should spawn");
    // Write stdin from a thread so the child's stdout can drain
    // concurrently (avoids pipe deadlock on large batches).
    let mut stdin = child.stdin.take().expect("stdin");
    let payload = input.to_string();
    let writer = std::thread::spawn(move || {
        stdin.write_all(payload.as_bytes()).expect("write input");
    });
    let output = child.wait_with_output().expect("binary should finish");
    writer.join().expect("writer thread");
    assert!(output.status.success());
    output.stdout
}

#[test]
fn parallel_batch_output_is_deterministic() {
    let words = [
        "evlerinde",
        "kitapçılık",
        "yaptırılmıştı",
        "suyu",
        "gözlükçü",
    ];
    let mut input = String::new();
    for index in 0..2_000 {
        input.push_str(&format!(
            "{{\"id\":\"{index}\",\"text\":\"{}\"}}\n",
            words[index % words.len()]
        ));
    }

    let sequential = run_batch("1", &input);
    let parallel = run_batch("16", &input);
    assert_eq!(sequential.len(), parallel.len());
    assert!(sequential == parallel, "outputs differ between job counts");
}
