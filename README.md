# Lanwell

Mount other machines' shares, and share folders on your network, without editing
fstab, smb.conf or /etc/exports.

- **Network folders**: type `//nas.lan/media` (SMB) or `nas.lan:/srv/media` (NFS),
  and the share appears at `~/Network/media`. It connects when the folder's first
  opened, so startup never waits for the NAS (or at startup, if you'd rather). SMB
  passwords are kept where only the system can read them.
- **Samba shares** for Windows, Macs and phones: pick a folder, choose who can
  open it (people on this machine, or anyone), done. Set each person's Samba
  password from the same place.
- **NFS exports** for Linux and other Unix machines, by host or network.
- **Leaves your config alone.** Lanwell writes files of its own and adds one
  `include` line to smb.conf; anything you configured by hand stays as it was.

For Linux; macOS maybe later. Rust core, a small privileged helper behind
polkit, and a Tauri + Solid app. See [docs/architecture.md](docs/architecture.md).

## Install

For x86_64 Linux, at least as new as Ubuntu 24.04 (Debian 13, Fedora 40, Arch).
The app goes in `~/.local`, for you. Its helper, the part that changes
system settings, goes in `/usr/lib/lanwell`, so the installer asks for your
password (with sudo):

```bash
curl -fsSL https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh | sh
```

Or download [install.sh](https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh),
read it, and run `sh install.sh`. Running it again updates Lanwell and its
helper.

Lanwell checks GitHub for a new version when it starts and once a day, and
says at the top of the window when one is out. To stop it, untick **Check
for new versions** at the foot of the sidebar.

You need WebKitGTK 4.1 (install.sh says if it's missing), polkit, and Samba
or NFS for whichever you'll use; Lanwell says what's missing.

To remove Lanwell and its helper:

```bash
curl -fsSL https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh | sh -s -- --uninstall
```

Shares and network folders you set up stay as they are.

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

### Releasing

Bump the version in `Cargo.toml` and `crates/app/ui/package.json`, merge, then
tag and push:

```bash
git tag v0.1.0 && git push origin v0.1.0
```

The Release workflow builds the tarball with `scripts/package.sh` and attaches
it to a draft release; publish the draft once its notes are written. To try a
build with install.sh first, run `scripts/package.sh`; it ends with the
command that installs what it built.

## Layout

| Path | What's there |
|---|---|
| `crates/core` | The model, validation, requests, state file and config rendering. No root, no D-Bus. |
| `crates/helper` | The privileged helper, and (in `linux/`) the files that install it. |
| `crates/app` | The Tauri app; `ui/` is the Solid front end. |
| `scripts` | Development and packaging tools. |
| `install.sh` | The installer, attached to each release. |
| `docs` | Architecture and decisions. |

## License

[MIT](LICENSE). Lanwell runs the system's own tools as separate programs (Samba's
`smbclient` and `smbpasswd`, cifs-utils' `mount.cifs`, nfs-utils' `exportfs`)
rather than including their code.
