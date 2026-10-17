#!/bin/sh
# Build Holdover's Tauri shell on Omarchy and check that a running window
# follows a theme-directory rename.
#
# This script does not run sudo unless you pass --install. It prints the
# pacman command either way.
#
# Usage:
#   tools/desktop-spike.sh
#   tools/desktop-spike.sh --install
#
# Rust on Patrik's machine is mise's rust@1.98.1, with no global default.
# The script uses `mise exec rust@1.98.1 --` when mise is on PATH.

set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$root"

install=0
if [ "${1:-}" = "--install" ]; then
  install=1
fi

packages="webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool"

echo "Arch packages this shell needs (Tauri 2 prerequisites, 2026-08-20):"
echo "  $packages"
echo "Install command, not run unless you passed --install:"
echo "  sudo pacman -S --needed $packages"

if ! command -v pacman >/dev/null 2>&1; then
  echo "pacman is not on PATH. Run this script on Omarchy (Arch)." >&2
  exit 1
fi

missing=""
for package in $packages; do
  if ! pacman -Q "$package" >/dev/null 2>&1; then
    missing="$missing $package"
  fi
done

if [ -n "$missing" ]; then
  echo "Missing:$missing" >&2
  if [ "$install" -eq 1 ]; then
    echo "Running: sudo pacman -S --needed$missing"
    # The sudo password prompt is visible. Nothing here is a silent install.
    sudo pacman -S --needed $missing
  else
    echo "Re-run with --install to run that pacman command, or install the packages yourself." >&2
    exit 1
  fi
fi

if command -v mise >/dev/null 2>&1; then
  echo "Building with: mise exec rust@1.98.1 -- cargo build --release -p holdover"
  mise exec rust@1.98.1 -- cargo build --release -p holdover
else
  echo "mise is not on PATH. cargo must already be Rust 1.98.1." >&2
  cargo build --release -p holdover
fi

bin="$root/target/release/holdover"
if [ ! -x "$bin" ]; then
  echo "Build finished but $bin is missing." >&2
  exit 1
fi

tmp=$(mktemp -d)
mkdir -p "$tmp/current/theme"
status="$tmp/palette.json"

theme_json() {
  accent=$1
  cat > "$2" << EOF
{
  "mode": "dark",
  "background": "#1a1916",
  "foreground": "#f3efe6",
  "accent": "$accent",
  "muted": "#8a8478",
  "red": "#e15a4a",
  "yellow": "#e0b15a",
  "orange": "#d98a55",
  "green": "#8fbf6a",
  "cyan": "#5aa8a8",
  "blue": "#7aa2d4",
  "magenta": "#b794d4",
  "surface": "#252219",
  "border": "#312e27"
}
EOF
}

theme_json "#112233" "$tmp/current/theme/holdover.json"

echo "Launching the spike window against a temporary theme directory."
HOLDOVER_THEME_DIR="$tmp/current" \
HOLDOVER_PALETTE_STATUS="$status" \
HOLDOVER_TEXT_SCALE=1 \
  "$bin" > "$tmp/holdover.log" 2>&1 &
pid=$!

cleanup_fail() {
  kill "$pid" 2>/dev/null || true
  echo "Window log: $tmp/holdover.log" >&2
}

wait_for() {
  needle=$1
  i=0
  while [ "$i" -lt 80 ]; do
    if [ -f "$status" ] && grep -q "$needle" "$status"; then
      return 0
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "The window exited before the palette status was written." >&2
      cleanup_fail
      return 1
    fi
    i=$((i + 1))
    sleep 0.25
  done
  echo "Timed out waiting for accent $needle in $status" >&2
  cleanup_fail
  return 1
}

wait_for "#112233"

mkdir -p "$tmp/current/next-theme"
theme_json "#abcdef" "$tmp/current/next-theme/holdover.json"
rm -rf "$tmp/current/theme"
mv "$tmp/current/next-theme" "$tmp/current/theme"

wait_for "#abcdef"

echo "Theme-switch check passed: accent #112233 then #abcdef after next-theme was renamed to theme."
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true

echo "Launching Holdover on the real Omarchy theme directory (HOLDOVER_THEME_DIR unset)."
unset HOLDOVER_THEME_DIR || true
unset HOLDOVER_PALETTE_STATUS || true
# nohup so the window stays up after this script exits.
nohup "$bin" > "$tmp/live.log" 2>&1 &
echo "Holdover is running (pid $!). Leave this window up for the checklist in docs/M1-DESKTOP.md."
echo "Window log: $tmp/live.log"
