# Installing the helper

These files install `netfoo-helper` as a system D-Bus service that starts on
demand. Packages put them here:

| File | Installed as |
|---|---|
| `netfoo-helper` (the binary) | `/usr/lib/netfoo/netfoo-helper` |
| `io.netfoo.Helper1.service` | `/usr/share/dbus-1/system-services/io.netfoo.Helper1.service` |
| `io.netfoo.Helper1.conf` | `/usr/share/dbus-1/system.d/io.netfoo.Helper1.conf` |
| `netfoo-helper.service` | `/usr/lib/systemd/system/netfoo-helper.service` |
| `io.netfoo.policy` | `/usr/share/polkit-1/actions/io.netfoo.policy` |

For development, `scripts/install-helper-dev.sh` installs a debug build to
the same places (and `--uninstall` removes it).

The names in these files must match `crates/core/src/bus.rs`.
