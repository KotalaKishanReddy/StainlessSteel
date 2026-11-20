#!/bin/sh
# StainlessSteel installer — downloads the right static Linux binary for your
# machine, verifies its SHA-256 checksum, and installs it.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/KotalaKishanReddy/StainlessSteel/main/install.sh | sh
#
# Environment:
#   STAINLESS_VERSION  release tag to install (default: v0.1.0)
#   PREFIX             install directory (default: $HOME/.local/bin)
#
# StainlessSteel is source-available under PolyForm Strict 1.0.0. By
# downloading and running this software you agree to the license terms in
# https://github.com/KotalaKishanReddy/StainlessSteel/blob/main/LICENSE.md
set -eu

REPO="KotalaKishanReddy/StainlessSteel"
VERSION="${STAINLESS_VERSION:-v0.1.0}"
PREFIX="${PREFIX:-$HOME/.local/bin}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command '$1' not found" >&2
    exit 1
  }
}

# Linux only: the published binaries are statically linked for Linux.
OS="$(uname -s)"
if [ "$OS" != "Linux" ]; then
  echo "error: no prebuilt binary for $OS — build from source instead:" >&2
  echo "  git clone https://github.com/$REPO.git && cd StainlessSteel && cargo build --release --locked" >&2
  exit 1
fi

# Pick the asset matching this machine.
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64 | amd64) ASSET_ARCH="x86_64" ;;
  aarch64 | arm64) ASSET_ARCH="aarch64" ;;
  *)
    echo "error: unsupported architecture '$ARCH' (need x86_64 or aarch64)" >&2
    exit 1
    ;;
esac

need curl
need sha256sum

ASSET="stainless-${VERSION#v}-linux-${ASSET_ARCH}"
BASE="https://github.com/$REPO/releases/download/$VERSION"

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT INT TERM

echo "Downloading $ASSET ($VERSION) ..."
curl -fsSL -o "$TMPDIR/$ASSET" "$BASE/$ASSET"
curl -fsSL -o "$TMPDIR/SHA256SUMS.txt" "$BASE/SHA256SUMS.txt"

echo "Verifying checksum ..."
(cd "$TMPDIR" && sha256sum -c SHA256SUMS.txt --status --ignore-missing) || {
  echo "error: checksum verification failed" >&2
  exit 1
}

mkdir -p "$PREFIX"
install -m 755 "$TMPDIR/$ASSET" "$PREFIX/stainless"

echo "Installed stainless to $PREFIX/stainless"
"$PREFIX/stainless" doctor --json

case ":$PATH:" in
  *":$PREFIX:"*) ;;
  *)
    echo "Note: $PREFIX is not on your PATH. Add this to your shell profile:" >&2
    echo "  export PATH=\"$PREFIX:\$PATH\"" >&2
    ;;
esac
