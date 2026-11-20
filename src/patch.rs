use crate::Result;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub const MAX_PATCH_BYTES: usize = 1024 * 1024;

fn clean_path(raw: &str) -> Result<PathBuf> {
    let mut path = raw.to_owned();
    for prefix in ["a/", "b/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            path = rest.to_owned();
            break;
        }
    }
    if path == "/dev/null" {
        return Ok(PathBuf::from("/dev/null"));
    }
    let candidate = PathBuf::from(&path);
    if candidate.is_absolute() {
        return Err("patch must use workspace-relative paths".into());
    }
    if candidate
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return Err("patch path escapes the workspace".into());
    }
    Ok(candidate)
}

fn parse_hunk_header(line: &str) -> Result<(usize, usize)> {
    let inner = line
        .strip_prefix("@@ ")
        .and_then(|s| s.split_once(" @@").map(|(ranges, _)| ranges))
        .ok_or("malformed hunk header")?;
    let mut ranges = inner.split_whitespace();
    let old = ranges.next().ok_or("malformed hunk header")?;
    let old = old.strip_prefix('-').ok_or("malformed hunk header")?;
    let mut nums = old.split(',');
    let start: usize = nums
        .next()
        .ok_or("malformed hunk header")?
        .parse()
        .map_err(|_| -> Box<dyn std::error::Error> { "malformed hunk header".into() })?;
    let len: usize = match nums.next() {
        None => 1,
        Some(n) => n
            .parse()
            .map_err(|_| -> Box<dyn std::error::Error> { "malformed hunk header".into() })?,
    };
    Ok((start, len))
}

struct Hunk {
    old_start: usize,
    old_len: usize,
    body: Vec<(u8, String)>,
}

fn apply_file(workspace: &Path, old: &Path, new: &Path, hunks: &[Hunk]) -> Result<String> {
    let dev_null = Path::new("/dev/null");
    if old == dev_null && new == dev_null {
        return Err("patch names /dev/null on both sides".into());
    }
    let target = workspace.join(if old == dev_null { new } else { old });
    if target == *workspace {
        return Err("patch targets the workspace root".into());
    }
    let original = if old == dev_null {
        String::new()
    } else {
        fs::read_to_string(&target).map_err(|_| -> Box<dyn std::error::Error> {
            "patch target is missing or not UTF-8".into()
        })?
    };
    if old != dev_null {
        let canonical = target
            .canonicalize()
            .map_err(|_| -> Box<dyn std::error::Error> { "patch target vanished".into() })?;
        if !canonical.starts_with(workspace) {
            return Err("patch target escapes the workspace".into());
        }
    }
    let source: Vec<&str> = original.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut pos = 0usize;
    let mut trailing_newline = original.ends_with('\n') || original.is_empty();
    for hunk in hunks {
        let base = if hunk.old_start == 0 {
            0
        } else {
            hunk.old_start - 1
        };
        if base < pos || base > source.len() {
            return Err("patch hunks overlap or run past end of file".into());
        }
        out.extend(source[pos..base].iter().map(|s| s.to_string()));
        pos = base;
        let mut consumed = 0usize;
        let mut last_op = b' ';
        for (op, text) in &hunk.body {
            match op {
                b' ' => {
                    let current = source
                        .get(pos)
                        .ok_or("patch context runs past end of file")?;
                    if current != text {
                        return Err(format!("patch context mismatch near line {}", pos + 1).into());
                    }
                    out.push((*current).to_owned());
                    pos += 1;
                    consumed += 1;
                }
                b'-' => {
                    let current = source
                        .get(pos)
                        .ok_or("patch removal runs past end of file")?;
                    if current != text {
                        return Err(format!("patch removal mismatch near line {}", pos + 1).into());
                    }
                    pos += 1;
                    consumed += 1;
                }
                b'+' => out.push(text.clone()),
                b'\\' => {
                    if last_op == b'+' || last_op == b' ' {
                        trailing_newline = false;
                    }
                }
                _ => return Err("malformed patch body".into()),
            }
            if *op != b'\\' {
                last_op = *op;
            }
        }
        if consumed != hunk.old_len {
            return Err("patch hunk length disagrees with its header".into());
        }
    }
    out.extend(source[pos..].iter().map(|s| s.to_string()));
    if new == dev_null {
        fs::remove_file(&target).map_err(|_| -> Box<dyn std::error::Error> {
            "could not delete patch target".into()
        })?;
        return Ok(format!("deleted {}", target.display()));
    }
    let mut rendered = out.join("\n");
    if trailing_newline && !out.is_empty() {
        rendered.push('\n');
    }
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|_| -> Box<dyn std::error::Error> {
                "could not create patch directories".into()
            })?;
        }
    }
    fs::write(&target, rendered)
        .map_err(|_| -> Box<dyn std::error::Error> { "could not write patch target".into() })?;
    Ok(format!("updated {}", target.display()))
}

pub fn apply(workspace: &Path, text: &str) -> Result<Vec<String>> {
    if text.len() > MAX_PATCH_BYTES {
        return Err("patch exceeds the 1 MiB limit".into());
    }
    let workspace = workspace.canonicalize()?;
    if !workspace.is_dir() {
        return Err("workspace must be a directory".into());
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut changed = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let old_raw = line
            .strip_prefix("--- ")
            .ok_or("expected a --- file header")?
            .split('\t')
            .next()
            .unwrap_or("")
            .trim();
        i += 1;
        let next = lines.get(i).ok_or("patch truncates after the --- header")?;
        let new_raw = next
            .strip_prefix("+++ ")
            .ok_or("expected a +++ file header")?
            .split('\t')
            .next()
            .unwrap_or("")
            .trim();
        i += 1;
        let (old, new) = (clean_path(old_raw)?, clean_path(new_raw)?);
        let mut hunks = Vec::new();
        while i < lines.len() && lines[i].starts_with("@@ ") {
            let (old_start, old_len) = parse_hunk_header(lines[i])?;
            i += 1;
            let mut body = Vec::new();
            while i < lines.len() && !lines[i].starts_with("@@ ") && !lines[i].starts_with("--- ") {
                let entry = lines[i];
                if let Some(marker) = entry.strip_prefix('\\') {
                    let _ = marker;
                    body.push((b'\\', String::new()));
                } else {
                    let op = entry.as_bytes().first().copied().unwrap_or(b' ');
                    if !matches!(op, b' ' | b'-' | b'+') {
                        return Err("malformed patch body".into());
                    }
                    body.push((op, entry[1..].to_owned()));
                }
                i += 1;
            }
            hunks.push(Hunk {
                old_start,
                old_len,
                body,
            });
        }
        if hunks.is_empty() {
            return Err("patch file entry has no hunks".into());
        }
        changed.push(apply_file(&workspace, &old, &new, &hunks)?);
    }
    Ok(changed)
}
