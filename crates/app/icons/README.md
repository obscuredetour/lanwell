# Icons

`logo.svg` is the logo: an "lw" whose w starts with the l, in the UI's accent
violet. `source.svg` is the app icon, the logo on a violet tile; to regenerate
the rest after changing it:

```bash
cargo tauri icon source.svg -o .
```

Then delete the `android/`, `ios/` and `Square*`/`StoreLogo` files it also makes (lanwell doesn't target those).
