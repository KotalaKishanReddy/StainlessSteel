use crate::Result;
use serde_json::{json, Value};
use std::{io::Read, time::Duration};

const DEFAULT_REPO: &str = "KotalaKishanReddy/StainlessSteel";

fn repo() -> String {
    std::env::var("STAINLESS_REPO").unwrap_or_else(|_| DEFAULT_REPO.into())
}

fn api_base() -> String {
    std::env::var("STAINLESS_API_BASE").unwrap_or_else(|_| "https://api.github.com".into())
}

fn download_base() -> String {
    std::env::var("STAINLESS_DOWNLOAD_BASE").unwrap_or_else(|_| "https://github.com".into())
}

pub fn channel() -> String {
    std::env::var("STAINLESS_CHANNEL").unwrap_or_else(|_| "stable".into())
}

fn tag_version(tag: &str) -> &str {
    tag.strip_prefix('v').unwrap_or(tag)
}

fn parse_version(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn latest_tag() -> Result<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();
    let res = agent
        .get(&format!("{}/repos/{}/releases/latest", api_base(), repo()))
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "stainless-update-check")
        .call()
        .map_err(|_| "update check failed: GitHub API unreachable")?;
    if res.status() != 200 {
        return Err(format!("update check failed: HTTP {}", res.status()).into());
    }
    let raw = res
        .into_string()
        .map_err(|_| "update check failed: invalid response")?;
    let body: Value =
        serde_json::from_str(&raw).map_err(|_| "update check failed: invalid response")?;
    body["tag_name"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "update check failed: no release tag".into())
}

fn install_receipt() -> Value {
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join(".stainless-install-receipt.json"))
        })
        .filter(|p| p.is_file());
    match path {
        Some(p) => {
            let raw = std::fs::read_to_string(&p).unwrap_or_default();
            let receipt: Value = serde_json::from_str(&raw).unwrap_or(Value::Null);
            json!({"kind":"direct","receipt":receipt})
        }
        None => json!({"kind":"unknown"}),
    }
}

pub fn check(machine: bool) -> Result<()> {
    let tag = latest_tag()?;
    let current = env!("CARGO_PKG_VERSION");
    let available = parse_version(tag_version(&tag)) > parse_version(current);
    if machine {
        println!(
            "{}",
            json!({"current":current,"latest":tag_version(&tag),"channel":channel(),"update_available":available})
        );
    } else if available {
        println!("Update available: v{current} → {tag}");
    } else {
        println!("Already up to date (v{current})");
    }
    Ok(())
}

fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(120))
        .build();
    let res = agent
        .get(url)
        .set("User-Agent", "stainless-update-check")
        .call()
        .map_err(|_| -> Box<dyn std::error::Error> { format!("download failed: {url}").into() })?;
    if res.status() != 200 {
        return Err(format!("download failed: HTTP {}", res.status()).into());
    }
    let mut bytes = Vec::new();
    res.into_reader()
        .read_to_end(&mut bytes)
        .map_err(|_| "download failed: unreadable body")?;
    Ok(bytes)
}

fn fetch_text(url: &str) -> Result<String> {
    let bytes = fetch_bytes(url)?;
    String::from_utf8(bytes).map_err(|_| "download failed: invalid text".into())
}

fn verify_sha256(path: &std::path::Path, want: &str) -> Result<()> {
    for tool in [vec!["sha256sum"], vec!["shasum", "-a", "256"]] {
        let probe = std::process::Command::new(tool[0])
            .args(&tool[1..])
            .arg(path)
            .output();
        let Ok(output) = probe else { continue };
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let got = stdout.split_whitespace().next().unwrap_or("");
        if got.eq_ignore_ascii_case(want) {
            return Ok(());
        }
        return Err("checksum mismatch: staged file deleted".into());
    }
    Err("no sha256 tool found (need sha256sum or shasum)".into())
}

/// Current asset scheme: `stainless-<os>-<arch>`, versioned by tag URL.
fn asset_name() -> Result<String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("stainless-linux-x86_64".into()),
        ("linux", "aarch64") => Ok("stainless-linux-aarch64".into()),
        ("macos", "aarch64") => Ok("stainless-macos-aarch64".into()),
        (os, arch) => {
            Err(format!("no prebuilt asset for {os}/{arch} yet; build from source").into())
        }
    }
}

/// Legacy v0.1.0 scheme kept as a download fallback only.
fn legacy_asset_name(version: &str) -> Result<String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok(format!("stainless-{version}-linux-x86_64")),
        ("linux", "aarch64") => Ok(format!("stainless-{version}-linux-aarch64")),
        (os, arch) => {
            Err(format!("no prebuilt asset for {os}/{arch} yet; build from source").into())
        }
    }
}

fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?;
        (file == asset).then(|| hash.to_owned())
    })
}

/// Fetch one tag's checksums once, pick the current asset name with a
/// legacy fallback, then fetch and verify exactly that asset. A checksum
/// mismatch aborts immediately: it is never retried as another name.
fn stage_tagged(tag: &str) -> Result<(std::path::PathBuf, String, String)> {
    let primary = asset_name()?;
    let base = download_base();
    let repo = repo();
    let sums = fetch_text(&format!(
        "{base}/{repo}/releases/download/{tag}/SHA256SUMS.txt"
    ))?;
    let (asset, want) = match checksum_for(&sums, &primary) {
        Some(want) => (primary, want),
        None => {
            let legacy = legacy_asset_name(tag_version(tag))?;
            let want =
                checksum_for(&sums, &legacy).ok_or("checksum entry missing for this asset")?;
            (legacy, want)
        }
    };
    let bytes = fetch_bytes(&format!("{base}/{repo}/releases/download/{tag}/{asset}"))?;
    let dir = ensure_state_dir()?.join(tag);
    std::fs::create_dir_all(&dir)?;
    let staged = dir.join(&asset);
    let tmp = dir.join(format!("{asset}.part"));
    std::fs::write(&tmp, &bytes)?;
    if verify_sha256(&tmp, &want).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err("checksum mismatch: staged file deleted".into());
    }
    std::fs::rename(&tmp, &staged)?;
    Ok((staged, want, asset))
}

/// Stage a verified release asset under the state directory. Download only:
/// nothing is executed, replaced, or restarted here.
pub fn download(version: Option<String>, machine: bool) -> Result<()> {
    let tag = match version {
        Some(v) => v,
        None => latest_tag()?,
    };
    let (path, want, asset) = stage_tagged(&tag)?;
    if machine {
        println!(
            "{}",
            json!({"version":tag_version(&tag),"asset":asset,"path":path,"sha256":want})
        );
    } else {
        println!("staged {} (sha256 verified)", path.display());
    }
    Ok(())
}

fn updates_dir() -> Result<std::path::PathBuf> {
    Ok(crate::store::directory()?.join("updates"))
}

fn journal(outcome: &str, detail: &Value) {
    let Ok(dir) = ensure_state_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let line = json!({"outcome":outcome,"detail":detail}).to_string() + "\n";
    use std::io::Write as _;
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("install.log"))
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

/// Create the state tree the same way `Store::open` does: the top
/// directory is private (0700). Any updater path that writes under the
/// state directory must go through here first, otherwise a default-mode
/// directory would make every later `Store::open` refuse to start.
fn ensure_state_dir() -> Result<std::path::PathBuf> {
    let state = crate::store::directory()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if !state.exists() {
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700).recursive(true);
            builder.create(&state)?;
        }
        let meta = std::fs::symlink_metadata(&state)?;
        if !meta.is_dir() {
            return Err("state path must be a private directory".into());
        }
        if meta.permissions().mode() & 0o077 != 0 {
            return Err("state directory must have mode 0700".into());
        }
    }
    #[cfg(not(unix))]
    {
        let _ = std::fs::create_dir_all(&state);
    }
    Ok(state.join("updates"))
}

fn acquire_lock() -> Result<std::path::PathBuf> {
    let dir = ensure_state_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("update.lock");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(pid) = raw.trim().parse::<i64>() {
            if pid > 0 && crate::agent::process_alive(pid) {
                return Err(format!("another update is in progress (pid {pid})").into());
            }
        }
        let _ = std::fs::remove_file(&path);
    }
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(path)
}

fn verify_attestation(file: &std::path::Path) -> Result<()> {
    let status = std::process::Command::new("gh")
        .args([
            "attestation",
            "verify",
            &file.display().to_string(),
            "--repo",
            &repo(),
        ])
        .output()
        .map_err(|_| -> Box<dyn std::error::Error> {
            "release attestation could not be verified (gh CLI missing); refusing to install".into()
        })?;
    if !status.status.success() {
        return Err("release attestation verification failed; refusing to install".into());
    }
    Ok(())
}

fn quiesce(db: &crate::store::Store) -> Result<()> {
    for s in db.list_all()? {
        let id = s["id"].as_str().unwrap_or("");
        if let Some(turn) = db.pending(id)? {
            return Err(format!(
                "session {id} has an active turn ({turn}); resolve it or wait before updating"
            )
            .into());
        }
    }
    Ok(())
}

fn health_check(exe: &std::path::Path, expect_version: &str) -> Result<()> {
    let doctor = std::process::Command::new(exe)
        .args(["doctor", "--json"])
        .output()
        .map_err(|_| -> Box<dyn std::error::Error> { "health check could not run".into() })?;
    if !doctor.status.success() {
        return Err("health check failed: doctor rejected the install".into());
    }
    let status = std::process::Command::new(exe)
        .args(["update", "status", "--json"])
        .output()
        .map_err(|_| -> Box<dyn std::error::Error> { "health check could not run".into() })?;
    if !status.status.success() {
        return Err("health check failed: status rejected the install".into());
    }
    let info: Value =
        serde_json::from_slice(&status.stdout).map_err(|_| -> Box<dyn std::error::Error> {
            "health check returned invalid JSON".into()
        })?;
    if info["version"] != expect_version {
        return Err("health check failed: version mismatch after activation".into());
    }
    Ok(())
}

/// Full update: verify, stage, attest, quiesce, back up state, atomically
/// activate, health-check, and roll back on any failure. Only direct
/// installs are eligible; package-managed and source builds are refused
/// with guidance instead of being touched.
pub fn install(version: Option<String>, machine: bool) -> Result<()> {
    if install_receipt()["kind"] != "direct" {
        return Err("this copy was not installed by the installer (package manager or source build?); update it with its own tool instead of stainless update install".into());
    }
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or("cannot locate the installed binary")?
        .to_path_buf();
    let previous = dir.join("stainless.previous");
    let tag = match version {
        Some(v) => v,
        None => latest_tag()?,
    };
    let lock = acquire_lock()?;
    let done = |outcome: &str, detail: &Value| {
        let _ = std::fs::remove_file(&lock);
        journal(outcome, detail);
    };
    if parse_version(tag_version(&tag)) <= parse_version(env!("CARGO_PKG_VERSION")) {
        done("already-current", &json!({"version": tag_version(&tag)}));
        if machine {
            println!(
                "{}",
                json!({"version":tag_version(&tag),"activated":false,"reason":"already-current"})
            );
        } else {
            println!("Already on v{}; nothing to do", tag_version(&tag));
        }
        return Ok(());
    }
    let (staged, want, _asset) = stage_tagged(&tag)?;
    if let Err(e) = verify_attestation(&staged) {
        done(
            "aborted",
            &json!({"version":tag_version(&tag),"reason":e.to_string()}),
        );
        return Err(e);
    }
    let db = crate::store::Store::open()?;
    if let Err(e) = quiesce(&db) {
        done(
            "aborted",
            &json!({"version":tag_version(&tag),"reason":e.to_string()}),
        );
        return Err(e);
    }
    let backup_path = updates_dir()?.join(format!(
        "sessions-pre-{}-{}.sqlite3",
        tag_version(&tag),
        std::process::id()
    ));
    if let Err(e) = db.backup(&backup_path) {
        done(
            "aborted",
            &json!({"version":tag_version(&tag),"reason":e.to_string()}),
        );
        return Err(format!("state backup failed: {e}").into());
    }
    if previous.exists() {
        let _ = std::fs::remove_file(&previous);
    }
    if std::fs::rename(&exe, &previous).is_err() {
        done(
            "aborted",
            &json!({"version":tag_version(&tag),"reason":"cannot stage previous binary"}),
        );
        return Err("cannot replace the installed binary (read-only directory?)".into());
    }
    let activated = std::fs::rename(&staged, &exe).is_ok();
    if !activated {
        let _ = std::fs::rename(&previous, &exe);
        done(
            "rolled-back",
            &json!({"version":tag_version(&tag),"reason":"activation rename failed"}),
        );
        return Err("activation failed; previous binary restored".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    if let Err(e) = health_check(&exe, tag_version(&tag)) {
        let _ = std::fs::rename(&exe, dir.join("stainless.failed"));
        let _ = std::fs::rename(&previous, &exe);
        done(
            "rolled-back",
            &json!({"version":tag_version(&tag),"reason":e.to_string()}),
        );
        return Err(format!("{e}; previous binary restored").into());
    }
    done(
        "installed",
        &json!({"from":env!("CARGO_PKG_VERSION"),"to":tag_version(&tag),"sha256":want}),
    );
    if machine {
        println!(
            "{}",
            json!({"version":tag_version(&tag),"activated":true,"previous":"stainless.previous"})
        );
    } else {
        println!(
            "Updated to v{} (previous binary kept as stainless.previous; `stainless update rollback` to revert)",
            tag_version(&tag)
        );
    }
    Ok(())
}

/// Restore the previous binary kept by `update install`.
pub fn rollback(machine: bool) -> Result<()> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or("cannot locate the installed binary")?
        .to_path_buf();
    let previous = dir.join("stainless.previous");
    if !previous.is_file() {
        return Err("no previous binary to roll back to".into());
    }
    let lock = acquire_lock()?;
    let failed = dir.join("stainless.failed");
    let _ = std::fs::remove_file(&failed);
    if std::fs::rename(&exe, &failed).is_err() || std::fs::rename(&previous, &exe).is_err() {
        let _ = std::fs::rename(&failed, &exe);
        let _ = std::fs::remove_file(&lock);
        return Err("rollback failed; install left untouched".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&lock);
    journal("rolled-back", &json!({}));
    if machine {
        println!("{}", json!({"rolled_back": true}));
    } else {
        println!("Rolled back; the replaced binary is kept as stainless.failed");
    }
    Ok(())
}

pub fn status(machine: bool) -> Result<()> {
    let info = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "channel": channel(),
        "install": install_receipt(),
    });
    if machine {
        println!("{info}");
    } else {
        println!(
            "stainless v{} · channel {} · install: {}",
            info["version"].as_str().unwrap(),
            info["channel"].as_str().unwrap(),
            info["install"]["kind"].as_str().unwrap()
        );
    }
    Ok(())
}
