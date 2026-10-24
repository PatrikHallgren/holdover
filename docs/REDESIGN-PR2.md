# Redesign PR2 desktop checklist

Range card, Charts, Reticle, Targets, Groups, Point-blank, Truing, Equipment, Compare, Documents, Sync, Settings, About, onboarding, and the Ctrl+K palette now use the B dashboard. Solve keeps the PR1 layout. The shared pieces are the tokens, cards, label / number / unit rows, steppers, the clock picker, and the status bar. Page helpers such as `.cols`, `.big`, and `.legend` live in `app/ui/holdover-b.css`.

Version 1.3.0 is the number in the crate, the window title, and the PKGBUILD. No tag and no release.

Use a throwaway library so the real one at `~/.local/share/holdover/holdover.db` stays put.

## Commands

On the Omarchy machine, from the repo root, on this branch. Rust is mise's `rust@1.98.1` with no global default. The installed binary does not have these pages. Build this branch before the window checks.

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover -p holdover-cli
mkdir -p /tmp
HOLDOVER_DB=/tmp/holdover-r2.db ./target/release/holdover
```

Leave `HOLDOVER_DB` unset when you want the real library again. The laptop context is unchanged: Omarchy 4, Hyprland, 2880×1920 at scale 1.6 (1800×1200 logical), `GDK_SCALE=2`, a dark theme and a light theme. Tokyo Night and Catppuccin Latte are the two the screenshots use.

Accept the onboarding example so the library is the `.308` profile, not an empty database. The Berger 7 PRC plate in the mockups is only a picture. This window shows whatever is in the library.

## What to check

- **Shell.** Every onboarded page has Skip to content, the scrolling screen row, Metric / US / Custom, the unit button, and the status bar. The unit switch changes numbers on that page. No shortcut uses SUPER. `Ctrl+K` opens the palette. `Ctrl+U` cycles the preset. `Ctrl+P` focuses the profile list on Solve. `F11` hides the chrome. Below 760 px the screen row becomes the current page name with ▾, and that opens the palette. Cards and the rail stack to one column.
- **Wide and half tile.** At about 1800×1200 logical, and at about half that width, the essential result of the page is on screen without scrolling it away. Secondary panels are tabs. Inputs sit two or three to a row. A stepper still accepts typed text, including a unit suffix.
- **Range card.** From, To, and Step use the range step (`Shift` ×4, `Ctrl` ×0.1). The table is the preview. Wind columns lead with LEFT, then the angle, then the unsigned click count. The current range row is marked when it matches the last Solve range. Conditions and the scale check sit under the table. Save PDF still opens the Rust file. Print this page is the fallback and is not the ruler-sized card. The scale bar on screen is a label, not a print ruler.
- **Charts.** Trajectory and Impact are tabs, and both stay in the page. Drop and wind stay independent checkboxes. The readout lists quantities down the side and profiles across. With the chart focused, Left and Right move the cursor. Export still writes SVG, PNG, and CSV.
- **Reticle.** View and Editor are tabs. The range stepper uses the range step. Rings are the published diameters, centred on the hold. A dashed hit area stays dashed. The editor still saves and exports the reticle file.
- **Targets.** The list selects the face. T2–T6 and the US rows keep their placeholder badges. The moose outline stays dashed and is not scored. Ring diameters come from the library. Nothing on this page invents a diameter.
- **Groups.** Calibrate, Holes, and Context are the three steps. The photograph, the tools, and the stats sit in one row on a wide window. The mean point of impact is words (RIGHT / DOWN or the direction the solver returned) plus the millimetres, milliradians, and clicks from that group. An empty group does not invent an offset.
- **Point-blank.** Optimal and Fixed stay the two modes. The zero-range stepper is active only for Fixed zero. The zone lines are the warning colour. Apply still asks before it writes the zero.
- **Truing.** The rail is the observation. Preview shows the summary and the before / after snapshot from the fit. It does not invent a hold table. History activate and revert still ask before they write.
- **Equipment.** Rifle, scope, bullet, load, and profile stay separate tabs. Zero range steps with the range step. Magnification steps by 1×. Archive is the list of archived rows, not a merged form.
- **Compare.** Range, wind speed, and the wind clock are steppers. At this range and the equipment table read down the quantities, with one column per profile. An empty scope says "no scope". An empty twist says "no twist". Anything else missing says "not set".
- **Documents.** The add form is the rail. The library is the table. Rows are the documents in this database.
- **Sync.** Export and Import stay the two tabs. Import filters show counts. A conflict offers Keep mine, Take theirs, and Keep both. Apply stays disabled until every conflict has a choice.
- **Settings.** Three columns: appearance, steps and units, weather and library. Range step and wind step are typed fields and they are remembered. Larger text and reduced motion stay checkboxes. There is no language control and no 100 / 115 / 130 text-size control.
- **About.** The version line reads 1.3.0. The window title is Holdover 1.3.0. The limitations are the existing sentences, including the spin-drift provenance.
- **Onboarding.** The dialog sits over the inert shell. Metric, US, and Custom still work. The example choice maps to the same checkbox as before.
- **Palette.** Search filters the list and marks the match. Up and Down move. Enter runs the row. Escape closes it. The rows are the screens, the presets, field mode, and copy hold. It does not switch profiles.
- **Theme.** Open a data page and Settings on a dark theme and a light theme. Small accent text stays readable. Input edges stay visible. A focused stepper shows the accent ring on the control.

## Package

From `packaging/`, after the window checks if you want the menu binary updated:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
mise exec rust@1.98.1 -- makepkg -si
```

The recipe version is 1.3.0. This does not create a tag.

## What the mockups do differently

- The Settings picture shows a language switch and three text sizes. Those are not in this round. Larger text stays the one checkbox. There is no `sv.ftl`.
- The palette picture includes profile rows. This palette does not switch profiles. `Ctrl+P` still focuses the Solve profile list.
- The truing picture has a before / after hold table for a sample observation. The window shows the fit summary and the before / after snapshot the solver returned.
- Chart kind in the picture is one choice. The window keeps a checkbox per kind, and more than one can be on.
- Equipment in the picture is one combined profile. The window keeps a tab per kind.
- The on-screen card's click column is the preview. The PDF cell is unchanged.
- Range step and wind step had no control before. They are typed on Settings and stored with the other preferences.
- The conditions card uses the profile's zero summary. It does not invent a 15 °C line.
- The scale bar is a screen label. CSS millimetres are not a print ruler in WebKit.
- The status bar names `←→` for the compare clock and for the chart cursor. Page-level Left and Right still move the Solve wind clock. On other pages the stepper's own Left and Right stay the text caret. Up and Down, and the clock buttons, move the compare clock.
- Headless captures use a live solve of `examples/308-g7.json` where a page shows numbers. They are not the Berger plate.
