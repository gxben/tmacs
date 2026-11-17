#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 Benjamin Zores
# Generate the Homebrew formula for a release.
#
#   homebrew-formula.sh <version> <owner/repo> <SHA256SUMS file> > tmacs.rb
#
# The checksums file lists `<sha256>  tmacs_<version>_<os>_<arch>.tar.gz`.
set -eu
version="${1:?version}"
repo="${2:?owner/repo}"
sums="${3:?SHA256SUMS file}"

sum() {
  s=$(grep "tmacs_${version}_$1.tar.gz" "$sums" | head -1 | cut -d ' ' -f 1)
  [ -n "$s" ] || { echo "missing checksum for $1" >&2; exit 1; }
  echo "$s"
}

url="https://github.com/${repo}/releases/download/v${version}/tmacs_${version}"

cat <<RUBY
class Tmacs < Formula
  desc "Lightweight terminal text editor with Emacs key bindings"
  homepage "https://github.com/${repo}"
  license "Apache-2.0"
  version "${version}"

  on_macos do
    if Hardware::CPU.arm?
      url "${url}_darwin_arm64.tar.gz"
      sha256 "$(sum darwin_arm64)"
    else
      url "${url}_darwin_amd64.tar.gz"
      sha256 "$(sum darwin_amd64)"
    end
  end

  on_linux do
    if Hardware::CPU.arm? && Hardware::CPU.is_64_bit?
      url "${url}_linux_arm64.tar.gz"
      sha256 "$(sum linux_arm64)"
    else
      url "${url}_linux_amd64.tar.gz"
      sha256 "$(sum linux_amd64)"
    end
  end

  def install
    bin.install "tmacs"
    pkgshare.install "init.toml.example"
    doc.install "README.md", "LICENSE"
  end

  def caveats
    <<~EOS
      An example configuration is installed at:
        #{pkgshare}/init.toml.example
      Copy it to ~/.config/tmacs/init.toml to customize tmacs.
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/tmacs --version")
  end
end
RUBY
