# Holdover desktop

Tauri 2 window for the Milestone 2 Solve screen. The solver is `holdover-core`. SQLite and the Omarchy theme follower live in this crate.

On Omarchy, from the repo root:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover
./target/release/holdover
```

The checklist for weather and library files is `docs/M5-DESKTOP.md`. Earlier windows are `docs/M2-DESKTOP.md`, `docs/M3-DESKTOP.md`, and `docs/M4-DESKTOP.md`. The 2026-10-16 shell result, which kept Tauri 2, is `docs/M1-DESKTOP.md`. `tools/desktop-spike.sh` still builds this binary and checks a theme-directory rename. It does not install packages unless you pass `--install`, and a missing `wget` does not stop the build.

CI compiles this crate. It does not open a window.
