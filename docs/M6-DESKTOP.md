# Milestone 6 desktop checklist

This is equipment depth on the same Tauri 2 window as Milestones 2–5: compare, documents, the load log, archive and older versions, the accessibility pass, the Arch package, and About. The English guide is [`USER-GUIDE.md`](USER-GUIDE.md). CI compiles the app, runs the new library and compare tests, and builds the Arch package inside `archlinux:base-devel`. It checks that the package contains the window, the CLI, the desktop entry, the icon, the theme template, and the MIT licence. It does not install the package on Omarchy, open a window, switch a live theme, or call Open-Meteo.

Version 1.0.0 is the number in the crate and on About. The `v1.0.0` tag is not created by this change.

## Commands

On the Omarchy machine, from the repo root, on this branch. Rust is mise's `rust@1.98.1` with no global default.

Packages required to build and run. This does not install them:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
```

```sh
mise exec rust@1.98.1 -- cargo build --release -p holdover
HOLDOVER_DB=/tmp/holdover-m6.db ./target/release/holdover
```

`/tmp/holdover-m6.db` is a throwaway library. Finish onboarding and add the example profile. Your real library stays at `~/.local/share/holdover/holdover.db` until you start the app without `HOLDOVER_DB`.

The laptop context for this pass: Omarchy 4, Hyprland with a Lua config, 2880×1920 at scale 1.6, `GDK_SCALE=2`, often a dark theme. Launch Holdover the same way other GTK windows get that scale. A special Holdover scale is not required.

## What to check

- **Compare, two profiles.** Add a second load and profile (a faster muzzle velocity is enough). Open Compare. The intro mentions the current air, the last inclination, and that cant and latitude stay at zero. Tick both profiles. At 300 m and a 9 o’clock wind the second table shows drop, elevation, windage, velocity, energy, time of flight, and Mach. The faster profile drops less. One ticked profile keeps the line that asks for a second.
- **Compare redraw.** Edit the range. The tables update without a trip back to Solve, and Solve’s own range is unchanged when you return. Switch the header to US: range, wind, sight height, velocity, and elevation change unit. Switch back to Metric. Turn spin drift on and off and confirm the windage column changes. Turn aerodynamic jump on: the note under the checkboxes still says it is not applied, and the table line says it was requested and not applied.
- **Documents.** Save a document with a title, an edition, an edition date you type, and `https://example.com`. Open. The browser or handler starts, and the window stays usable. Save another with a small PDF. Open that one in a PDF viewer. A `file://` link is refused. Archive the PDF row, then Equipment → Archive → Restore, and Open again.
- **Load log.** Equipment → Load log. The boundary paragraph is on that tab and names recipes, interpolation, pressure, and ladder nodes. Enter a charge and a COAL. Save. The solver muzzle velocity line matches the load and does not change after the save. Solve the same profile: the hold matches a solve from before the log entry. If a chronograph session exists for that load, its mean is in the current velocity unit, and Set as load MV asks before it writes.
- **Archive.** On a rifle that no profile uses, change the name and save. Equipment → Archive lists the previous name under Older versions. Restore this version asks, then the rifle shows the old name. Archive the rifle. It leaves the live list. Restore brings it back. Restoring a version while it is still archived reports that the archived record comes first.
- **About.** Footer, About. The version line reads 1.0.0. The MIT text is on the page, with Copyright (c) 2026 Patrik Hallgren. Open-Meteo is named with both URLs and CC BY 4.0, and the personal non-commercial sentence is there. The known-limitations paragraph names T2, T3, T4, T5, T6, and IPSC. Under it, the spin-drift sentence says the coefficient is not marked verified against the primary text, and the ejecta sentence says 1219.2 m/s has not been confirmed against the SAAMI publication. Settings has the same About button. Ctrl+K includes About.
- **Keyboard and focus.** Tab from the window. Skip to content appears when it is focused and moves into the page. The focus ring is visible on buttons and fields in a dark theme and a light theme. Escape closes the Ctrl+K list and the units popover. Every new control has a visible label.
- **Themes and type.** Switch the Omarchy theme to a dark one, then a light one, while Compare, Documents, Load log, Archive, and About are open. Secondary lines, the jump note, and errors stay readable on the page background and on the panels. Move the Omarchy text-size knob. The type size follows it without a restart. Settings, Larger text, grows it again.
- **Package.** From `packaging/`:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
mise exec rust@1.98.1 -- makepkg -si
```

The menu entry Holdover starts `/usr/bin/holdover`. `which -a holdover` may still list `~/.local/bin/holdover` first. Leave that file in place to keep the source binary for the shell, or rename it so the shell matches the package:

```sh
mv ~/.local/bin/holdover ~/.local/bin/holdover.from-source
mv ~/.local/bin/holdover-cli ~/.local/bin/holdover-cli.from-source
```

Start the packaged window once with the real library (no `HOLDOVER_DB`). It should open the library you already have. Quit. The optional theme template is only:

```sh
cp /usr/share/holdover/holdover.json.tpl ~/.config/omarchy/themed/
```

Skip that copy when the template from Milestone 1 is already there. Holdover does not write `~/.config` itself.

## Tag, after this checklist

Do this only when you want the release. It creates an annotated tag on current `main` and pushes the tag. It does not create a GitHub release.

```sh
git checkout main
git pull origin main
git tag -a v1.0.0 -m "Holdover 1.0.0"
git push origin v1.0.0
```

## Still open

T2–T6 and the US rulebook targets stay on About and in the user guide. They wait on ruler photos and measurements. Spin drift stays unverified against Litz. Aerodynamic jump stays accepted and unapplied. The recoil ejecta default stays unconfirmed against SAAMI. No stand-in diameters were added for those items.

CI’s package job is a `makepkg` in an Arch container and a file-list check. A fresh install on this Omarchy 4 machine, the Hyprland menu, `GDK_SCALE=2`, a live theme switch, the text-size knob, and the HP M276nw are this checklist. The printer is unchanged from Milestone 3: a Rust PDF, opened and printed at 100 percent on the paper the sheet names.

## Not in this build

No group photographs. No phone companion. No AUR listing. No recipe, pressure, or ladder calculation. No second locale.
