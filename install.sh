#!/bin/sh
# Installs Lanwell from its latest release:
#
#   curl -fsSL https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh | sh
#
# The app goes in ~/.local, for you. Its helper, the part that changes system
# settings, goes in /usr/lib/lanwell, so sudo asks for your password.
# Running this again updates both.
#
#   sh install.sh --uninstall    removes lanwell, and its helper if it's installed
#
# LANWELL_VERSION=v0.1.0 installs that release instead of the latest.
# LANWELL_RELEASE_URL=file:///…/dist installs a build from scripts/package.sh.

set -eu

repo="obscuredetour/lanwell"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
lib="$HOME/.local/lib/lanwell"
bin="$HOME/.local/bin"
desktop="$data/applications/lanwell.desktop"
icons="$data/icons/hicolor"
icon_sizes="32 128 256 512"
# Where lanwell installs its helper; see crates/helper/src/install.rs.
helper="/usr/lib/lanwell/lanwell-helper"

say() { printf '%s\n' "$*"; }
fail() {
  printf 'lanwell: %s\n' "$*" >&2
  exit 1
}

main() {
  case "${1:-}" in
    "") install ;;
    --uninstall) uninstall ;;
    -h | --help) say "Usage: install.sh [--uninstall]" ;;
    *) fail "unknown option $1 (try --help)" ;;
  esac
}

install() {
  [ "$(id -u)" -ne 0 ] ||
    fail "run this as yourself, not as root. Lanwell installs for you, and asks for an administrator's password for its helper."
  [ "$(uname -s)" = Linux ] || fail "Lanwell is for Linux."
  arch="$(uname -m)"
  [ "$arch" = x86_64 ] || fail "Lanwell's releases are for x86_64 so far, and this is $arch."
  command -v curl >/dev/null || fail "curl isn't installed."
  command -v sudo >/dev/null || fail "sudo isn't installed, and Lanwell needs it to install its helper."

  if [ -n "${LANWELL_RELEASE_URL:-}" ]; then
    url="$LANWELL_RELEASE_URL"
  elif [ -n "${LANWELL_VERSION:-}" ]; then
    url="https://github.com/$repo/releases/download/$LANWELL_VERSION"
  else
    url="https://github.com/$repo/releases/latest/download"
  fi
  asset="lanwell-$arch-linux.tar.gz"

  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  say "Downloading $url/$asset"
  curl -fsSL -o "$tmp/$asset" "$url/$asset" || fail "couldn't download $asset."
  curl -fsSL -o "$tmp/$asset.sha256" "$url/$asset.sha256" || fail "couldn't download $asset.sha256."
  (cd "$tmp" && sha256sum --check --quiet "$asset.sha256") ||
    fail "the download doesn't match its checksum; try again."
  tar -xzf "$tmp/$asset" -C "$tmp"
  new="$tmp/lanwell"

  check_libraries "$new/lanwell"

  # The helper runs as root, so it's installed from the download just checked,
  # and goes only where root alone can change it: never beside the app, where
  # anything running as you could replace it. First, so that if sudo is
  # refused, nothing has changed.
  say "Installing Lanwell's helper in /usr/lib/lanwell; sudo asks for your password."
  sudo "$new/lanwell-helper" install >/dev/null ||
    fail "the helper wasn't installed, so nothing was. Run this again to try again."

  mkdir -p "$lib" "$bin" "$data/applications"
  # Rename into place, so a lanwell that's running keeps the files it started with.
  for file in lanwell VERSION; do
    cp "$new/$file" "$lib/.$file.new"
    mv -f "$lib/.$file.new" "$lib/$file"
  done
  # Earlier installs kept a copy of the helper here.
  rm -f "$lib/lanwell-helper"
  ln -sfn "$lib/lanwell" "$bin/lanwell"

  # The menu entry runs lanwell by its full path; ~/.local/bin isn't always on PATH.
  while IFS= read -r line; do
    case "$line" in
      Exec=*) printf 'Exec="%s"\n' "$lib/lanwell" ;;
      *) printf '%s\n' "$line" ;;
    esac
  done <"$new/lanwell.desktop" >"$desktop"
  for size in $icon_sizes; do
    mkdir -p "$icons/${size}x$size/apps"
    cp "$new/icons/$size.png" "$icons/${size}x$size/apps/lanwell.png"
  done

  say "Installed Lanwell $(cat "$lib/VERSION") in $lib, and its helper in $(dirname "$helper")."
  case ":$PATH:" in
    *":$bin:"*) ;;
    *) say "$bin isn't on your PATH, so start Lanwell from your app menu (or add it to PATH)." ;;
  esac
  say "Open Lanwell from your app menu (if it's open, close it and open it again)."
}

# Fails, saying what to install, if lanwell can't run here.
check_libraries() {
  command -v ldd >/dev/null || return 0
  missing="$(ldd "$1" 2>&1 | grep 'not found' || true)"
  [ -n "$missing" ] || return 0
  case "$missing" in
    *GLIBC_*)
      fail "this system's C library (glibc) is older than Lanwell needs. Lanwell is built on Ubuntu 24.04, and runs on distributions at least that new." ;;
    *libwebkit2gtk-4.1*)
      fail "Lanwell needs WebKitGTK 4.1. $(install_hint webkit)" ;;
    *)
      fail "Lanwell can't find these libraries:
$missing" ;;
  esac
}

# How to install a dependency on this distribution, if it's one we know.
install_hint() {
  [ -r /etc/os-release ] || return 0
  # shellcheck disable=SC1091
  distro="$(. /etc/os-release && echo "${ID:-} ${ID_LIKE:-}")"
  case " $distro " in
    *" arch "*) say "Install it with: sudo pacman -S --needed webkit2gtk-4.1" ;;
    *" debian "* | *" ubuntu "*) say "Install it with: sudo apt install libwebkit2gtk-4.1-0" ;;
    *" fedora "* | *" rhel "*) say "Install it with: sudo dnf install webkit2gtk4.1" ;;
    *suse*) say "Install it with: sudo zypper install libwebkit2gtk-4_1-0" ;;
  esac
}

uninstall() {
  [ "$(id -u)" -ne 0 ] || fail "run this as yourself, not as root, so it finds your installation."

  if [ -e "$helper" ]; then
    say "Removing Lanwell's helper, which needs an administrator's password."
    sudo "$helper" uninstall >/dev/null
  fi

  rm -f "$lib/lanwell" "$lib/lanwell-helper" "$lib/VERSION"
  rmdir "$lib" 2>/dev/null || true
  # Only our link; a lanwell installed some other way stays.
  if [ -L "$bin/lanwell" ] && [ "$(readlink "$bin/lanwell")" = "$lib/lanwell" ]; then
    rm -f "$bin/lanwell"
  fi
  rm -f "$desktop"
  for size in $icon_sizes; do
    rm -f "$icons/${size}x$size/apps/lanwell.png"
  done

  say "Removed Lanwell. Shares and network folders you set up stay as they are; their settings are in /etc/lanwell."
}

# Everything above only defines functions, so a download that's cut short
# runs nothing.
main "$@"
