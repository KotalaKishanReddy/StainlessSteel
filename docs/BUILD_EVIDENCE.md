# Development build evidence

20 November 2026. The native build ran on Debian 13, x86-64, using Rust 1.88.0 and the checked-in Cargo.lock. The test model and server were installed separately for validation.

| Check | Observed result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked` | Nine CLI integration tests passed |
| `cargo build --release --locked` | Produced a 2,887,568-byte GNU/Linux executable |
| Local model connection | SmolLM2-135M-Instruct Q4_K_M served by llama.cpp b12563. The CLI consumed a 3-delta greeting and a 255-delta source review in one session. Both streams completed. |
| Journal replay | A fresh process exported six ordered events: creation, user, assistant, user, assistant, checkpoint. The backup verified independently. |
| Interactive terminal | A 110 × 32 pseudo-terminal opened the Ratatui view. Ctrl+N created a session and Esc restored the original terminal settings. |
| Process kill | A test killed the CLI after its request reached a controlled server. The saved intent stayed visible, a competing turn was refused, and recovery emitted no second request. |
| Negative paths | Coverage includes a truncated stream, stale recovery ID, shared state directory, unsupported schema, unrelated backup database, non-TTY input, occupied backup path and missing session. |

The model weights had SHA-256 `2e8040ceae7815abe0dcb3540b9995eaa1fa0d2ca9e797d0a635ae4433c68c2d`. The native GNU/Linux binary had SHA-256 `790baf2e788f350ccb9ef1b2392d476f614a0f7e66ad517e2b28dd9f425f72c3`. Generated text and delta counts vary. The smoke test measures the transport, storage and replay path. Coding quality and throughput require separate benchmarks.

GitHub-hosted CI and native macOS verification remain pending. The project scope, legal file templates and future acceptance gates are recorded in the master blueprint and pre-release checklist.

## Portable Linux development binaries

The private download includes two statically linked musl executables. Rust 1.88.0 supplied the target standard libraries. Zig 0.13.0 compiled bundled SQLite. Both executables ran version, doctor, session creation, checkpoint, resume, backup and backup verification checks. The ARM64 smoke test used QEMU user-mode emulation on x86-64.

| Target | Bytes | SHA-256 |
|---|---:|---|
| x86_64-unknown-linux-musl | 3,182,496 | `8ea1eec7557fa4bb0d60076430fefbd9df6a135105f8e502c019f8a20950b49e` |
| aarch64-unknown-linux-musl | 2,863,984 | `8b75753750b8d5b0452f81e862d373050fae45b3178f81a2b5d75be4dcf903a0` |
