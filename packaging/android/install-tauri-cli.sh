#!/bin/sh
# Install the tauri-cli 2.12.4 binary that matches the tauri crate in Cargo.lock.
# Compiling the CLI from source is unnecessary.
set -eu

version="${HOLDOVER_TAURI_CLI:-2.12.4}"
target="${HOLDOVER_TAURI_TARGET:-x86_64-unknown-linux-gnu}"
dest="${CARGO_HOME:-$HOME/.cargo}/bin"
url="https://github.com/tauri-apps/tauri/releases/download/tauri-cli-v${version}/cargo-tauri-${target}.tgz"
tmp="$(mktemp -d)"
curl -fsSL "$url" -o "$tmp/cli.tgz"
mkdir -p "$dest"
tar -xzf "$tmp/cli.tgz" -C "$dest"
rm -rf "$tmp"
chmod +x "$dest/cargo-tauri"
"$dest/cargo-tauri" --version
