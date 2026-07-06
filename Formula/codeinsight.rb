class Codeinsight < Formula
  desc "Local-first code intelligence MCP server for AI agents"
  homepage "https://github.com/sleticalboy/CodeInsight-mcp"
  version "0.1.2"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.2/codeinsight-aarch64-apple-darwin.tar.gz"
      sha256 "2df85a498465c47f24a0e920f355a579556804f8be062bd474cd08a5e5161979"
    end

    on_intel do
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.2/codeinsight-x86_64-apple-darwin.tar.gz"
      sha256 "4892ccf6fb569b66788cc5ea519b4137bf2288b1af5db48ce620b8e7385f3a6e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.2/codeinsight-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "e081d9e61b5bf3fe98bb5b3779f3d6462ee4d0d13455837134c64ecbf98e342c"
    end

    on_intel do
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.2/codeinsight-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "48185f291e7e3c62d6c5ffe32ab28af6580ab1a1859a8aba5dafa1c0eb9908c4"
    end
  end

  def install
    bin.install "codeinsight"
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/codeinsight --help")
  end
end
