use crate::{store::Store, Result};
use std::process::Command;

pub fn max_agents() -> i64 {
    std::env::var("STAINLESS_MAX_AGENTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n >= 0)
        .unwrap_or(4)
}

#[cfg(unix)]
pub fn process_alive(pid: i64) -> bool {
    Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn process_alive(pid: i64) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .any(|l| l.contains(&pid.to_string()))
        })
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn kill_process(pid: i64) -> Result<()> {
    let status = Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .status()
        .map_err(|_| -> Box<dyn std::error::Error> { "could not signal agent process".into() })?;
    if !status.success() {
        return Err("agent process is already gone".into());
    }
    Ok(())
}

#[cfg(windows)]
pub fn kill_process(pid: i64) -> Result<()> {
    let status = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .status()
        .map_err(|_| -> Box<dyn std::error::Error> { "could not signal agent process".into() })?;
    if !status.success() {
        return Err("agent process is already gone".into());
    }
    Ok(())
}

/// Reconcile every running agent against its process and turn.
/// A dead process never replays: its turn is marked interrupted when still
/// pending, and the agent is recorded as failed.
pub fn refresh(db: &mut Store) -> Result<()> {
    for snapshot in db.agents(None)? {
        if snapshot["status"] != "running" {
            continue;
        }
        let id = snapshot["id"].as_str().unwrap().to_owned();
        let session = snapshot["session"].as_str().unwrap().to_owned();
        let pid = snapshot["pid"].as_i64().unwrap_or(-1);
        let turn = snapshot["turn"].as_i64().unwrap_or(-1);
        if pid > 0 && process_alive(pid) {
            continue;
        }
        if turn < 0 {
            db.set_agent_status(&id, "failed")?;
            continue;
        }
        let complete = matches!(db.turn_status(&session, turn)?, Some(s) if s == "complete");
        if !complete && db.pending(&session)? == Some(turn) {
            db.finish(&session, turn, None)?;
        }
        db.set_agent_status(&id, if complete { "done" } else { "failed" })?;
    }
    Ok(())
}
