class Basitop < Formula
  desc "Beautiful Apple Silicon performance monitor"
  homepage "https://github.com/tejasgajare/basitop"
  version "0.1.0"
  license "MIT"

  # Apple Silicon only — basitop reads M-series IOKit symbols.
  depends_on :macos
  depends_on arch: :arm64

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/tejasgajare/basitop/releases/download/v#{version}/basitop-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_SHA256"
    else
      url "https://github.com/tejasgajare/basitop/releases/download/v#{version}/basitop-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_SHA256"
    end
  end

  def install
    bin.install "basitop"
  end

  test do
    assert_match "basitop", shell_output("#{bin}/basitop --help")
  end
end
