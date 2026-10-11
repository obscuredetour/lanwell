# Installing the helper

These files install `netfoo-helper` as a system D-Bus service that starts on
demand. Packages put them here, and so does `netfoo-helper install` (the
app's Install helper button, or `scripts/install-helper-dev.sh`), which has
them built in. The list is in `src/install.rs`; keep the two in step:

| File | Installed as |
|---|---|
| `netfoo-helper` (the binary) | `/usr/lib/netfoo/netfoo-helper` |
| `io.netfoo.Helper1.service` | `/usr/share/dbus-1/system-services/io.netfoo.Helper1.service` |
| `io.netfoo.Helper1.conf` | `/usr/share/dbus-1/system.d/io.netfoo.Helper1.conf` |
| `netfoo-helper.service` | `/usr/lib/systemd/system/netfoo-helper.service` |
| `io.netfoo.policy` | `/usr/share/polkit-1/actions/io.netfoo.policy` |

A package that installs these files makes the Install button unnecessary:
the app sees the helper is there and the same version, and doesn't offer it.
Remove a button-installed helper (`netfoo-helper uninstall`) before
installing a package, or the package manager will find its files already
there.

The names in these files must match `crates/core/src/bus.rs`.
