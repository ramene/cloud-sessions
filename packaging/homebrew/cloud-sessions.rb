# DRAFT Homebrew formula for the cloud-sessions CLI (not published; no tap exists yet).
#
# To publish: create a tap repository (for example <owner>/homebrew-tap), copy this file to Formula/, and replace
# each sha256 below with the matching line from the release's `.tar.gz.sha256` assets. The repository is private
# today, so `brew install` can only download these assets with HOMEBREW_GITHUB_API_TOKEN-style access; make the
# release public (or host the archives elsewhere) before advertising it.
class CloudSessions < Formula
  desc "Start, read, watch and send into Claude Code cloud sessions (CLI, MCP server, plugin)"
  homepage "https://github.com/ramene/cloud-sessions"
  license "MIT"
  version "0.2.2"

  base = "https://github.com/ramene/cloud-sessions/releases/download/cloud-sessions-v#{version}"

  on_macos do
    url "#{base}/cloud-sessions-#{version}-macos-universal.tar.gz"
    sha256 "REPLACE_WITH_SHA256_OF_macos-universal_ARCHIVE"
  end
  on_linux do
    on_intel do
      url "#{base}/cloud-sessions-#{version}-linux-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_OF_linux-x86_64_ARCHIVE"
    end
    on_arm do
      url "#{base}/cloud-sessions-#{version}-linux-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_OF_linux-aarch64_ARCHIVE"
    end
  end

  def install
    bin.install "cloud-sessions"
    pkgshare.install "plugin"
  end

  def caveats
    <<~EOS
      Add the Claude Code plugin (the marketplace is in the repository):
        claude plugin marketplace add ramene/cloud-sessions
        claude plugin install cloud-sessions@cloud-sessions
      Or load the copy that came with this formula:
        claude --plugin-dir #{opt_pkgshare}/plugin
      Allow repositories yourself: export CLOUD_SESSIONS_REPOS=owner/name
      Inside Claude Code, reading uses its built-in RemoteTrigger tool: nothing to set up. To read from a terminal,
      opt in yourself: cloud-sessions auth use-claude-login (a claude setup-token token cannot read sessions).
    EOS
  end

  test do
    assert_match "usage: cloud-sessions", shell_output("#{bin}/cloud-sessions help")
    assert_match '"ok":false', shell_output("#{bin}/cloud-sessions send 'bad id' --message hi 2>&1", 1)
  end
end
