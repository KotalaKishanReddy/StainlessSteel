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

fn asset_name() -> Result<String> {
    let version = env!("CARGO_PKG_VERSION");
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok(format!("stainless-{version}-linux-x86_64")),
        ("linux", "aarch64") => Ok(format!("stainless-{version}-linux-aarch64")),
        (os, arch) => {
            Err(format!("no prebuilt asset for {os}/{arch} yet; build from source").into())
        }
    }
}

/// Stage a verified release asset under the state directory. Download only:
/// nothing is executed, replaced, or restarted here. Activation with
/// signature checks and rollback belongs to the updater workstream.
pub fn download(version: Option<String>, machine: bool) -> Result<()> {
    let tag = match version {
        Some(v) => v,
        None => latest_tag()?,
    };
    let asset = asset_name()?;
    let base = download_base();
    let repo = repo();
    let sums = fetch_text(&format!(
        "{base}/{repo}/releases/download/{tag}/SHA256SUMS.txt"
    ))?;
    let want = sums
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let file = parts.next()?;
            (file == asset).then(|| hash.to_owned())
        })
        .ok_or("checksum entry missing for this asset")?;
    let bytes = fetch_bytes(&format!("{base}/{repo}/releases/download/{tag}/{asset}"))?;
    let dir = crate::store::directory()?.join("updates").join(&tag);
    std::fs::create_dir_all(&dir)?;
    let staged = dir.join(&asset);
    let tmp = dir.join(format!("{asset}.part"));
    std::fs::write(&tmp, &bytes)?;
    if verify_sha256(&tmp, &want).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err("checksum mismatch: staged file deleted".into());
    }
    std::fs::rename(&tmp, &staged)?;
    if machine {
        println!(
            "{}",
            json!({"version":tag_version(&tag),"asset":asset,"path":staged,"sha256":want})
        );
    } else {
        println!(
            "staged {} (sha256 verified; activation is not implemented yet)",
            staged.display()
        );
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
