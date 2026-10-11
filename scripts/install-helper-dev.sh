#!/usr/bin/env bash
# Builds netfoo-helper and installs it with sudo, the same way the app's
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

cargo build --quiet --manifest-path "$repo/Cargo.toml" -p netfoo-helper
sudo "$repo/target/debug/netfoo-helper" "$command"
