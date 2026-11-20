# First-party source-available / third-party license compliance policy

## Outbound licensing

1. First-party Rust harness application, UX assets and documentation owned by the rights holder: **PolyForm Strict 1.0.0**.
2. Never claim ownership of upstream third-party source. Retain the upstream license, patent and notice obligations.
3. A separate signed license is required for proprietary/commercial use or redistribution of the *first-party StainlessSteel product* outside PolyForm Strict's grant.
4. The original license text in `LICENSE.md` must not be changed; official PolyForm license text may be reused. Copyright owner identification goes in `COPYRIGHT_AND_SCOPE.md`.
5. Compliance is not equivalent to restricting every file in a combined distribution. Permissively licensed components keep their independent freedoms.

## Inbound licensing

Do not accept code contributions until a contributor agreement and narrow development grant are approved and signed. A DCO alone certifies provenance but does not by itself solve the rights needed for proprietary/source-available relicensing.

## Dependency pipeline

- Require every *direct* external repository to have >=1,000 public GitHub stars unless explicitly approved: Infinity; ACP Rust SDK; selected reputable official SDKs/standards maintainers. Tare is prohibited.
- Pin versions and feature flags and use `--locked` builds; record `Cargo.lock` + build inputs.
- Run `cargo deny check licenses advisories sources bans` after adapting configuration to workspace.
- Generate `cargo about` notices and a CycloneDX SBOM, then verify binaries/files/fonts that Cargo cannot see.
- Fail closed on `GPL-*`, `AGPL-*`, unknown, missing-license, unreviewed `BUSL-*`, `SSPL-*` and other use-restricted/strong copyleft dependencies unless a lawyer signs an explicit compatibility decision.
- For LGPL/MPL/EPL/CDDL: permit only after targeted legal review of source disclosure/relinking/file-level conditions.
- Check submodules, build scripts, generated code, native dependencies, grammar data, plugin bundles, model assets, installers and update artifacts.
- Review MIT notices, Apache-2.0 LICENSE/NOTICE and change notices, OFL font rights/reserved names.
- Never treat an independently installed or remote service as automatically part of the distributable application; still comply when you redistribute it.

## Cargo manifest for first-party crates

The application should not be published to crates.io automatically. In each first-party manifest, use the correct relative path to the unchanged license file, for example:

```toml
[package]
name = "stainlesssteel-core"
version = "0.1.0"
publish = false
license-file = "../../LICENSE.md"  # adjust relative to this Cargo.toml
```

Do not assume that every registry or scanner recognizes the PolyForm Strict SPDX expression. Validate metadata and license discovery using the actual build and use the `license-file` fallback if needed. Third-party crates retain their independent `license` metadata.

## CI steps (illustrative; not ready-to-run without workspace)

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo deny check licenses advisories sources bans
cargo about generate about.hbs > dist/THIRD_PARTY_LICENSES.html
cargo cyclonedx --format json
# Independently scan included executables, fonts, grammars, licenses and notices
# Validate license audit for exact commit SHA and release artifacts
```

## Preserve repo access distinctions

A source-available license restricts your copyright, but does not override GitHub's public view/fork terms, third-party licenses, public-domain materials, statutory exceptions, or independently developed implementations of an idea.
