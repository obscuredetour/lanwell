# netfoo

Mount other machines' shares, and share folders on your network, without editing
fstab, smb.conf or /etc/exports.

- **Network folders**: type `//nas.lan/media`, sign in, and the share appears at
  `~/Network/media`. It connects when the folder's first opened, so startup never
  waits for the NAS (or at startup, if you'd rather). The password is kept where
  only the system can read it.
- **Samba shares** for Windows, Macs and phones: pick a folder, choose who can
  open it (people on this machine, or anyone), done. Set each person's Samba
  password from the same place.
- **NFS exports** for Linux and other Unix machines, by host or network.
- **Leaves your config alone.** netfoo writes files of its own and adds one
  `include` line to smb.conf; anything you configured by hand stays as it was.

Linux now; macOS is planned. Rust core, a small privileged helper behind
polkit, and a Tauri + Solid app. See [docs/architecture.md](docs/architecture.md).

## Development

You need Rust, Node 24 with pnpm, and Tauri's system libraries
(webkit2gtk-4.1 on Linux).

```bash
pnpm --dir crates/app/ui install
```

```bash
cargo tauri dev
```

The app can show what's shared without any setup, but making changes goes
through the helper, a system D-Bus service. Until it's installed, the app
shows an **Install helper** button, which asks for an administrator's
password (with pkexec) and installs the debug build `cargo tauri dev` made.
After changing the helper, the button changes to **Update helper**, since
the installed helper is no longer the build beside the app. To install from
a terminal instead (asks for sudo):

```bash
scripts/install-helper-dev.sh
```

`--uninstall` removes it.

### Checks

```bash
cargo test --workspace
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
pnpm --dir crates/app/ui lint
```

`cargo test` also regenerates the UI's TypeScript types in
`crates/app/ui/src/bindings` from the Rust types; commit them with the change.

## Layout

| Path | What's there |
|---|---|
| `crates/core` | The model, validation, requests, state file and config rendering. No root, no D-Bus. |
| `crates/helper` | The privileged helper, and (in `linux/`) the files that install it. |
| `crates/app` | The Tauri app; `ui/` is the Solid front end. |
| `scripts` | Development tools. |
| `docs` | Architecture and decisions. |

## License

[MIT](LICENSE). netfoo runs the system's own tools as separate programs (Samba's
`smbclient` and `smbpasswd`, cifs-utils' `mount.cifs`, nfs-utils' `exportfs`)
rather than including their code.
