#!/bin/sh
# Installs lanwell for you (not system-wide) from its latest release:
#
#   curl -fsSL https://github.com/obscuredetour/lanwell/releases/latest/download/install.sh | sh
#
# The app goes in ~/.local and needs no password. lanwell asks for one later,
# when it installs its helper (the part that changes system settings).
# Running this again updates lanwell.
#
#   sh install.sh --uninstall    removes lanwell, and its helper if it's installed
#
# LANWELL_VERSION=v0.1.0 installs that release instead of the latest.
# LANWELL_RELEASE_URL=file:///…/dist installs a build from scripts/package.sh.

set -eu

repo="obscuredetour/lanwell"
app_id="io.github.obscuredetour.lanwell"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
lib="$HOME/.local/lib/lanwell"
bin="$HOME/.local/bin"
desktop="$data/applications/$app_id.desktop"
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
    fail "run this as yourself, not as root. lanwell installs for you, and asks for an administrator's password when it needs one."
  [ "$(uname -s)" = Linux ] || fail "lanwell is for Linux."
  arch="$(uname -m)"
  [ "$arch" = x86_64 ] || fail "lanwell's releases are for x86_64 so far, and this is $arch."
  command -v curl >/dev/null || fail "curl isn't installed."

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
  command -v pkexec >/dev/null ||
    say "Note: pkexec isn't installed. lanwell needs it to install its helper; it's in your distribution's polkit (or pkexec) package."

  mkdir -p "$lib" "$bin" "$data/applications"
  # Rename into place, so a lanwell that's running keeps the files it started with.
  for file in lanwell lanwell-helper VERSION; do
    cp "$new/$file" "$lib/.$file.new"
    mv -f "$lib/.$file.new" "$lib/$file"
  done
  ln -sfn "$lib/lanwell" "$bin/lanwell"

  # The menu entry runs lanwell by its full path; ~/.local/bin isn't always on PATH.
  while IFS= read -r line; do
    case "$line" in
      Exec=*) printf 'Exec="%s"\n' "$lib/lanwell" ;;
      *) printf '%s\n' "$line" ;;
    esac
  done <"$new/$app_id.desktop" >"$desktop"
  for size in $icon_sizes; do
    mkdir -p "$icons/${size}x$size/apps"
    cp "$new/icons/$size.png" "$icons/${size}x$size/apps/$app_id.png"
  done

  say "Installed lanwell $(cat "$lib/VERSION") in $lib."
  case ":$PATH:" in
    *":$bin:"*) ;;
    *) say "$bin isn't on your PATH, so start lanwell from your app menu (or add it to PATH)." ;;
  esac
  if [ -e "$helper" ]; then
    say "Open lanwell; if its helper is older than this version, it offers to update it."
  else
    say "Open lanwell from your app menu. To make changes, it asks to install its helper first."
  fi
}

# Fails, saying what to install, if lanwell can't run here.
check_libraries() {
  command -v ldd >/dev/null || return 0
  missing="$(ldd "$1" 2>&1 | grep 'not found' || true)"
  [ -n "$missing" ] || return 0
  case "$missing" in
    *GLIBC_*)
      fail "this system's C library (glibc) is older than lanwell needs. lanwell is built on Ubuntu 24.04, and runs on distributions at least that new." ;;
    *libwebkit2gtk-4.1*)
      fail "lanwell needs WebKitGTK 4.1. $(install_hint webkit)" ;;
    *)
      fail "lanwell can't find these libraries:
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
    say "Removing lanwell's helper, which needs an administrator's password."
    if command -v sudo >/dev/null; then
      sudo "$helper" uninstall
    else
      pkexec "$helper" uninstall
    fi
  fi

  rm -f "$lib/lanwell" "$lib/lanwell-helper" "$lib/VERSION"
  rmdir "$lib" 2>/dev/null || true
  # Only our link; a lanwell installed some other way stays.
  if [ -L "$bin/lanwell" ] && [ "$(readlink "$bin/lanwell")" = "$lib/lanwell" ]; then
    rm -f "$bin/lanwell"
  fi
  rm -f "$desktop"
  for size in $icon_sizes; do
    rm -f "$icons/${size}x$size/apps/$app_id.png"
  done

  say "Removed lanwell. Shares and network folders you set up stay as they are; their settings are in /etc/lanwell."
}

# Everything above only defines functions, so a download that's cut short
# runs nothing.
main "$@"
