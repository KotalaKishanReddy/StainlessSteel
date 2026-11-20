use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
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
    fn piped(&self, args: &[&str], stdin_data: &str) -> Output {
        let mut child = self
            .cmd(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_data.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
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
fn update_status_reports_version_without_network() {
    let c = Case::new();
    let v = c.json(&["update", "status", "--json"]);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["channel"], "stable");
    assert_eq!(v["install"]["kind"], "unknown");
    assert!(c.ok(&["update", "status"]).contains("stainless v"));
}
#[test]
fn completions_emit_shell_scripts() {
    let c = Case::new();
    for shell in ["bash", "zsh", "fish"] {
        let out = c.ok(&["completions", shell]);
        assert!(out.contains("stainless"), "empty {shell} completions");
    }
}
#[test]
fn context_inspect_counts_events_and_turns() {
    let c = Case::new();
    let id = c.ok(&["session", "new", "--name", "ctx"]);
    c.ok(&["session", "checkpoint", &id]);
    let s = c.json(&["context", "inspect", "--json", &id]);
    assert_eq!(s["session"], id);
    assert_eq!(s["events"], 2);
    assert_eq!(s["by_kind"]["created"], 1);
    assert_eq!(s["by_kind"]["checkpoint"], 1);
    assert_eq!(s["turns"]["complete"], 0);
    assert_eq!(s["budget"], 262144);
    assert!(c.ok(&["context", "inspect", &id]).contains("events: 2"));
    assert!(!c.exec(&["context", "inspect", "missing"]).status.success());
}
#[test]
fn migrates_v1_journal_to_v3() {
    let c = Case::new();
    fs::create_dir_all(&c.0).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&c.0, fs::Permissions::from_mode(0o700)).unwrap();
    }
    rusqlite::Connection::open(c.0.join("sessions.sqlite3"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE sessions(id TEXT PRIMARY KEY, name TEXT NOT NULL, workspace TEXT NOT NULL);
            CREATE TABLE events(seq INTEGER PRIMARY KEY, session TEXT NOT NULL REFERENCES sessions(id), kind TEXT NOT NULL, body TEXT NOT NULL, at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
            CREATE INDEX session_events ON events(session, seq);
            CREATE TABLE turns(id INTEGER PRIMARY KEY REFERENCES events(seq), session TEXT NOT NULL REFERENCES sessions(id), model TEXT NOT NULL, status TEXT NOT NULL, reply TEXT);
            CREATE UNIQUE INDEX one_active ON turns(session) WHERE status='running';
            INSERT INTO sessions VALUES('ses_legacy','legacy','/tmp');
            INSERT INTO events(session,kind,body) VALUES('ses_legacy','created','legacy');
            PRAGMA user_version=1;",
        )
        .unwrap();
    assert_eq!(c.json(&["session", "list", "--json"])[0]["name"], "legacy");
    c.ok(&["session", "checkpoint", "ses_legacy"]);
    assert_eq!(
        c.json(&["context", "inspect", "--json", "ses_legacy"])["events"],
        2
    );
}
#[test]
fn fork_import_and_archive_sessions() {
    let c = Case::new();
    let id = c.ok(&["session", "new", "--name", "orig"]);
    c.ok(&["session", "checkpoint", &id]);
    let fork = c.ok(&["session", "fork", &id]);
    assert_ne!(fork, id);
    assert_eq!(
        c.json(&["session", "export", &fork])["events"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let partial = c.ok(&["session", "fork", &id, "--at", "1"]);
    assert_eq!(
        c.json(&["session", "export", &partial])["events"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    c.ok(&["session", "archive", &id]);
    let list = c.json(&["session", "list", "--json"]);
    assert!(list.as_array().unwrap().iter().all(|s| s["id"] != id));
    assert_eq!(
        c.json(&["session", "list", "--json", "--all"])
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(!c.exec(&["session", "archive", "missing"]).status.success());
    let export = c.ok(&["session", "export", &fork]);
    let path = c.0.join("exp.json");
    fs::write(&path, &export).unwrap();
    let imp = c.ok(&[
        "session",
        "import",
        path.to_str().unwrap(),
        "--name",
        "copy",
    ]);
    assert_ne!(imp, fork);
    assert_eq!(
        c.json(&["session", "export", &imp])["events"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(!c
        .exec(&[
            "session",
            "import",
            path.to_str().unwrap(),
            "--workspace",
            "/path-that-does-not-exist-ss"
        ])
        .status
        .success());
}
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn git_setup(path: &str) {
    for args in [
        vec!["init"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test"],
    ] {
        assert!(Command::new("git")
            .arg("-C")
            .arg(path)
            .args(&args)
            .output()
            .unwrap()
            .status
            .success());
    }
}
#[test]
fn repo_commands_wrap_system_git() {
    if !git_available() {
        return;
    }
    let c = Case::new();
    let ws = c.0.join("ws");
    fs::create_dir_all(&ws).unwrap();
    git_setup(ws.to_str().unwrap());
    fs::write(ws.join("new.txt"), "hello\n").unwrap();
    let w = ws.to_str().unwrap();
    let status = c.json(&["repo", "status", "--workspace", w, "--json"]);
    assert!(status["porcelain"].as_str().unwrap().contains("new.txt"));
    assert!(Command::new("git")
        .arg("-C")
        .arg(w)
        .args(["add", "new.txt"])
        .output()
        .unwrap()
        .status
        .success());
    c.ok(&[
        "repo",
        "commit",
        "--workspace",
        w,
        "--message",
        "init",
        "new.txt",
    ]);
    assert!(!c.ok(&["repo", "branch", "--workspace", w]).is_empty());
    fs::write(ws.join("new.txt"), "hello\nworld\n").unwrap();
    assert!(c.ok(&["repo", "diff", "--workspace", w]).contains("+world"));
    assert!(c
        .ok(&["repo", "diff", "--workspace", w, "--stat"])
        .contains("new.txt"));
    let wt = c.0.join("wt");
    c.ok(&[
        "repo",
        "worktree",
        "add",
        "--workspace",
        w,
        wt.to_str().unwrap(),
        "--branch",
        "agent-branch",
    ]);
    assert!(c
        .ok(&["repo", "worktree", "list", "--workspace", w])
        .contains(wt.to_str().unwrap()));
    c.ok(&[
        "repo",
        "worktree",
        "remove",
        "--workspace",
        w,
        wt.to_str().unwrap(),
    ]);
    assert!(!c
        .exec(&[
            "repo",
            "status",
            "--workspace",
            "/path-that-does-not-exist-ss"
        ])
        .status
        .success());
}
#[test]
fn patch_applies_modify_create_delete_and_rejects() {
    let c = Case::new();
    let ws = c.0.join("ws");
    fs::create_dir_all(&ws).unwrap();
    let w = ws.to_str().unwrap();
    fs::write(ws.join("hello.txt"), "a\nb\nc\n").unwrap();
    fs::write(ws.join("gone.txt"), "gone\n").unwrap();
    let modify = "--- a/hello.txt\n+++ b/hello.txt\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n";
    let out = c.piped(&["patch", "apply", "--workspace", w], modify);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(ws.join("hello.txt")).unwrap(),
        "a\nB\nc\n"
    );
    let create = "--- /dev/null\n+++ b/new.txt\n@@ -0,0 +1,2 @@\n+one\n+two\n";
    let out = c.piped(&["patch", "apply", "--workspace", w], create);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(ws.join("new.txt")).unwrap(),
        "one\ntwo\n"
    );
    let delete = "--- a/gone.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n";
    let out = c.piped(&["patch", "apply", "--workspace", w], delete);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!ws.join("gone.txt").exists());
    let mismatch = "--- a/hello.txt\n+++ b/hello.txt\n@@ -1,3 +1,3 @@\n a\n-WRONG\n c\n";
    assert!(!c
        .piped(&["patch", "apply", "--workspace", w], mismatch)
        .status
        .success());
    let traversal = "--- a/../evil.txt\n+++ b/../evil.txt\n@@ -0,0 +1 @@\n+x\n";
    assert!(!c
        .piped(&["patch", "apply", "--workspace", w], traversal)
        .status
        .success());
    assert!(!c.0.join("evil.txt").exists());
}
#[test]
fn agent_budget_is_enforced() {
    let c = Case::new();
    let out = c
        .cmd(&["agent", "spawn", "--model", "m", "hi"])
        .env("STAINLESS_MAX_AGENTS", "0")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("budget"));
}
#[test]
fn agent_spawn_runs_turn_and_reconciles() {
    let c = Case::new();
    let id = c.ok(&["session", "new"]);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server = thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        request(&s);
        reply(&mut s, true);
    });
    let agent = c.ok(&[
        "agent",
        "spawn",
        "--session",
        &id,
        "--port",
        &port,
        "--model",
        "fixture",
        "hi",
    ]);
    let mut status = String::new();
    for _ in 0..100 {
        status = c.json(&["agent", "inspect", "--json", &agent])["status"]
            .as_str()
            .unwrap()
            .to_owned();
        if status != "running" {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(status, "done");
    server.join().unwrap();
    assert!(c
        .json(&["agent", "list", "--json"])
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["id"] == agent));
    assert!(c.ok(&["agent", "tree"]).contains(&agent));
    assert!(!c.exec(&["agent", "cancel", &agent]).status.success());
    assert!(!c
        .exec(&["agent", "inspect", "agt_missing"])
        .status
        .success());
}
#[test]
fn agent_spawn_marks_failure_when_model_unreachable() {
    let c = Case::new();
    let id = c.ok(&["session", "new"]);
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = dead.local_addr().unwrap().port().to_string();
    drop(dead);
    let agent = c.ok(&[
        "agent",
        "spawn",
        "--session",
        &id,
        "--port",
        &port,
        "--model",
        "fixture",
        "hi",
    ]);
    let mut status = String::new();
    for _ in 0..100 {
        status = c.json(&["agent", "inspect", "--json", &agent])["status"]
            .as_str()
            .unwrap()
            .to_owned();
        if status != "running" {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(status, "failed");
}
fn serve_fixture(listener: TcpListener, sums: String, asset: Vec<u8>, n: usize) {
    for _ in 0..n {
        let (mut s, _) = listener.accept().unwrap();
        let head = request_head(&s);
        let payload: &[u8] = if head.contains("SHA256SUMS.txt") {
            sums.as_bytes()
        } else {
            &asset
        };
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", payload.len()).unwrap();
        s.write_all(payload).unwrap();
    }
}
fn request_head(s: &TcpStream) -> String {
    s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .unwrap();
    let mut r = BufReader::new(s);
    let mut head = String::new();
    loop {
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        head.push_str(&line);
    }
    head
}
#[test]
fn update_download_stages_verified_asset() {
    if std::env::consts::OS != "linux" {
        return;
    }
    let asset_name = format!("stainless-linux-{}", std::env::consts::ARCH);
    let asset: Vec<u8> = b"fake-binary-bytes".to_vec();
    let hash = {
        let p = std::env::temp_dir().join(format!("stainless-hash-{}", std::process::id()));
        fs::write(&p, &asset).unwrap();
        let hex =
            String::from_utf8(Command::new("sha256sum").arg(&p).output().unwrap().stdout).unwrap();
        fs::remove_file(&p).unwrap();
        hex.split_whitespace().next().unwrap().to_owned()
    };
    let sums = format!("{hash}  {asset_name}\n");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server = thread::spawn(move || serve_fixture(listener, sums, asset, 2));
    let c = Case::new();
    let base = format!("http://127.0.0.1:{port}");
    let out = c
        .cmd(&["update", "download", "--version", "v9.9.9", "--json"])
        .env("STAINLESS_API_BASE", &base)
        .env("STAINLESS_DOWNLOAD_BASE", &base)
        .env("STAINLESS_REPO", "x")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["asset"], asset_name);
    let staged = PathBuf::from(v["path"].as_str().unwrap());
    assert_eq!(fs::read(&staged).unwrap(), b"fake-binary-bytes");
    server.join().unwrap();
    let bad_sums = format!("{}  {asset_name}\n", "0".repeat(64));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server =
        thread::spawn(move || serve_fixture(listener, bad_sums, b"tampered-bytes".to_vec(), 2));
    let c2 = Case::new();
    let base = format!("http://127.0.0.1:{port}");
    let out = c2
        .cmd(&["update", "download", "--version", "v9.9.9"])
        .env("STAINLESS_API_BASE", &base)
        .env("STAINLESS_DOWNLOAD_BASE", &base)
        .env("STAINLESS_REPO", "x")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("checksum"));
    assert!(!c2
        .0
        .join("updates")
        .join("v9.9.9")
        .join(&asset_name)
        .exists());
    server.join().unwrap();
}
fn fake_gh(dir: &std::path::Path, code: i32) -> PathBuf {
    let bin = dir.join("fakebin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("gh");
    fs::write(&script, format!("#!/bin/sh\nexit {code}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin
}
fn direct_install_copy(c: &Case) -> PathBuf {
    fs::create_dir_all(&c.0).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&c.0, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let dest = c.0.join("stainless");
    fs::copy(env!("CARGO_BIN_EXE_stainless"), &dest).unwrap();
    fs::write(
        c.0.join(".stainless-install-receipt.json"),
        r#"{"install_kind":"direct"}"#,
    )
    .unwrap();
    dest
}
#[test]
fn update_install_refuses_foreign_installs_and_empty_rollback() {
    let c = Case::new();
    let out = c.exec(&["update", "install", "--version", "v0.1.0"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("its own tool"));
    assert!(!c.exec(&["update", "rollback"]).status.success());
    assert!(
        String::from_utf8_lossy(&c.exec(&["update", "rollback"]).stderr)
            .contains("no previous binary")
    );
}
#[test]
fn update_install_reports_current_and_journals() {
    let c = Case::new();
    let dest = direct_install_copy(&c);
    let current = format!("v{}", env!("CARGO_PKG_VERSION"));
    let out = Command::new(&dest)
        .args(["update", "install", "--version", &current])
        .env("STAINLESS_HOME", &c.0)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("nothing to do"));
    let log = fs::read_to_string(c.0.join("updates").join("install.log")).unwrap();
    assert!(log.contains("already-current"));
}
#[test]
fn update_install_aborts_when_attestation_fails() {
    if std::env::consts::OS != "linux" {
        return;
    }
    let asset_name = format!("stainless-linux-{}", std::env::consts::ARCH);
    let asset: Vec<u8> = b"tampered-bytes".to_vec();
    let hash = {
        let p = std::env::temp_dir().join(format!("stainless-h2-{}", std::process::id()));
        fs::write(&p, &asset).unwrap();
        let hex =
            String::from_utf8(Command::new("sha256sum").arg(&p).output().unwrap().stdout).unwrap();
        fs::remove_file(&p).unwrap();
        hex.split_whitespace().next().unwrap().to_owned()
    };
    let sums = format!("{hash}  {asset_name}\n");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server = thread::spawn(move || serve_fixture(listener, sums, asset, 2));
    let c = Case::new();
    let dest = direct_install_copy(&c);
    let bin = fake_gh(&c.0, 1);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let base = format!("http://127.0.0.1:{port}");
    let out = Command::new(&dest)
        .args(["update", "install", "--version", "v9.9.9"])
        .env("STAINLESS_HOME", &c.0)
        .env("PATH", &path)
        .env("STAINLESS_API_BASE", &base)
        .env("STAINLESS_DOWNLOAD_BASE", &base)
        .env("STAINLESS_REPO", "x")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("attestation"));
    let log = fs::read_to_string(c.0.join("updates").join("install.log")).unwrap();
    assert!(log.contains("aborted"));
    server.join().unwrap();
}
#[test]
fn update_install_refuses_while_a_turn_is_active() {
    if std::env::consts::OS != "linux" {
        return;
    }
    let c = Case::new();
    let dest = direct_install_copy(&c);
    let bin = fake_gh(&c.0, 0);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let asset_name = format!("stainless-linux-{}", std::env::consts::ARCH);
    let asset: Vec<u8> = b"quiesce-bytes".to_vec();
    let hash = {
        let p = std::env::temp_dir().join(format!("stainless-h3-{}", std::process::id()));
        fs::write(&p, &asset).unwrap();
        let hex =
            String::from_utf8(Command::new("sha256sum").arg(&p).output().unwrap().stdout).unwrap();
        fs::remove_file(&p).unwrap();
        hex.split_whitespace().next().unwrap().to_owned()
    };
    let sums = format!("{hash}  {asset_name}\n");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server = thread::spawn(move || serve_fixture(listener, sums, asset, 2));
    let base = format!("http://127.0.0.1:{port}");
    let id = c.ok(&["session", "new"]);
    let hanging = TcpListener::bind("127.0.0.1:0").unwrap();
    let hport = hanging.local_addr().unwrap().port().to_string();
    let holder = thread::spawn(move || {
        let (s, _) = hanging.accept().unwrap();
        request(&s);
        thread::sleep(Duration::from_secs(20));
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_stainless"))
        .args([
            "run",
            "--session",
            &id,
            "--port",
            &hport,
            "--model",
            "m",
            "hi",
        ])
        .env("STAINLESS_HOME", &c.0)
        .env_remove("STAINLESS_MODEL")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut pending = false;
    for _ in 0..100 {
        let v = c.json(&["session", "recovery", "inspect", &id]);
        if !v["pending_turn"].is_null() {
            pending = true;
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    assert!(pending);
    let out = Command::new(&dest)
        .args(["update", "install", "--version", "v9.9.9"])
        .env("STAINLESS_HOME", &c.0)
        .env("PATH", &path)
        .env("STAINLESS_API_BASE", &base)
        .env("STAINLESS_DOWNLOAD_BASE", &base)
        .env("STAINLESS_REPO", "x")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("active turn"));
    child.kill().unwrap();
    child.wait().unwrap();
    drop(holder);
    server.join().unwrap();
}
#[test]
fn update_download_falls_back_to_legacy_asset_name() {
    if std::env::consts::OS != "linux" {
        return;
    }
    let legacy = format!("stainless-9.9.9-linux-{}", std::env::consts::ARCH);
    let asset: Vec<u8> = b"legacy-bytes".to_vec();
    let hash = {
        let p = std::env::temp_dir().join(format!("stainless-h4-{}", std::process::id()));
        fs::write(&p, &asset).unwrap();
        let hex =
            String::from_utf8(Command::new("sha256sum").arg(&p).output().unwrap().stdout).unwrap();
        fs::remove_file(&p).unwrap();
        hex.split_whitespace().next().unwrap().to_owned()
    };
    let sums = format!("{hash}  {legacy}\n");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let server = thread::spawn(move || serve_fixture(listener, sums, asset, 2));
    let c = Case::new();
    let base = format!("http://127.0.0.1:{port}");
    let out = c
        .cmd(&["update", "download", "--version", "v9.9.9", "--json"])
        .env("STAINLESS_API_BASE", &base)
        .env("STAINLESS_DOWNLOAD_BASE", &base)
        .env("STAINLESS_REPO", "x")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["asset"], legacy);
    assert_eq!(
        fs::read(PathBuf::from(v["path"].as_str().unwrap())).unwrap(),
        b"legacy-bytes"
    );
    server.join().unwrap();
}
#[test]
fn tool_list_publishes_registry() {
    let c = Case::new();
    let out = c.ok(&["tool", "list"]);
    assert!(out.contains("session.new") && out.contains("patch.apply"));
    let defs = c.json(&["tool", "list", "--json"]);
    let arr = defs.as_array().unwrap();
    assert!(arr.len() > 20);
    assert!(arr
        .iter()
        .all(|d| d["id"].is_string() && d["input_schema"]["type"] == "object"));
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
