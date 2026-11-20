# Update pipeline

How StainlessSteel ships releases and how clients consume them. The client
half (`update check|status|download`) is implemented; activation with
signature checks and rollback is explicitly unwritten (see §5).

## 1. Release workflow (`.github/workflows/release.yml`)

Tag-driven: pushing `vX.Y.Z` builds, verifies, and publishes. No release is
cut from a plain push to `main` — CI on `main` only runs format, Clippy,
tests, a release build, and a doctor smoke test.

Matrix (Tier 1 targets from the blueprint):

| Runner | Target | Asset |
|---|---|---|
| ubuntu-22.04 | x86_64-unknown-linux-gnu | stainless-linux-x86_64 |
| ubuntu-24.04-arm | aarch64-unknown-linux-gnu | stainless-linux-aarch64 |
| macos-latest | aarch64-apple-darwin | stainless-macos-aarch64 |

Each job runs the full gate suite on its runner, smoke-tests `doctor --json`
and `licenses` on the built binary, then uploads the asset. Provenance is
attested with `actions/attest-build-provenance` (Sigstore, no long-lived
keys). The publish job collects assets, writes `SHA256SUMS.txt` and
`manifest.json`, and creates the GitHub Release. Tags containing `-`
(`v1.3.0-beta.1`) publish as pre-release on the `beta` channel.

macOS x86_64, musl statics, distro packages (`.deb`, `.rpm`, Homebrew tap),
and code signing/notarization are follow-ups recorded in the blueprint, not
part of this workflow yet.

## 2. Release manifest (`manifest.json`, schema 1)

```json
{
  "schema": 1,
  "project": "StainlessSteel",
  "version": "v0.1.0",
  "channel": "stable",
  "assets": [
    {"file": "stainless-linux-x86_64", "target": "x86_64-unknown-linux-gnu", "sha256": "…", "kind": "executable"}
  ]
}
```

Stable schema: clients may rely on field names. `channel` is `stable` or
`beta`. The manifest ships beside the assets in every release.

## 3. Client behavior

- `update check` resolves the latest tag for the configured channel
  (`STAINLESS_CHANNEL`, default `stable`; beta tags are ignored on stable)
  via the GitHub Releases API. HTTPS, 15 s timeout, proxy from environment,
  read-only: nothing is downloaded or executed.
- `update status` reports version, channel, and install kind. Install kind
  comes from an install receipt (`.stainless-install-receipt.json` next to
  the binary, written by `install.sh`): `direct`, otherwise `unknown`
  (source or manual install). No guessing from `PATH`.
- `update download [--version TAG]` stages one checksum-verified asset to
  `$STATE/updates/<tag>/`. It verifies SHA-256 against the release
  `SHA256SUMS.txt`, deletes the staged file on mismatch, and refuses
  platforms with no prebuilt asset. **It stops at staging.** There is no
  activation, restart, migration, or rollback step yet.

## 4. Channels

`STAINLESS_CHANNEL=stable` (default) tracks full releases; `beta` will track
pre-releases once the client learns to list them (`update check` currently
reads `releases/latest`, which only ever resolves stable). Channel support
in `check` is implemented; channel-aware listing is a follow-up.

## 5. What is deliberately missing

Per the blueprint's safe-update rules, the following must exist before any
`update install` ships, and none of it does: trusted code-signing keys and
signature/attestation verification on the client, an install receipt written
by every install path, a staged out-of-process activator (never overwrite
the running binary), transactional schema migration with a backward-compatible
read path, health checks with automatic rollback to the previous binary and
verified backup, package-manager awareness (brew/apt/dnf/pacman/nix defer to
their own tools), and single-flight locking for concurrent updaters. The
corresponding failure matrix (F.5 in the blueprint) is the acceptance test
for that work.
