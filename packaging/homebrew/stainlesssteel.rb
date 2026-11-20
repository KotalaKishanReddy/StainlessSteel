class Stainlesssteel < Formula
  desc "Local-first coding assistant (source-available)"
  homepage "https://github.com/KotalaKishanReddy/StainlessSteel"
  version "0.2.3"
  license "PolyForm-Strict-1.0.0"

  on_macos do
    on_arm do
      url "https://github.com/KotalaKishanReddy/StainlessSteel/releases/download/v0.2.3/stainless-macos-aarch64"
      sha256 "1c8bdeeb530d0c578c22938e12417c94f896e6ef21d2d0cb17eb4eeebf427293"

      def install
        bin.install "stainless-macos-aarch64" => "stainless"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/KotalaKishanReddy/StainlessSteel/releases/download/v0.2.3/stainless-linux-x86_64"
      sha256 "f0f72d131b3a9879f251cf147c3ba99f3f961989416b6ff9f14d80cbbdcd177d"

      def install
        bin.install "stainless-linux-x86_64" => "stainless"
      end
    end
    on_arm do
      url "https://github.com/KotalaKishanReddy/StainlessSteel/releases/download/v0.2.3/stainless-linux-aarch64"
      sha256 "429011160fe821a1b7ea12bec33610083ac4f07fe6d8fa583611030ff14ab138"

      def install
        bin.install "stainless-linux-aarch64" => "stainless"
      end
    end
  end

  test do
    assert_match "ok", shell_output("#{bin}/stainless doctor --json")
  end
end
