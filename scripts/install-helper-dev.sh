#!/usr/bin/env bash
# Builds lanwell-helper and installs it with sudo, the same way the app's
# Install button does (with pkexec). `--uninstall` removes it again.
#
# What gets installed, and where, is in crates/helper/src/install.rs.

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"

if [[ "${1:-}" == "--uninstall" ]]; then
  command=uninstall
else
  command=install
fi

cargo build --quiet --manifest-path "$repo/Cargo.toml" -p lanwell-helper
sudo "$repo/target/debug/lanwell-helper" "$command"
