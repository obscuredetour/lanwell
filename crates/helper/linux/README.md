# Installing the helper

These files install `lanwell-helper` as a system D-Bus service that starts on
demand. Packages put them here, and so does `lanwell-helper install` (run by
install.sh, the app's Install helper button in development, or
`scripts/install-helper-dev.sh`), which has them built in. The list is in `src/install.rs`; keep the two in step:

| File | Installed as |
|---|---|
| `lanwell-helper` (the binary) | `/usr/lib/lanwell/lanwell-helper` |
| `io.github.obscuredetour.lanwell.Helper1.service` | `/usr/share/dbus-1/system-services/io.github.obscuredetour.lanwell.Helper1.service` |
| `io.github.obscuredetour.lanwell.Helper1.conf` | `/usr/share/dbus-1/system.d/io.github.obscuredetour.lanwell.Helper1.conf` |
| `lanwell-helper.service` | `/usr/lib/systemd/system/lanwell-helper.service` |
| `io.github.obscuredetour.lanwell.policy` | `/usr/share/polkit-1/actions/io.github.obscuredetour.lanwell.policy` |

A package that installs these files makes the Install button unnecessary:
the app sees the helper is there and the same version, and doesn't offer it.
Remove a helper installed any other way (`sudo /usr/lib/lanwell/lanwell-helper
uninstall`) before installing a package, or the package manager will find its
files already there.

The names in these files must match `crates/core/src/bus.rs`.
