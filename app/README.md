# Holdover desktop shell

Tauri 2 window used for the Omarchy spike. The solver is `holdover-core`. This is not the Milestone 2 solve screen.

Build and theme-switch check, on Omarchy:

```sh
tools/desktop-spike.sh
```

See `docs/M1-DESKTOP.md` for the scaling, theme, text-size, and tiling checklist, and for the `sudo pacman` line. The script does not install packages unless you pass `--install`.

CI compiles this crate. It does not open a window. Qt 6 Quick stays the fallback if the checklist fails.
