use crate::Result;
use serde_json::{json, Value};
use std::time::Duration;

const DEFAULT_REPO: &str = "KotalaKishanReddy/StainlessSteel";

fn repo() -> String {
    std::env::var("STAINLESS_REPO").unwrap_or_else(|_| DEFAULT_REPO.into())
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
        .get(&format!(
            "https://api.github.com/repos/{}/releases/latest",
            repo()
        ))
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
