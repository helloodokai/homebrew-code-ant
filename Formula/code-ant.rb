class CodeAnt < Formula
  desc "Autonomous code-improvement agent for the command line"
  homepage "https://github.com/helloodokai/code-ant"
  url "https://github.com/helloodokai/code-ant/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "60cfdbe3b20fee7824a9d54703b1c582baa194f148206f05d1d75698b0ef0edb"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "Autonomous code-improvement agent", shell_output("#{bin}/code-ant --help")
  end
end
