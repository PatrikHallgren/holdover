# Holdover desktop shell

Tauri 2 window used for the Omarchy spike. The solver is `holdover-core`. This is not the Milestone 2 solve screen.

Build and theme-switch check, on Omarchy:

```sh
tools/desktop-spike.sh
```

See `docs/M1-DESKTOP.md` for the checklist and the result. On 2026-10-16 that checklist passed on Omarchy 4, so the shell stays Tauri 2. The script does not install packages unless you pass `--install`, and a missing `wget` does not stop the build.

CI compiles this crate. It does not open a window.
