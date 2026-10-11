# Icons

A placeholder. `source.svg` is the original; to regenerate the rest after changing it:

```bash
rsvg-convert -w 1024 -h 1024 source.svg -o /tmp/netfoo-icon.png
cargo tauri icon /tmp/netfoo-icon.png -o .
```

Then delete the `android/`, `ios/` and `Square*`/`StoreLogo` files it also makes (netfoo doesn't target those).
