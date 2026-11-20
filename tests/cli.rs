use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "stainless-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_stainless"));
        c.args(args)
            .env("STAINLESS_HOME", &self.0)
            .env_remove("STAINLESS_MODEL");
        c
    }
    fn exec(&self, args: &[&str]) -> Output {
        self.cmd(args).output().unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let o = self.exec(args);
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_owned()
    }
    fn json(&self, args: &[&str]) -> Value {
        serde_json::from_str(&self.ok(args)).unwrap()
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn request(s: &TcpStream) -> Value {
    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut r = BufReader::new(s);
    let mut header = String::new();
    let mut size = 0;
    loop {
        header.clear();
        assert!(r.read_line(&mut header).unwrap() > 0);
        if header == "\r\n" {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                size = v.trim().parse().unwrap();
            }
        }
    }
    let mut data = vec![0; size];
    r.read_exact(&mut data).unwrap();
    serde_json::from_slice(&data).unwrap()
}
fn reply(s: &mut TcpStream, done: bool) {
    write!(s,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {{\"choices\":[{{\"delta\":{{\"content\":\"hello\"}}}}]}}\n\n").unwrap();
    if done {
        write!(s, "data: [DONE]\n\n").unwrap();
    }
}
#[test]
fn session_lifecycle_and_isolation() {
    let c = Case::new();
    let a = c.ok(&["session", "new", "--name", "alpha"]);
    let b = c.ok(&["session", "new", "--name", "beta"]);
    assert_ne!(a, b);
    assert_eq!(c.json(&["session", "list", "--json"])[0]["id"], b);
    c.ok(&["session", "checkpoint", &a]);
    let x = c.json(&["session", "export", "last"]);
    assert_eq!(x["session"], a);
    assert_eq!(x["events"][1]["kind"], "checkpoint");
    assert_eq!(
        c.json(&["session", "export", &b])["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(c.ok(&["session", "resume", &a]).contains("alpha"));
    assert!(!c.exec(&["session", "resume", "missing"]).status.success());
}
#[test]
fn reject_non_tty_and_invalid_input() {
    let c = Case::new();
    assert!(!c.exec(&["steel"]).status.success());
    assert!(!c.0.exists());
    assert!(!c
        .exec(&[
            "session",
            "new",
            "--workspace",
            "/path-that-does-not-exist-ss"
        ])
        .status
        .success());
    assert!(!c.exec(&["run", "--model", "m", " "]).status.success());
    assert_eq!(
        c.json(&["session", "list", "--json"]),
        serde_json::json!([])
    );
}
#[test]
fn streamed_turn_and_history() {
    let c = Case::new();
    let id = c.ok(&["session", "new"]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server = thread::spawn(move || {
        for index in 0..2 {
            let (mut s, _) = listener.accept().unwrap();
            let req = request(&s);
            assert_eq!(req["messages"].as_array().unwrap().len(), 2 * index + 1);
            if index == 1 {
                assert_eq!(req["messages"][1]["content"], "hello");
            }
            reply(&mut s, true);
        }
    });
    for _ in 0..2 {
        let stream = c.ok(&[
            "run",
            "--json",
            "--session",
            &id,
            "--port",
            &port,
            "--model",
            "fixture",
            "hi",
        ]);
        let lines: Vec<Value> = stream
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert_eq!(lines[1]["text"], "hello");
        assert_eq!(lines.last().unwrap()["type"], "done");
    }
    server.join().unwrap();
}
#[test]
fn truncated_stream_marks_interruption() {
    let c = Case::new();
    let id = c.ok(&["session", "new"]);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port().to_string();
    let srv = thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        request(&s);
        reply(&mut s, false);
    });
    assert!(!c
        .exec(&[
            "run",
            "--session",
            &id,
            "--model",
            "m",
            "--port",
            &port,
            "hi"
        ])
        .status
        .success());
    srv.join().unwrap();
    assert_eq!(
        c.json(&["session", "export", &id])["events"][2]["kind"],
        "interrupted"
    );
}
#[test]
fn killed_turn_requires_explicit_recovery_and_never_replays() {
    let c = Case::new();
    let id = c.ok(&["session", "new"]);
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port().to_string();
    let args = [
        "run",
        "--session",
        &id,
        "--port",
        &port,
        "--model",
        "m",
        "hi",
    ];
    let mut child = c
        .cmd(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (s, _) = l.accept().unwrap();
    request(&s); // Turn intent was committed before this request.
    let conflict = c.exec(&args);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("active turn"));
    child.kill().unwrap();
    child.wait().unwrap();
    drop(s);
    let n = c.json(&["session", "recovery", "inspect", &id])["pending_turn"]
        .as_i64()
        .unwrap()
        .to_string();
    c.ok(&["session", "resume", &id]);
    assert!(!c
        .exec(&["session", "recovery", "resolve", &id, "--turn", "999"])
        .status
        .success());
    c.ok(&["session", "recovery", "resolve", &id, "--turn", &n]);
    assert!(c.json(&["session", "recovery", "inspect", &id])["pending_turn"].is_null());
    l.set_nonblocking(true).unwrap();
    assert_eq!(
        l.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn refuses_future_schema() {
    let c = Case::new();
    c.ok(&["doctor", "--json"]);
    rusqlite::Connection::open(c.0.join("sessions.sqlite3"))
        .unwrap()
        .execute_batch("PRAGMA user_version=99")
        .unwrap();
    assert!(!c.exec(&["doctor", "--json"]).status.success());
}
#[cfg(unix)]
#[test]
fn refuses_shared_state_directory() {
    use std::os::unix::fs::PermissionsExt;
    let c = Case::new();
    fs::create_dir(&c.0).unwrap();
    fs::set_permissions(&c.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!c.exec(&["doctor"]).status.success());
    assert!(!c.0.join("sessions.sqlite3").exists());
}

#[test]
fn backup_snapshot_is_independent_and_never_overwrites() {
    let c = Case::new();
    let id = c.ok(&["session", "new", "--name", "original"]);
    let path = c.0.join("backup.db");
    let path = path.to_str().unwrap();
    c.ok(&["backup", "create", path]);
    c.ok(&["backup", "verify", path]);
    c.ok(&["session", "rename", &id, "updated"]);
    assert!(!c.exec(&["backup", "create", path]).status.success());
    let snapshot = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        snapshot
            .query_row("SELECT name FROM sessions", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "original"
    );
    assert_eq!(c.json(&["session", "list", "--json"])[0]["name"], "updated");
}
#[test]
fn verify_rejects_foreign_db_without_creating_state() {
    let c = Case::new();
    let path = c.0.with_extension("foreign");
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA user_version=1")
        .unwrap();
    assert!(!c
        .exec(&["backup", "verify", path.to_str().unwrap()])
        .status
        .success());
    fs::remove_file(path).unwrap();
    assert!(!c.0.exists());
}
