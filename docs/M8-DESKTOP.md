# Milestone 8 desktop checklist

This is shot-group photo analysis on the same Tauri 2 window as Milestones 2–7. The geometry, the group statistics, and the scoring live in `holdover-core`, so the later phone app can call the same functions. The window imports the photograph, stores a rectified copy of the transform, and lets you mark holes. CI compiles the app and runs the synthetic-target test. It does not open a window or switch a live theme.

Version 1.2.0 is the number in the crate, the window title, and the PKGBUILD. No tag and no release are created by this change.

Use a throwaway library so the real one at `~/.local/share/holdover/holdover.db` stays put.

## Commands

On the Omarchy machine, from the repo root, on this branch. Rust is mise's `rust@1.98.1` with no global default. The installed 1.1.0 binary does not have Groups. Build this branch, or install the PKGBUILD from this branch, before the window checks.

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover -p holdover-cli
mise exec rust@1.98.1 -- ./target/release/holdover-cli group-check
mkdir -p /tmp/holdover-m8
HOLDOVER_DB=/tmp/holdover-m8.db ./target/release/holdover
```

`group-check` prints that the synthetic photograph passed. The photograph and the known positions are `tests/fixtures/synthetic-group.png` and `tests/fixtures/synthetic-group.json`. Regenerate them with:

```sh
mise exec rust@1.98.1 -- ./target/release/holdover-cli group-fixture --out tests/fixtures/synthetic-group.png
```

The blobs for this library sit in `/tmp/holdover-m8.blobs/`. Leave `HOLDOVER_DB` unset when you want the real library again.

The laptop context is unchanged: Omarchy 4, Hyprland with a Lua config, 2880×1920 at scale 1.6, `GDK_SCALE=2`, light and dark themes.

## Synthetic card

The drawing is a 200 mm by 280 mm rectangle with a cross at each corner and five black holes. It is not a photograph of a range target. The aim point is the centre of that rectangle. The distance is 100 m and the bullet diameter is 7.82 mm. Five shots. The mean point of impact is 22 mm right and 16 mm down from the aim point. Extreme spread and mean radius are the `extreme_spread_mm` and `mean_radius_mm` values in the JSON sidecar (about 86.7 mm and 36.6 mm).

## What to check

- **Import.** Finish onboarding and add the example profile. Groups. Import `tests/fixtures/synthetic-group.png`. The canvas shows the card. Zoom in, zoom out, and Fit still show the crosses. Pan moves the card and leaves the marks on the crosses and holes.
- **Calibrate.** Leave Four corners selected. Width mm is 200 and Height mm is 280. Tick Aim point is the centre of the reference rectangle. Corners, then click the four crosses in order: top left, top right, bottom right, bottom left. Distance 100 m, Shots fired 5, Bullet diameter 7.82 mm. The strip shows 5 shots, extreme spread and mean radius within 1% of the sidecar, and the offset about 22 mm right and 16 mm down, with milliradians and MOA beside the linear figures. Find holes should land on the five circles. Move and Delete can correct a mark. A mark's hole count can be raised when two holes share a centre.
- **Save.** Save group stays disabled until the marked count is 5 and the distance is set. After Save, Equipment, Groups lists it, and Open returns to the same marks.
- **Units and theme.** With the strip showing, switch the header between Metric and US. The linear figures follow the small-length unit. Milliradians and MOA stay on the strip. Open Groups on a dark theme and a light theme. The page, the side panel, and an error stay readable. Tab reaches the tools, the file button, and the fields. Every control has a visible label.
- **Target.** The target list starts with the Swedish figures. Älgfigur scores the rings. A hole on the dashed moose outline is not given points for that outline, and the note says the dashed hit area is not scored. T2–T6 and the US targets do not grow a size you did not measure: Use sheet size appears only when that target already has a published width and height. An empty target is not scored.
- **Two points.** Switch calibration to Two points, clear the corners, and click two ends of a known side. The strip warns that the photograph is assumed square to the target, and the linear figures are whole millimetres.
- **Truing and impact.** Send to truing fills the truing distance, the correction (the opposite of the miss), the shot count, and the extreme spread. Show on the impact view draws the measured holes on the chart page's target. On a load log, a group saved with that load is listed and opens the group.
- **Photograph limits.** A HEIC file is refused with the sentence that this build reads JPEG and PNG. The stored file is a PNG. Keep the photograph's location, checked before import, is the only way latitude and longitude are kept, and they are fields on the group rather than data inside the image.
- **Sync.** Sync, Export. The intro mentions group photographs. Import the bundle into a second library (`HOLDOVER_DB=/tmp/holdover-m8-b.db` if you want a second file). The group and its photograph come across. With the location box unchecked, the other computer does not receive coordinates.
- **About.** Footer, About. The version line reads 1.2.0. The window title is Holdover 1.2.0. The crate list names image 0.25.10 and kamadak-exif 0.6.1. Ctrl+K includes Groups.

## Package

From `packaging/`, after the window checks if you want the menu binary updated:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
mise exec rust@1.98.1 -- makepkg -si
```

The recipe version is 1.2.0. This does not create a tag.

## Not in this build

No phone app. The analysis functions it will call are already in `holdover-core`. T2–T6 and the US rulebook targets stay unmeasured.
