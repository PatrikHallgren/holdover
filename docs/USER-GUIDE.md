# Holdover user guide

Holdover is a personal, non-commercial ballistic calculator and equipment notebook for the Omarchy 4 desktop. Predictions come from the model and the numbers you type. Confirm them with live fire. You are responsible for safe and lawful shooting.

The window is English. Version 1.3.1 is prepared in this tree. No tag is created here. The 1.0.0 tag commands stay in [`M6-DESKTOP.md`](M6-DESKTOP.md). The sync checklist is [`M7-DESKTOP.md`](M7-DESKTOP.md). The group-photograph checklist is [`M8-DESKTOP.md`](M8-DESKTOP.md). The B dashboard checklist for the pages after Solve is [`REDESIGN-PR2.md`](REDESIGN-PR2.md). The 1.3.1 fix list is [`REDESIGN-FIX-1.3.1.md`](REDESIGN-FIX-1.3.1.md).

## Install

Two copies can exist. They share one library.

| Copy | How it starts | Binary |
| --- | --- | --- |
| Source build | `./target/release/holdover`, or a binary you copied to `~/.local/bin/holdover` | Whatever you built |
| Package | The application menu runs `/usr/bin/holdover` | `/usr/bin/holdover` and `/usr/bin/holdover-cli` |

The library is `$XDG_DATA_HOME/holdover/holdover.db`, usually `~/.local/share/holdover/holdover.db`. PDF files for Documents sit beside it in `holdover.blobs/`. `HOLDOVER_DB` points both the window and the blobs at another file.

A shell uses `PATH`. If `~/.local/bin` comes before `/usr/bin`, typing `holdover` still runs the source copy. The menu does not: its entry is `/usr/bin/holdover`. To make the shell match the package:

```sh
mv ~/.local/bin/holdover ~/.local/bin/holdover.from-source
mv ~/.local/bin/holdover-cli ~/.local/bin/holdover-cli.from-source
```

Skip a `mv` when that file is absent. `which -a holdover` shows every copy.

### From the repository

On Omarchy, from the repo root. Rust is mise's `rust@1.98.1`.

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover -p holdover-cli
./target/release/holdover
```

### From the PKGBUILD

The recipe is `packaging/PKGBUILD`. It builds this checkout and installs the window, the CLI, a desktop entry, a scalable icon, the MIT licence, the English strings, and `/usr/share/holdover/holdover.json.tpl`. It does not copy anything into `~/.config`.

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
cd packaging
mise exec rust@1.98.1 -- makepkg -si
```

`mise` is rustup, so the recipe keeps `rust-toolchain.toml` and builds with Rust 1.98.1. A distro `cargo` that is not rustup drops that file during the build and uses the distro compiler. After `pacman` finishes, the post-install note repeats the `PATH` warning.

The desktop file is `se.hallgren.holdover.desktop`. The icon name is `se.hallgren.holdover`. Reopen the launcher if the icon was cached. The English strings you see are compiled into the binary. The package also installs a copy at `/usr/share/holdover/locales/en.ftl` for reading; editing that file does not change the window.

The theme template is optional. Copy it only when you want Omarchy to render Holdover's colours:

```sh
cp /usr/share/holdover/holdover.json.tpl ~/.config/omarchy/themed/
```

A sample theme-set hook is `/usr/share/holdover/hooks/theme-set.d/holdover`. Copy it to `~/.config/omarchy/hooks/theme-set.d/` only when you want that stamp file. The window already follows the live theme without the hook.

This package is not published to the AUR.

## Library, units, and backup

First launch asks you to pick Metric or US and can add the example .308 profile from `examples/308-g7.json`. That profile is a repository example, labelled as such.

The header switches Metric, US, and Custom on every page. A typed suffix wins over the chip. `gr` is grains and `g` is grams. Changing the preset redraws the page from the stored SI value. It does not rewrite a clean field.

Settings, Library file, saves JSON or CSV. `schema_version` stays 1. A backup from Milestone 5 still restores: missing `documents`, `load_logs`, and `history` are empty. A new backup adds those sections. Rifle serials are cleared on export. Precise coordinates are omitted unless Include precise coordinates is checked. Nothing is written until Replace or Merge.

PDF bytes are not in the JSON or the CSV. Copy `holdover.db` and `holdover.blobs/` together when you want the files on another computer. A restored row whose PDF is absent opens with “The library row is here. The PDF file is not on this computer.”

## Screens

Solve keeps the profile, the summary line, truing, range, wind, direction, and inclination beside the hold. Air, the moving target, and the effect checkboxes are tabs under the hold, so that top stays on screen. The range card, charts, the reticle, targets, point-blank range, truing, and equipment behave as in Milestones 2–5. This guide covers what Milestone 6 adds, plus the controls that apply everywhere. Point-blank shows the five ranges in one row above the chart. The explanations sit under the chart.

### Compare

Compare reads two to six profiles at one range and one wind. The first table is the equipment. The second is drop, elevation, windage, velocity, energy, time of flight, and Mach.

The air is the current air from Solve. The inclination is the last inclination from Solve. Cant and latitude stay at zero on this page, so a Coriolis or cant check here is a zero-angle shot. Spin drift and aerodynamic jump follow the checkboxes. The spin-drift sentence and the jump note stay under the checkboxes. Compare does not write Solve’s remembered range or wind.

One profile is shown on its own, with a line asking for a second. A seventh box will not stay checked.

### Documents

A document is a title plus an `http` or `https` link, a PDF, or both. Edition and edition date are the text you type. Holdover does not download the link. Open hands the link or a temporary copy of the PDF to `xdg-open`. The PDF is stored and is never run.

A PDF has to start with `%PDF`, end in `.pdf`, and be 32 MB or smaller. The bytes are stored by SHA-256 under `holdover.blobs/` next to the database. Two documents with the same bytes share one file. Archiving a document keeps the file, so Restore can open it again.

### Load log

Equipment, Load log, is a notebook of your own loads: powder, primer, case, bullet lot, the charge you recorded, COAL, notes, and links to chronograph sessions and documents. The screen states this once:

> Holdover does not generate recipes, interpolate or extrapolate charges, predict pressure, or suggest ladder nodes. Use a published, pressure-tested source, and file that source under Documents. The charge on this page is the number you wrote down. It is not used to calculate anything.

The solver uses the muzzle velocity stored on the load. “Set as load MV” copies a session mean only after you confirm. The charge field does not change that velocity and does not change a solve.

### Archive and older versions

Archive is a soft delete. The row stays in the library with a timestamp. Equipment, Archive, lists those rows and can restore them. A rifle, scope, bullet, or load that a live profile or load still uses cannot be archived until that link is gone.

Each save that changes a live record keeps the previous copy. Archive, Older versions, lists the newest 100. Restore asks, writes that copy back, and keeps the current body as a new older version. Restore an archived record before restoring an older version of it. Truing still has its own activate and revert.

### Sync

Sync copies this library to another Holdover library as a `.holdover` file. The file is every record plus the document PDFs those records use. Move it however you like. The other library shows a preview before anything is written.

Export is on Sync. The file lands in the backups folder next to the database. With the usual library that is `~/.local/share/holdover/backups/`. With `HOLDOVER_DB=/tmp/holdover-m7/a.db` it is `/tmp/holdover-m7/backups/`. Rifle serials are left out. Precise coordinates are left out unless Include precise coordinates is checked. The weather place stays on this computer.

Import lists each record as new, updated, deleted, unchanged, or in conflict. A conflict needs Keep mine, Take theirs, or Keep both before Apply. Keep both is there when both copies are still in use and the row is not an older version. Apply is one step. If it fails, the library is unchanged. This computer's device id is not replaced.

A newer bundle, a file that is not a Holdover bundle, and a damaged or cut-off file are refused. Nothing is written. Two libraries on one computer use two `HOLDOVER_DB` paths. The steps are in [`M7-DESKTOP.md`](M7-DESKTOP.md).

### About

About is in the status bar, on Settings, and in the Ctrl+K list. It shows version 1.3.1, the MIT licence (Copyright (c) 2026 Patrik Hallgren), the Open-Meteo CC BY 4.0 attribution, the personal non-commercial note, the JBM drag-table source, the direct crate licences (including zip 2.6.1, image 0.25.10, and kamadak-exif 0.6.1), and the known limitations below. py-ballisticcalc 3.1.1 is a CI oracle under LGPL-3.0 and is not installed with the app. The window has no npm packages. WebKitGTK and GTK come from the system.

## Keyboard, type, and themes

Tab order starts at Skip to content, then the screen buttons, the unit presets, the page, and About in the status bar. The focused control draws a ring in the theme focus colour. Buttons and fields have a visible label or a tooltip. Ctrl+K opens the command palette and focuses the search field. Escape closes that list and the units popover. Ctrl+U cycles Metric, US, and Custom. Letter shortcuts on Solve stay quiet while a field is focused.

Type size follows the Omarchy text-size knob (`text-scaling-factor`). Settings, Larger text, adds a step on top. `HOLDOVER_TEXT_SCALE` overrides the knob when you set it. Drawings and text use the theme colour variables. Secondary text, warnings, errors, and the focus colour are lifted when a theme would leave them below the usual contrast floors (4.5:1 for text, 3:1 for the focus ring) on both the background and the surface.

Field mode (F11, or Field in the Ctrl+K list) hides the header, the rail, and the status bar so the hold can fill the window. Ctrl+K still opens the command palette.

## Printing

A range card or a target sheet that you will measure is a PDF written by the Rust printer, then opened for the system viewer. Print that file at 100 percent, Actual size, with fit-to-page off, on the same paper the sheet names. On the HP M276nw, that is US Letter when the sheet says Letter. The window’s print button remains a fallback and does not produce the ruler-sized page. WebKit’s print path follows `GDK_SCALE` and the monitor scale.

## Weather

Fetch weather on Solve is the only network call, and only when you press it. It gives up after 4 seconds and does not block the rest of the window. Attribution, shown on Solve, Settings, and About:

> Weather data by Open-Meteo (https://open-meteo.com/), CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).

The free tier is personal and non-commercial.

## Known limitations

These are open. The app badges the target items and repeats the sentences on About. No measurement below has been filled in.

- **T2.** The moose hit-area outline is a placeholder. It waits on a square photograph with a ruler, or the width, height, and edge measurements in [`PLAN.md`](PLAN.md) §33.
- **T3.** The moose silhouette size, and where the rings sit on it, are unmeasured.
- **T4.** Roe-buck rings of 10, 15, and 20 cm are the published figures. They are not confirmed on an owned target. The oval is a stand-in.
- **T5.** Other owned official targets are unmeasured.
- **T6.** Red deer and fallow deer have no official diameter in the sources used here, so none is invented. A diameter you type is your choice, and the T6 badge stays.
- **US rulebook targets.** IPSC/USPSA, IDPA, and steel have no confirmed size. That waits on ruler photos. Those photos will take a while.
- **Spin drift.** The solver uses the cited equation, and the coefficient is unverified against the book. The About screen shows the stored sentence: “cited as equation 6.1 of Litz, Applied Ballistics for Long Range Shooting, 3rd ed. (2015); the book is not in this repository, so the coefficient is not marked verified against the primary text.”
- **Aerodynamic jump.** The flag is accepted. The shot is solved without it, and the warning says aerodynamic jump was requested and not applied.
- **Recoil ejecta.** The default is the stored sentence: “Constant 1219.2 m/s (4000 ft/s). This is a common published powder-gas assumption. It has not been confirmed against the SAAMI publication.”

Group photographs, the phone companion, and manufacturer bullet catalogues are later work. The `.holdover` bundle is the desktop copy those later steps can reuse. They are not a phone app in this version.
