use serde_json::{json, Value};

/// A native tool the broker may execute. This registry is the uniform,
/// documented tool API that Code Mode-style callers program against:
/// instead of one model turn per tool call, a caller receives these
/// descriptors, writes a single batch program, and the broker runs it
/// and returns only the final outputs. No credentials ever appear in a
/// descriptor, an argument, or a result.
pub struct ToolDef {
    pub id: &'static str,
    pub description: &'static str,
    pub risk: &'static str,
    pub input_schema: Value,
}

fn schema(props: &[(&str, &str)], required: &[&str]) -> Value {
    let properties: serde_json::Map<String, Value> = props
        .iter()
        .map(|(name, kind)| (name.to_string(), json!({"type": kind})))
        .collect();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

fn def(
    id: &'static str,
    description: &'static str,
    risk: &'static str,
    input_schema: Value,
) -> ToolDef {
    ToolDef {
        id,
        description,
        risk,
        input_schema,
    }
}

pub fn descriptor(tool: &ToolDef) -> Value {
    json!({"id":tool.id,"description":tool.description,"risk":tool.risk,"input_schema":tool.input_schema})
}

pub fn registry() -> Vec<ToolDef> {
    let session = schema(&[("id", "string")], &[]);
    let session_named = schema(&[("name", "string"), ("workspace", "string")], &[]);
    let workspace = schema(&[("workspace", "string")], &[]);
    vec![
        def(
            "session.new",
            "Open a durable session in a workspace.",
            "write:journal",
            session_named.clone(),
        ),
        def(
            "session.list",
            "List active sessions, newest first.",
            "read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "session.resume",
            "Replay the transcript of a session.",
            "read",
            session.clone(),
        ),
        def(
            "session.rename",
            "Rename a session.",
            "write:journal",
            schema(&[("id", "string"), ("name", "string")], &["id", "name"]),
        ),
        def(
            "session.checkpoint",
            "Record a checkpoint marker in a session.",
            "write:journal",
            session.clone(),
        ),
        def(
            "session.export",
            "Export a session transcript as JSON.",
            "read",
            session.clone(),
        ),
        def(
            "session.fork",
            "Copy a transcript into a new session; turn state is not carried over.",
            "write:journal",
            schema(&[("id", "string"), ("at", "integer")], &["id"]),
        ),
        def(
            "session.import",
            "Import a session export file as a new session with provenance.",
            "write:journal",
            schema(
                &[
                    ("path", "string"),
                    ("name", "string"),
                    ("workspace", "string"),
                ],
                &["path"],
            ),
        ),
        def(
            "session.archive",
            "Archive a session so it leaves the active list.",
            "write:journal",
            session.clone(),
        ),
        def(
            "session.recovery.inspect",
            "Inspect the interrupted turn of a session.",
            "read",
            session.clone(),
        ),
        def(
            "session.recovery.resolve",
            "Mark an interrupted turn resolved; never replays it.",
            "write:journal",
            schema(&[("id", "string"), ("turn", "integer")], &["id", "turn"]),
        ),
        def(
            "repo.status",
            "Show working-tree status via system git.",
            "read",
            workspace.clone(),
        ),
        def(
            "repo.diff",
            "Show the uncommitted diff via system git.",
            "read",
            workspace.clone(),
        ),
        def(
            "repo.branch",
            "Show the current branch via system git.",
            "read",
            workspace.clone(),
        ),
        def(
            "repo.commit",
            "Commit staged changes with a message via system git.",
            "write:vcs",
            schema(
                &[("workspace", "string"), ("message", "string")],
                &["message"],
            ),
        ),
        def(
            "repo.worktree",
            "List, add, or remove isolated git worktrees for agents.",
            "write:vcs",
            workspace.clone(),
        ),
        def(
            "patch.apply",
            "Apply a unified diff to the workspace; rejects escapes and mismatches.",
            "write:files",
            workspace.clone(),
        ),
        def(
            "context.inspect",
            "Report event, turn, and byte counts against the token budget.",
            "read",
            session.clone(),
        ),
        def(
            "agent.spawn",
            "Launch a supervised worker running one turn in a session.",
            "execute:model",
            schema(
                &[
                    ("session", "string"),
                    ("prompt", "string"),
                    ("model", "string"),
                ],
                &["prompt"],
            ),
        ),
        def(
            "agent.list",
            "List supervised workers and their states.",
            "read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "agent.inspect",
            "Inspect one supervised worker.",
            "read",
            schema(&[("id", "string")], &["id"]),
        ),
        def(
            "agent.cancel",
            "Kill a running worker and mark its turn interrupted.",
            "execute:process",
            schema(&[("id", "string")], &["id"]),
        ),
        def(
            "agent.tree",
            "Show sessions with their supervised workers.",
            "read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "update.check",
            "Query the release channel for a newer version.",
            "network:read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "update.status",
            "Report version, channel, and install provenance.",
            "read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "update.download",
            "Stage a verified release asset; never activates it.",
            "network:read",
            schema(&[("version", "string")], &[]),
        ),
        def(
            "backup.create",
            "Write a verified portable snapshot of the journal.",
            "write:files",
            schema(&[("path", "string")], &["path"]),
        ),
        def(
            "backup.verify",
            "Verify a snapshot without touching live state.",
            "read",
            schema(&[("path", "string")], &["path"]),
        ),
        def(
            "run",
            "Send one turn to the local model for a session.",
            "execute:model",
            schema(
                &[
                    ("session", "string"),
                    ("prompt", "string"),
                    ("model", "string"),
                ],
                &["prompt"],
            ),
        ),
        def(
            "review",
            "Ask the local model to review piped source.",
            "execute:model",
            schema(&[("session", "string")], &[]),
        ),
        def(
            "doctor",
            "Check the state directory and journal health.",
            "read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "licenses",
            "Show first-party terms and the third-party inventory.",
            "read",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        def(
            "completions",
            "Print shell completions.",
            "read",
            schema(&[("shell", "string")], &["shell"]),
        ),
    ]
}
