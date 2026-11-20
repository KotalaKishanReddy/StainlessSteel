# Direct dependencies

GitHub repository metadata checked 20 November 2026. Cargo.lock records exact registry versions and checksums. Bundled SQLite removes a runtime system SQLite dependency. On this revision the local model adapter uses unencrypted loopback HTTP and disables redirects and environment proxy discovery.

| Crate | Pinned version | Upstream | Stars at check | Declared license |
|---|---|---|---:|---|
| clap | 4.5.50 | [clap-rs/clap](https://github.com/clap-rs/clap) | 16,901 | MIT OR Apache-2.0 |
| rusqlite | 0.32.1 | [rusqlite/rusqlite](https://github.com/rusqlite/rusqlite) | 4,497 | MIT |
| serde_json | 1.0.145 | [serde-rs/json](https://github.com/serde-rs/json) | 5,674 | MIT OR Apache-2.0 |
| ratatui | 0.29.0 | [ratatui/ratatui](https://github.com/ratatui/ratatui) | 23,370 | MIT |
| crossterm | 0.28.1 | [crossterm-rs/crossterm](https://github.com/crossterm-rs/crossterm) | 4,268 | MIT |
| ureq | 2.12.1 | [algesten/ureq](https://github.com/algesten/ureq) | 2,234 | MIT OR Apache-2.0 |

See the [locked inventory](../THIRD_PARTY_NOTICES.md) and [source-based licensing assessment](DEPENDENCY_LICENSE_AUDIT.md). The build-specific inventory supersedes the pre-implementation package estimates. The inference demo used a separately installed server and weights.

The [CycloneDX source inventory](source-sbom.cdx.json) includes the full lock graph, registry checksums, and bundled SQLite. Target-specific final artifacts and the Rust toolchain still require a distribution audit.
