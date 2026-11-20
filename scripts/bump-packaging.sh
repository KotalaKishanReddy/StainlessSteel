#!/bin/sh
# Regenerate Homebrew formula + AUR package from a published release.
# Usage: sh scripts/bump-packaging.sh vX.Y.Z
# Requires: gh (authenticated), sha256sum.
set -eu

VERSION="${1:?usage: bump-packaging.sh vX.Y.Z (e.g. v0.2.0)}"
REPO="KotalaKishanReddy/StainlessSteel"
TAG="$VERSION"

need() {
  command -v "$1" >/dev/null 2>&1 || { echo "error: missing $1" >&2; exit 1; }
}
need gh
need sha256sum

fetch_sha() {
  # $1 = asset name → prints its SHA-256 from the release checksums
  gh release download "$TAG" --repo "$REPO" --pattern SHA256SUMS.txt --output - 2>/dev/null \
    | awk -v asset="$1" '$2 == asset { print $1 }'
}

SHA_X64="$(fetch_sha stainless-linux-x86_64)"
SHA_ARM="$(fetch_sha stainless-linux-aarch64)"
SHA_MAC="$(fetch_sha stainless-macos-aarch64)"
[ -n "$SHA_X64" ] && [ -n "$SHA_ARM" ] && [ -n "$SHA_MAC" ] || {
  echo "error: could not read all checksums for $TAG" >&2
  exit 1
}
VER="${TAG#v}"
BASE="https://github.com/$REPO/releases/download/$TAG"

mkdir -p packaging/homebrew packaging/aur

cat > packaging/homebrew/stainlesssteel.rb <<EOF
class Stainlesssteel < Formula
  desc "Local-first coding assistant (source-available)"
  homepage "https://github.com/$REPO"
  version "$VER"
  license "PolyForm-Strict-1.0.0"

  on_macos do
    on_arm do
      url "$BASE/stainless-macos-aarch64"
      sha256 "$SHA_MAC"

      def install
        bin.install "stainless-macos-aarch64" => "stainless"
      end
    end
  end

  on_linux do
    on_intel do
      url "$BASE/stainless-linux-x86_64"
      sha256 "$SHA_X64"

      def install
        bin.install "stainless-linux-x86_64" => "stainless"
      end
    end
    on_arm do
      url "$BASE/stainless-linux-aarch64"
      sha256 "$SHA_ARM"

      def install
        bin.install "stainless-linux-aarch64" => "stainless"
      end
    end
  end

  test do
    assert_match "ok", shell_output("#{bin}/stainless doctor --json")
  end
end
EOF

cat > packaging/aur/PKGBUILD <<EOF
pkgname=stainless-steel-bin
pkgver=$VER
pkgrel=1
pkgdesc='Local-first coding assistant (source-available, PolyForm Strict 1.0.0)'
arch=('x86_64' 'aarch64')
url='https://github.com/$REPO'
license=('custom:PolyForm-Strict-1.0.0')
depends=('glibc')
source_x86_64=("$BASE/stainless-linux-x86_64")
source_aarch64=("$BASE/stainless-linux-aarch64")
sha256sums_x86_64=('$SHA_X64')
sha256sums_aarch64=('$SHA_ARM')

package() {
  install -Dm755 "stainless-linux-\$CARCH" "\$pkgdir/usr/bin/stainless"
}
EOF

cat > packaging/aur/.SRCINFO <<EOF
pkgbase = stainless-steel-bin
	pkgdesc = Local-first coding assistant (source-available, PolyForm Strict 1.0.0)
	pkgver = $VER
	pkgrel = 1
	url = https://github.com/$REPO
	arch = x86_64
	arch = aarch64
	license = custom:PolyForm-Strict-1.0.0
	depends = glibc
	source_x86_64 = $BASE/stainless-linux-x86_64
	source_aarch64 = $BASE/stainless-linux-aarch64
	sha256sums_x86_64 = $SHA_X64
	sha256sums_aarch64 = $SHA_ARM

pkgname = stainless-steel-bin
EOF

echo "Wrote packaging/homebrew/stainlesssteel.rb and packaging/aur/{PKGBUILD,.SRCINFO} for $TAG"
echo "Next:"
echo "  1. homebrew: push packaging/homebrew/stainlesssteel.rb as Formula/stainlesssteel.rb in the homebrew-stainlesssteel tap repo"
echo "  2. aur: copy packaging/aur/* into a stainless-steel-bin AUR checkout, makepkg --printsrcinfo to confirm, then git push (needs your AUR SSH key)"
