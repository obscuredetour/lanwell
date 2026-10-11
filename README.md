# netfoo

Share folders on your network, and (soon) mount other machines' shares, without
editing smb.conf, /etc/exports or fstab.

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
through the helper, which has to be installed as a system D-Bus service. To
install a debug build (asks for sudo):

```bash
scripts/install-helper-dev.sh
```

Run it again after changing the helper; `--uninstall` removes it.

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
