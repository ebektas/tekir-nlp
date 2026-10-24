//! Server smoke test: health, a large POST body, and JSON errors.
#![cfg(feature = "server")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command};

fn request(port: u16, raw: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    stream.write_all(raw.as_bytes()).expect("write");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read");
    response
}

fn wait_up(port: u16, child: &mut Child) {
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        assert!(child.try_wait().expect("poll").is_none(), "server exited");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("server did not come up");
}

#[test]
fn serves_health_and_large_bodies() {
    let port = 8461;
    let mut child = Command::new(env!("CARGO_BIN_EXE_tekir-cli"))
        .args(["serve", "--port", &port.to_string(), "--threads", "2"])
        .spawn()
        .expect("server should spawn");
    wait_up(port, &mut child);

    let health = request(
        port,
        "GET /health HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    );
    assert!(health.contains("200"), "health failed: {health}");
    assert!(health.contains("\"ok\""));

    // ~2MB body through the tokenizer.
    let text = "evdeki kediler uyudu ".repeat(100_000);
    let body = format!("{{\"text\":\"{text}\"}}");
    let large = request(
        port,
        &format!(
            "POST /tokenize HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(large.contains("200"), "large body failed");
    assert!(large.contains("token_count"));

    let bad = request(
        port,
        "POST /morph HTTP/1.1\r\nHost: x\r\nConnection: close\r\nContent-Length: 3\r\n\r\nnop",
    );
    assert!(bad.contains("400"));

    let _ = child.kill();
    let _ = child.wait();
}
