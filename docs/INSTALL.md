# Install StainlessSteel

One binary, many doors. Every method below installs the same checksum-verified
release assets from GitHub. Source-available under PolyForm Strict 1.0.0 —
free for noncommercial use; no redistribution or commercial use without
separate written permission (see `LICENSE`).

| Method | Command |
|---|---|
| curl (Linux) | `curl -fsSL https://raw.githubusercontent.com/KotalaKishanReddy/StainlessSteel/main/install.sh \| sh` |
| Homebrew (macOS/Linux) | `brew install KotalaKishanReddy/stainlesssteel/stainlesssteel` |
| npm | `npm install -g stainlesssteel` |
| bun | `bun pm trust stainlesssteel && bun add -g stainlesssteel` |
| pnpm | `pnpm add -g --allow-build=stainlesssteel stainlesssteel` |
| yarn classic | `yarn global add stainlesssteel` |
| AUR (Arch) | `yay -S stainless-steel-bin` (or `paru -S stainless-steel-bin`) |
| mise | `mise use -g ubi:KotalaKishanReddy/StainlessSteel` |
| Manual | Download asset + `SHA256SUMS.txt` from [Releases](https://github.com/KotalaKishanReddy/StainlessSteel/releases), verify, place on `PATH` |
| Source | `git clone … && cargo build --release --locked` (Rust 1.88.0) |

After installing, check the channel and provenance:

```sh
stainless update check     # ask the release channel for anything newer
stainless update status    # version, channel, install kind
```

## curl

The installer detects x86_64 vs ARM64, downloads that asset for the latest
release (override with `STAINLESS_VERSION=vX.Y.Z`), verifies SHA-256, installs
to `~/.local/bin` (override with `PREFIX`), and writes an install receipt
that `stainless update status` reports. macOS users: build from source or use
Homebrew below — there is no Linux-static-on-macOS hack here.

## Homebrew

```sh
brew install KotalaKishanReddy/stainlesssteel/stainlesssteel
stainless doctor
```

The tap's formula pins the release asset and its SHA-256. Updates flow
through `brew upgrade` — and `stainless update install` deliberately refuses
Homebrew-owned copies instead of overwriting the package manager.

## npm / bun / pnpm / yarn

All four managers consume the same [`stainlesssteel`](https://www.npmjs.com/package/stainlesssteel)
package. Its postinstall downloads the platform binary for your machine,
verifies SHA-256, and links a `stainless` shim. Node ≥ 18 required.
`STAINLESS_VERSION` overrides the bundled version for testing.

- **npm** runs the postinstall directly (it prints a scripts notice first).
- **bun** blocks postinstalls by default: trust the package first, as shown
  in the table, then install.
- **pnpm** blocks build scripts by default: pass `--allow-build=stainlesssteel`
  (or run the interactive `pnpm approve-builds -g` once).
- **Yarn Classic** runs the postinstall with no extra step. Yarn Berry
  applies its own script policy — allow `stainlesssteel` when prompted.

## AUR

```sh
yay -S stainless-steel-bin
```

The `-bin` package repacks the official release binary with checksum
verification; no compilation, no vendored surprises.

## mise

```sh
mise use -g ubi:KotalaKishanReddy/StainlessSteel
mise x stainlesssteel@latest -- stainless --version
```

mise resolves the latest GitHub release and picks the asset matching your
OS/architecture automatically. mise holds brand-new releases for 24 hours by
default (`minimum_release_age`); install a pinned version such as
`ubi:KotalaKishanReddy/StainlessSteel@0.2.3` on day zero, or plain `@latest`
after the quarantine lifts. The installed binary is named `StainlessSteel`
(after the repo); call it by that exact name or alias it to `stainless`.

## Manual

Pick your asset from the latest
[release](https://github.com/KotalaKishanReddy/StainlessSteel/releases)
(`stainless-linux-x86_64`, `stainless-linux-aarch64`,
`stainless-macos-aarch64`), plus `SHA256SUMS.txt`:

```sh
sha256sum -c SHA256SUMS.txt   # must say OK for your asset
install -m 755 stainless-linux-x86_64 ~/.local/bin/stainless
```

Manual copies have no install receipt, so `stainless update status` reports
`unknown` and `stainless update install` refuses them — use the curl
installer if you want managed updates.

## Source

```sh
git clone https://github.com/KotalaKishanReddy/StainlessSteel.git
cd StainlessSteel
cargo build --release --locked   # Rust 1.88.0, see rust-toolchain.toml
./target/release/stainless doctor --json
```

Source builds are `unknown` provenance by design: update them with `git pull`
and a rebuild, never with `stainless update install`.

## After installing

Set a model and take it for a spin against a local OpenAI-compatible server:

```sh
export STAINLESS_MODEL=smollm2:135m   # served on 127.0.0.1:11434
stainless run "Explain Rust's ownership model in one paragraph."
stainless steel                        # terminal session manager
```
