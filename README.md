# StainlessSteel

**a harness built with rust**

A local-first coding assistant for Linux and macOS development. Chat with a locally served model, send code or a diff for review, and pick up the same conversation after closing the terminal. The executable is `stainless`. This repository contains the first development build from the [revision 1.2 blueprint](docs/STAINLESSSTEEL_MASTER_BLUEPRINT.md).

## Install (Linux)

```sh
curl -fsSL https://raw.githubusercontent.com/KotalaKishanReddy/StainlessSteel/main/install.sh | sh
```

Installs the prebuilt static binary for your architecture (x86_64 or ARM64) into `~/.local/bin`, verifying its SHA-256 checksum first. macOS users: build from source below.

## Build and try it

Install Rust via rustup and a C compiler for bundled SQLite. On macOS the Xcode Command Line Tools supply the compiler. From the repository root:

```sh
cargo build --release --locked
./target/release/stainless doctor --json
./target/release/stainless session new --name "first review" --workspace .
./target/release/stainless session resume last
./target/release/stainless steel
```

The session manager and history work offline. In the TUI, press **Ctrl+N** to create a session, **Tab** to switch sessions, **Enter** to send a message, and **Esc** to quit. Set the model name before launching the TUI to enable chat.

## Run a model

Start an OpenAI-compatible chat server on the same computer and load a model. For example, if Ollama is installed, use `ollama run smollm2:135m` once to load the model. The following shell commands use its default loopback port. For llama.cpp, start `llama-server` and pass its port with `--port` and its model alias with `--model`.

```sh
export PATH="$PWD/target/release:$PATH"
export STAINLESS_MODEL=smollm2:135m
stainless run --session last "Explain Rust's ownership model in one paragraph."
git diff --no-ext-diff | stainless review --session last
stainless run --session last --json "Which change would you test first?"
stainless session checkpoint last
stainless session export last > transcript.json
```

The diff example needs a nonempty working-tree diff. You can also pipe a source file directly into `stainless review`. Each run creates a session unless you specify `--session`. JSONL output contains a session ID, streamed deltas and a final completion or model-error record. A delta is provisional until completion. Help and argument errors use ordinary CLI diagnostics.

## Journal and recovery

Each session has a stable ID, name, workspace and ordered event sequence. SQLite uses WAL and full synchronous commits. Model requests are sent to `127.0.0.1`. The client transmits the current prompt and up to 32 completed turns within a 256 KiB context budget. Full transcripts remain available locally. Prompts, source sent for review, and replies are stored as plaintext in a private state directory. Keep credentials out of prompts. Model replies are displayed as text. They have no shell or filesystem execution privileges.

```sh
stainless session list --json
stainless session rename last "parser review"
stainless session recovery inspect last
stainless backup create ./stainless-backup.sqlite3
stainless backup verify ./stainless-backup.sqlite3
```

If an operating-system kill leaves a pending turn, first confirm the original process is gone. Use the exact ID and turn number from inspection with `stainless session recovery resolve SESSION_ID --turn NUMBER`. This marks the turn interrupted and lets you submit a new request. A second process cannot start another turn on a session with a pending request. Separate sessions can run concurrently.

Set `STAINLESS_HOME` to choose a private directory. Otherwise the journal lives in `$XDG_STATE_HOME/stainless` or `~/.local/state/stainless`. A verified snapshot can be inspected independently by copying it to `sessions.sqlite3` in a **new** mode-0700 state directory and starting the CLI with that directory. The backup command refuses to overwrite an existing path.

## Build status

| Capability | Development build |
|---|---|
| CLI, Ratatui session manager, JSONL, local chat and source review | Implemented |
| SQLite session isolation, checkpoint markers, transcript replay, recovery and verified backups | Implemented |
| Linux x86-64 | Built and exercised with a real local model |
| Linux ARM64 | Static binary built. Session and backup smoke tests passed under QEMU |
| macOS | CI job prepared. Native execution pending |
| Cloud providers, vault, automated workspace edits and sandboxed test tools | Planned |
| Rune desktop, Infinity, Obscura, MCP/ACP, plugins and signed updater | Planned |

Run `cargo test --locked` for regression coverage. GitHub Actions runs formatting, Clippy, tests, a release build and a doctor smoke test. [Build evidence](docs/BUILD_EVIDENCE.md) records the checks run on this revision.

## License: source-available

StainlessSteel — *a harness built with rust* — is **source-available under PolyForm Strict License 1.0.0** for original StainlessSteel code (not an OSI-approved open-source project).

- You may inspect the source and use StainlessSteel for purposes permitted by PolyForm Strict.
- The license **does not grant general redistribution or modification rights**.
- Commercial/proprietary use, reselling, distribution, hosting as a service, and publication of modified copies require a separate **written license**, unless independently permitted by applicable law.
- Third-party libraries and fonts retain their own licenses and rights. See [Third-Party Notices](THIRD_PARTY_NOTICES.md).
- Public GitHub forks are possible under GitHub's separate platform terms; a fork does not confer additional permissions to distribute or exploit StainlessSteel outside those terms.

License: [PolyForm Strict 1.0.0](LICENSE) · [Rights & scope](COPYRIGHT_AND_SCOPE.md) · [Request permission](COMMERCIAL_AND_REDISTRIBUTION.md) · [Contributing](CONTRIBUTING.md)

Run `stainless licenses` for the locked third-party inventory, or `stainless licenses --full` to print available upstream license texts.

This is a development snapshot (v0.1.0). The [pre-release checklist](docs/PRE_RELEASE_LEGAL_CHECKLIST.md) still requires verified ownership, contact details, legal review and a final distribution audit.
