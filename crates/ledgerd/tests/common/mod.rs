//! Run the real `ledgerd` binary and talk to it over plain HTTP/1.1.

#![allow(dead_code)] // each test file uses a different subset

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Server {
    pub child: Child,
    pub addr: String,
    // Held open so the server can always write to its stdout.
    _stdout: BufReader<ChildStdout>,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start `ledgerd` on a free port against the journal at `db`.
pub fn start(db: &Path) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ledgerd"))
        .env("LEDGER_DB", db)
        .env("LEDGER_ADDR", "127.0.0.1:0")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("ledgerd starts");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .expect("ledgerd prints its address");
    let addr = line
        .trim()
        .rsplit(' ')
        .next()
        .expect("an address")
        .to_string();
    Server {
        child,
        addr,
        _stdout: stdout,
    }
}

/// Send one request and return the status code and the body.
pub fn http(addr: &str, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connects");
    let body = body.unwrap_or("");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("sends");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("reads");
    let code = response
        .get(9..12)
        .and_then(|c| c.parse().ok())
        .expect("a status line");
    let body = response
        .split_once("\r\n\r\n")
        .map_or("", |(_, b)| b)
        .to_string();
    (code, body)
}

pub fn json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("not JSON ({e}): {body}"))
}

/// Poll `GET /jobs/{job}` until `done` holds, or panic with the last reply after `limit`.
pub fn wait_for(
    addr: &str,
    job: u64,
    limit: Duration,
    done: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = Instant::now() + limit;
    loop {
        let (_, body) = http(addr, "GET", &format!("/jobs/{job}"), None);
        let status = json(&body);
        if done(&status) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "gave up waiting; last status: {status}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
