use crate::Result;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn workspace_dir(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
        return Err("workspace must be a directory".into());
    }
    Ok(canonical)
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|_| -> Box<dyn std::error::Error> { "git executable not found".into() })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if detail.is_empty() {
            "git command failed".into()
        } else {
            detail.into()
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
