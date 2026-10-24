# Redesign PR1 desktop checklist

Solve now uses the B dashboard: Omarchy tokens, the shared cards and steppers, and a hold that leads with the direction and the unsigned click count. Range card, Charts, Equipment, and the other screens keep their previous layout. They may pick up a new token, and they are not restyled here. That is PR2.

Version 1.2.0 stays the number in the crate, the window title, and the PKGBUILD. No tag and no release.

Use a throwaway library so the real one at `~/.local/share/holdover/holdover.db` stays put.

## Commands

On the Omarchy machine, from the repo root, on this branch. Rust is mise's `rust@1.98.1` with no global default. The installed 1.2.0 binary does not have this Solve page. Build this branch before the window checks.

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover -p holdover-cli
mkdir -p /tmp
HOLDOVER_DB=/tmp/holdover-r1.db ./target/release/holdover
```

Leave `HOLDOVER_DB` unset when you want the real library again. The laptop context is unchanged: Omarchy 4, Hyprland, 2880×1920 at scale 1.6 (1800×1200 logical), `GDK_SCALE=2`, a dark theme and a light theme. Tokyo Night and Catppuccin Latte are the two the screenshots use.

## What to check

- **Wide Solve.** At about 1800×1200 logical the window shows the rail, the hold, Trajectory, Sensitivity, Air, Moving target, Effects, the notes, and the status bar without scrolling the results. The hold digits are at least 48 px. UP / DOWN and LEFT / RIGHT lead, the click count is unsigned (`UP · 16 clicks`, `LEFT · 1 click`), and the exact angle sits small at the top of each readout. Speed on screen is `m/s`. `mps` is only the stored unit key.
- **Half tile.** At about half the width the same essentials stay on screen. Air, Moving target, and Effects collapse into one tabbed card. The nav stays one scrolling row. The context line under the hold stays one row and scrolls sideways if the unit buttons and Copy hold do not fit.
- **Phone width.** Narrower than 760 px the page is one column: hold, then the shot inputs, then the cards. Hit targets grow to 44 px. The screen list becomes Solve ▾, which opens the command palette. Key hints hide.
- **Invalid input.** Type `4 mp` into wind speed. The field turns red, the hint says it is not a speed, and the last hold stays dimmed with the reason. Arrow keys and the −/+ buttons do not rewrite that text. Clearing it back to `0` solves again.
- **Steps.** Up and down arrows step the range (`Shift` ×4, `Ctrl` ×0.1). Left and right arrows move the wind clock one hour. `+` and `−` step the wind speed and do not grow with Shift or Ctrl. Inclination steps 1°, temperature 1 °C (1.8 °F after a unit change), pressure 1 hPa, humidity 5 percentage points, target speed 0.5 m/s. On Equipment, zero range steps with the range step, and magnification steps by 1×. Every one of those fields still accepts typed text, including a unit suffix.
- **Theme.** Open Solve on a dark theme and a light theme. Small accent text (the UP / LEFT labels and the current sensitivity row) stays readable. Input edges stay visible. Skip to content appears when focused. A focused stepper shows the accent ring on the control, not a second ring inside the number.
- **Keys.** No shortcut uses SUPER. `R` `W` `D` `A` focus the shot fields when focus is not already in a field. `U` cycles the hold unit. `Ctrl+U` cycles the preset on every page. `Ctrl+K` opens the command palette. `Ctrl+P` focuses the profile list. `Ctrl+Shift+C` copies the hold. `F11` is field mode: chrome hides and the digits grow, and leaving it restores the page. The status line says `↑↓ range step` and `←→ clock`.
- **Spin drift.** With spin drift on and the wind at 0, the notes say the windage is spin drift. The long citation under Effects matches About and `holdover-cli` (the same sentence as `SPIN_DRIFT_PROVENANCE`).
- **Other pages.** Range card, Charts, Reticle, Targets, Groups, Point-blank, Truing, Equipment, Compare, Documents, Sync, and Settings still use the previous page layout. The unit switch in the header still changes their numbers. Equipment's zero range and magnification are the only new steppers off Solve.
- **About.** Footer, About. The version line reads 1.2.0. The window title is Holdover 1.2.0.

## Package

From `packaging/`, after the window checks if you want the menu binary updated:

```sh
sudo pacman -S --needed base-devel webkit2gtk-4.1 gtk3 openssl hicolor-icon-theme
mise exec rust@1.98.1 -- makepkg -si
```

The recipe version is 1.2.0. This does not create a tag.

## What the mockups do differently

- Inclination, temperature, pressure, humidity, target speed, and magnification are typed-only in the mockups. The approved steps are on the controls anyway.
- The profile control is the existing list (`Ctrl+P` focuses it), not a static name.
- The Effects card quotes the full spin-drift provenance, the same sentence as the CLI. A separate note is added when the wind is 0 and the windage is spin drift. The CLI solve text does not print that windage sentence, and the click sign inside the solver is unchanged.
- The status bar keeps the full disclaimer, shortened with an ellipsis. The tooltip is the whole sentence. Other pages still show the full disclaimer in the old footer.
- Stability (SG) is a display badge: at least 1.5 is stable, at least 1.0 is marginal, otherwise unstable. It is not a solver output.
- The angular segment says `cm/100 m`, from the existing string, rather than `cm@100 m`.
- The Air card keeps the real weather block (the offer, the attribution, and the non-commercial line), so it is taller than the mockup's short source line. On a wide window it fills the right-hand column beside Trajectory and Moving target.
- Headless captures use Noto Sans and JetBrains Mono. The window uses IBM Plex when Omarchy has it. The screenshot numbers are the mockup's illustrative hold (1.61 mrad, 0.08 mrad), not a live solve. A real `.308` at 300 m will differ.
