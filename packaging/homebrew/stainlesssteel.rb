class Stainlesssteel < Formula
  desc "Local-first coding assistant (source-available)"
  homepage "https://github.com/KotalaKishanReddy/StainlessSteel"
  version "0.2.0"
  license "PolyForm-Strict-1.0.0"

  on_macos do
    on_arm do
      url "https://github.com/KotalaKishanReddy/StainlessSteel/releases/download/v0.2.0/stainless-macos-aarch64"
      sha256 "16084c68841a867aeae6c1b5104c7a67fad697e8370934f3525b7138a16a326d"

      def install
        bin.install "stainless-macos-aarch64" => "stainless"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/KotalaKishanReddy/StainlessSteel/releases/download/v0.2.0/stainless-linux-x86_64"
      sha256 "0e2091a132e468151f91558c3d72e68f139598455ea06322c3d834428acc8880"

      def install
        bin.install "stainless-linux-x86_64" => "stainless"
      end
    end
    on_arm do
      url "https://github.com/KotalaKishanReddy/StainlessSteel/releases/download/v0.2.0/stainless-linux-aarch64"
      sha256 "032895145a7698720680682b8eb9aa69924c19803e9e02a627f6e2c85532fd27"

      def install
        bin.install "stainless-linux-aarch64" => "stainless"
      end
    end
  end

  test do
    assert_match "ok", shell_output("#{bin}/stainless doctor --json")
  end
end
