# StainlessSteel — Dependency Licensing & Release Compatibility Audit

**Audit as of 20 November 2026.** This is **source/repository-level triage**, not a pinned dependency or final artifact audit. **No complete `Cargo.lock` or finished product exists.** A current upstream license cannot establish the license of an unpinned earlier commit.

## Rule

First-party StainlessSteel code: **PolyForm Strict 1.0.0**. Each third-party component retains its own license. MIT, Apache-2.0 and OFL materials may generally be incorporated into a restrictive application subject to their exact conditions; they cannot be relicensed as exclusively PolyForm Strict. GPL/AGPL and other reciprocal licenses need a build/linking/distribution-specific legal assessment. Unknown/no-license code is blocked.

| Component | Repository | License at reviewed source | Role | Evidence state | Delivery boundary | Required action |
|---|---|---|---|---|---|---|
| Rune | anomalyco/rune | MIT | Native GUI | Source verified | Link/library | Keep MIT notice; pin SHA |
| Infinity | hydro-project/infinity | Apache-2.0 | Subagents | Source verified | Library/runtime | Whitelisted star exception; keep NOTICE/license |
| Obscura | h4ckf0r0day/obscura | Apache-2.0 | Browser | Source verified | External executable/MCP | Audit exact binary, notices & assets |
| Rig | 0xPlaygrounds/rig | MIT | Model/tools | Source verified | Library | Retain MIT notice |
| MCP Rust SDK | modelcontextprotocol/rust-sdk | MIT historical + Apache-2.0 current | MCP | Mixed history verified | Library | Review exact snapshot; preserve both as applicable |
| ACP Rust SDK | agentclientprotocol/rust-sdk | Apache-2.0 | ACP | Source verified | Library | Star-policy exception; retain notice |
| Ratatui | ratatui/ratatui | MIT | Terminal UI | Source verified | Library | Retain MIT notice |
| RTK OSS edition | rtk-ai/rtk | Apache-2.0 OSS only | Token-light output | Edition split verified | Optional CLI | Exclude BUSL engine and proxy; verify compiled edition |
| ripgrep | BurntSushi/ripgrep | MIT OR Unlicense | Search | Source verified | Library/CLI | Record chosen license branch |
| ast-grep | ast-grep/ast-grep | MIT | AST search | Source verified | Library/CLI | Keep MIT notice |
| Tree-sitter | tree-sitter/tree-sitter | MIT | Parser | Source verified | Library | Grammars each require separate audit |
| Tantivy | quickwit-oss/tantivy | MIT | Full text | Source verified | Optional library | Keep MIT text and notices |
| LanceDB | lancedb/lancedb | Apache-2.0 | Vector retrieval | Source verified | Optional library | Review transitive Arrow/Lance graph |
| Gitoxide | GitoxideLabs/gitoxide | MIT OR Apache-2.0 | Git | Source verified | Library | Record selected branch |
| Wasmtime | bytecodealliance/wasmtime | Apache-2.0 | Plugins | Source verified | Optional library | Check transitive licenses |
| Tokio | tokio-rs/tokio | MIT | I/O | Verify pinned | Library | Audit Cargo.lock |
| Tracing | tokio-rs/tracing | MIT | Telemetry | Verify pinned | Library | Audit Cargo.lock |
| rusqlite | rusqlite/rusqlite | MIT (upstream wrapper); SQLite public-domain core, extensions vary | SQLite | Verify pinned | Library | Inspect SQLite/native features/extensions |
| async-openai | 64bit/async-openai | Verify exact license | OpenAI adapter | Not verified | Optional library | Pin and inspect license |
| mistral.rs | EricLBuehler/mistral.rs | Verify exact license | Local inference | Not verified | Separate process | Optional external, not core |
| cargo-deny | EmbarkStudios/cargo-deny | MIT OR Apache-2.0 | Audit | Verify pinned | Build tool | Does not enter distributed app unless bundled |
| cargo-about | EmbarkStudios/cargo-about | MIT OR Apache-2.0 | Notices generation | Source verified | Build tool | Generate exact notices |
| cargo-dist | axodotdev/cargo-dist | MIT OR Apache-2.0 | Release | Source verified | Build tool | Inspect binaries and installers |
| JetBrains Mono | JetBrains/JetBrainsMono | OFL-1.1 | GUI font | Source verified | Bundled assets only if needed | Bundle license; preserve reserved-font naming |
| IBM Plex | IBM/plex | OFL-1.1 | GUI font | Source verified | Bundled assets only if needed | Bundle license; preserve reserved-font naming |
| Code grammars | various upstreams | Varies | Parser language support | Unverified | Optional assets | Per-grammar license and generated code audit |
| Third-party MCP servers | various upstreams | Varies | Extensions | Unverified | User-installed or optional bundles | Avoid automatically packaging unreviewed plugins |
| Model weights/datasets | various upstreams | Varies; sometimes use-restricted | AI/models | Unverified | Never core | Independent licensing/acceptable use review |
| Tare | mstuart/tare | Excluded | Compaction | BANNED | None | Do not integrate |
| RTK BUSL engine | rtk-ai/rtk/engine | BUSL-1.1 | Compression | Source verified restricted | Excluded | Must not ship without written clearance |

## Current blockers

1. **No reproducible build/SBOM.** All binary dependencies and vendored source remain unverified.
2. **MCP history:** its repository contains an MIT-to-Apache license transition. Record all applicable notices from the exact pinned commit.
3. **RTK engine:** non-OSS BUSL code must not enter the binary or installer.
4. **Tree-sitter grammars, native libs, fonts and assets:** their licenses are separate from framework license.
5. **GPL, AGPL, LGPL, MPL, EPL, CDDL or custom reciprocal dependencies:** block pending written legal compatibility determination with precise link/modify/distribute facts.
6. **GitHub public fork rights and AI data terms:** separate from copyright; counsel should review platform implications if restricting AI training is an objective.
7. **Contributors:** signed individual licensing permission and reviewed contributor agreement required before importing code.
8. **Distribution:** a distro may freely distribute a permissively licensed dependency independently, but cannot redistribute the combined StainlessSteel product without a separate StainlessSteel license.

## Sources inspected

- PolyForm Strict: https://polyformproject.org/licenses/strict/1.0.0
- PolyForm text reuse: https://github.com/polyformproject/polyform-licenses/blob/1.0.0/LICENSE.md
- GitHub ToS: https://docs.github.com/en/site-policy/github-terms/github-terms-of-service
- Linux Foundation policy: https://www.linuxfoundation.org/licensebestpractices
- Rune: https://github.com/anomalyco/rune
- Infinity: https://github.com/hydro-project/infinity
- Obscura: https://github.com/h4ckf0r0day/obscura/blob/main/LICENSE
- Rig: https://github.com/0xPlaygrounds/rig
- MCP rust SDK: https://github.com/modelcontextprotocol/rust-sdk/blob/main/LICENSE
- ACP SDK: https://github.com/agentclientprotocol/rust-sdk/blob/main/LICENSE
- RTK edition breakdown: https://github.com/rtk-ai/rtk/blob/develop/docs/guide/resources/editions.md
- Ratatui: https://github.com/ratatui/ratatui
- ripgrep: https://github.com/BurntSushi/ripgrep/blob/master/COPYING
- ast-grep: https://github.com/ast-grep/ast-grep/blob/main/LICENSE
- Tree-sitter: https://github.com/tree-sitter/tree-sitter
- Tantivy: https://github.com/quickwit-oss/tantivy/blob/main/LICENSE
- LanceDB: https://github.com/lancedb/lancedb
- Gitoxide: https://github.com/GitoxideLabs/gitoxide
- Wasmtime: https://github.com/bytecodealliance/wasmtime
- IBM Plex: https://github.com/IBM/plex/blob/master/LICENSE.txt
- JetBrains Mono: https://github.com/JetBrains/JetBrainsMono

## Not a legal opinion

All statuses are preliminary, and every release must recheck pins and artifacts.
