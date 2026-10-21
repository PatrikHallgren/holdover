# Milestone 7 desktop checklist

This is the sync bundle on the same Tauri 2 window as Milestones 2–6. Sync exports a `.holdover` file and imports it after a preview. The merge rules live in `holdover-core`, so the later phone companion and LAN sync can use the same decisions. CI compiles the app and runs the bundle tests. It does not open a window, switch a live theme, or move a file between two desktops.

Version 1.1.0 is the number in the crate, the window title, and the PKGBUILD. No tag and no release are created by this change.

You have one desktop. The two-library check uses `HOLDOVER_DB` so the real library at `~/.local/share/holdover/holdover.db` stays put.

## Commands

On the Omarchy machine, from the repo root, on this branch. Rust is mise's `rust@1.98.1` with no global default. The installed 1.0.0 binary does not have Sync. Build this branch, or install the PKGBUILD from this branch, before the window checks.

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover
mkdir -p /tmp/holdover-m7
HOLDOVER_DB=/tmp/holdover-m7/a.db ./target/release/holdover
```

Quit that window, then start the second library:

```sh
HOLDOVER_DB=/tmp/holdover-m7/b.db ./target/release/holdover
```

`a.db` and `b.db` are throwaway libraries. PDFs for A sit in `/tmp/holdover-m7/a.blobs/`. PDFs for B sit in `/tmp/holdover-m7/b.blobs/`. A bundle saved from A is under `/tmp/holdover-m7/backups/`. The packaged binary is the same idea: `HOLDOVER_DB=/tmp/holdover-m7/a.db /usr/bin/holdover`. Leave `HOLDOVER_DB` unset when you want the real library again.

Two windows can be open at once when each has its own `HOLDOVER_DB`.

The laptop context is unchanged: Omarchy 4, Hyprland with a Lua config, 2880×1920 at scale 1.6, `GDK_SCALE=2`, light and dark themes.

## What to check

- **Export.** On A, finish onboarding and add the example profile. Give a rifle a serial. Save a document with a small PDF. Sync, Export. The intro says the bundle is the library plus the PDFs, and that nothing is written on import until you apply a preview. Leave Include precise coordinates off. Save .holdover bundle. The line under the tabs names a file in `/tmp/holdover-m7/backups/` ending in `.holdover`.
- **Empty import.** On B, finish onboarding without the example, or with it. Sync, Import, and choose A's bundle. The preview is mostly New. Apply bundle. The profile, the document, and Open on that PDF work. The rifle serial on B is empty. A's serial is still on A. B's weather place, if you set one, is unchanged.
- **Second import.** Import the same file again. The rows are Unchanged. Apply bundle stays disabled, and the line says nothing in the bundle changes the library.
- **One-sided edit.** On A, rename the profile and export again (coordinates still off). On B, Import. That profile is Updated, with the two names when they differ. Apply. B shows A's new name. B's rifle serial stays empty.
- **Conflict.** On A and on B, change that same profile's name to two different names. Export from A. Import on B. The row is Conflict, and the filter opens on Conflict. Keep mine, Take theirs, and Keep both are there, and Apply stays disabled until one is pressed. Try each on a fresh conflict, one library at a time:
  - Keep mine leaves B's name and still applies the other rows.
  - Take theirs uses A's name.
  - Keep both leaves B's row and adds a second profile with A's name.
- **Tombstone.** On A, archive a rifle that no profile uses, export, and import on B. That rifle is Deleted. Apply. It shows under Equipment, Archive, on B, and it is gone from the live list.
- **Refusals.** On Import, choose a PDF or the library JSON from Settings. The message says the file is not a Holdover bundle, and the library is unchanged. Shorten a `.holdover` file with a text editor, or cut it off, and import that. The message says the bundle is incomplete or damaged, and the library is unchanged. A bundle whose schema number is newer than this app is refused whole with the newer-schema sentence. The tests cover that file; you do not have to build one by hand.
- **Device and place.** After every apply, Settings still shows the weather place you saved on that library. About does not show the other computer's device id. The import intro says the device id and the weather place are not replaced.
- **Units.** With a preview open, switch the header between Metric and US. The preview, the choices you already pressed, and Include precise coordinates stay. Solve still changes unit when you go back.
- **Theme, type, and focus.** Open Sync on a dark theme and a light theme. Secondary lines and an error stay readable on the page and on the panel. Tab reaches Skip to content, the tabs, the checkbox, the file button, and the choice buttons. Arrow keys move between Export and Import, and between the filters. The focused control shows the ring. Every control has a visible label or a tooltip. Larger text and the Omarchy text-size knob still apply.
- **About.** Footer, About. The version line reads 1.1.0. The window title is Holdover 1.1.0. The crate list names zip 2.6.1 (MIT). Ctrl+K includes Sync.

## Package

From `packaging/`, after the window checks if you want the menu binary updated:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
mise exec rust@1.98.1 -- makepkg -si
```

The recipe version is 1.1.0. This does not create a tag.

## Not in this build

No phone companion, no LAN sync, and no group photographs. The merge function they will call is already in `holdover-core`. Desktop import does not copy the other computer's weather place.
