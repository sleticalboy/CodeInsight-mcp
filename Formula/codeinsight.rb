class Codeinsight < Formula
  desc "Local-first code intelligence MCP server for AI agents"
  homepage "https://github.com/sleticalboy/CodeInsight-mcp"
  license "Apache-2.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.1/codeinsight-aarch64-apple-darwin.tar.gz"
      sha256 "40251ce737d1808260062d358aed43cebef5cbbc4162e98ce5c306f59ae0f0c4"
    else
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.1/codeinsight-x86_64-apple-darwin.tar.gz"
      sha256 "43ef42bfdef901ba49bb3bd65fd1ceb3568678c51e7553c03317b5d59e0bfe56"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.1/codeinsight-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "b8785eb2f647f2ef5ae2af96e3368411184f318b0f69d8ed1b31a0b57203c10d"
    else
      url "https://github.com/sleticalboy/CodeInsight-mcp/releases/download/v0.1.1/codeinsight-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "e777c420936f4ee440b6a737c13fe421aeabb800f7f4fdbda49147eca76d6c97"
    end
  end

  def install
    bin.install "codeinsight"
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/codeinsight --help")
  end
end
