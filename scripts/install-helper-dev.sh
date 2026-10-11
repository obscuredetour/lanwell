#!/usr/bin/env bash
# Installs a debug build of netfoo-helper, and the D-Bus, systemd and polkit
# files that let the app start it, so the app can make changes in development.
# Asks for sudo. `--uninstall` removes everything it installed.
#
# The paths are the ones packages use (see crates/helper/linux/README.md).

set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
files="$repo/crates/helper/linux"

binary=/usr/lib/netfoo/netfoo-helper
installed=(
  "$files/io.netfoo.Helper1.service:/usr/share/dbus-1/system-services/io.netfoo.Helper1.service"
  "$files/io.netfoo.Helper1.conf:/usr/share/dbus-1/system.d/io.netfoo.Helper1.conf"
  "$files/netfoo-helper.service:/usr/lib/systemd/system/netfoo-helper.service"
  "$files/io.netfoo.policy:/usr/share/polkit-1/actions/io.netfoo.policy"
)

reload() {
  sudo systemctl daemon-reload
  # D-Bus reads its policy files (system.d) again when reloaded.
  sudo systemctl reload dbus
}

if [[ "${1:-}" == "--uninstall" ]]; then
  # A running helper would keep the old binary around until it exits.
  sudo systemctl stop netfoo-helper.service 2>/dev/null || true
  sudo rm -f "$binary"
  sudo rmdir --ignore-fail-on-non-empty "$(dirname "$binary")"
  for pair in "${installed[@]}"; do
    sudo rm -f "${pair#*:}"
  done
  reload
  echo "Removed netfoo-helper. (Shares it set up, and /etc/netfoo, are left as they are.)"
  exit 0
fi

cargo build --manifest-path "$repo/Cargo.toml" -p netfoo-helper

sudo systemctl stop netfoo-helper.service 2>/dev/null || true
sudo install -D -m 755 "$repo/target/debug/netfoo-helper" "$binary"
for pair in "${installed[@]}"; do
  sudo install -D -m 644 "${pair%%:*}" "${pair#*:}"
done
reload

echo "Installed netfoo-helper. The app starts it when it's needed."
