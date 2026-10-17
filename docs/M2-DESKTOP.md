# Milestone 2 desktop checklist

This is the Solve screen in the Tauri 2 app: a rifle and load profile, a zero atmosphere kept apart from the current air, a target range and wind, and a large hold with clicks. The header unit switch is on every page. CI compiles the app and runs the unit round-trip. It does not launch a window. The shell checks that already passed are in [`M1-DESKTOP.md`](M1-DESKTOP.md).

## Commands

On the Omarchy machine, from the repo root. Rust is mise's `rust@1.98.1` with no global default.

Packages required to build and run. This does not install them:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
```

`curl`, `wget`, `file`, `appmenu-gtk-module`, `libappindicator-gtk3`, and `xdotool` are not required. A missing `wget` must not stop the build.

```sh
mise exec rust@1.98.1 -- cargo build --release -p holdover
./target/release/holdover
```

The library is `$XDG_DATA_HOME/holdover/holdover.db`, or `~/.local/share/holdover/holdover.db`. Backups are written next to it, under `backups/`. For a throwaway library:

```sh
HOLDOVER_DB=/tmp/holdover-m2.db ./target/release/holdover
```

`tools/desktop-spike.sh` still builds this same binary and checks a theme-directory rename. It is the M1 check, not a substitute for the list below.

The theme template is optional. Without it, a live Omarchy switch is read from `colors.toml`:

```sh
mkdir -p ~/.config/omarchy/themed
cp share/holdover/holdover.json.tpl ~/.config/omarchy/themed/
```

## What to check

- **Onboarding.** A new database asks for Metric/Nordic, US, or Custom. Custom shows a control for each dimension. The example `.308` can be included or left off. Accept is disabled until a preset is chosen, and the disclaimer is on the page.
- **Unit switch.** Metric, US, and Custom sit in the header on Solve, Range card, Charts, Equipment, and Settings. Switching Metric to US changes the shown range, temperature, pressure, and equipment numbers. `Ctrl+U` cycles the presets. A clean field that is saved again keeps the original text: a sight height entered as `45 mm` is still `45 mm` in the backup after a US switch.
- **Per-field units.** The chip beside a field cycles that field's display (`m`, `yd`, `mm`, and the rest of its dimension). A typed suffix is converted in the preview, for example `478 yd` on a metre field. A bare number uses the chip. `175 gr` and `175 g` are different. `gr.` is rejected.
- **Profile.** Equipment can save a rifle (twist and hand, sight height), a scope (FFP or SFP, click value), a bullet (G1 or G7, a second BC band), a load (muzzle velocity and its temperature), and a profile that ties them to a zero. The zero's own air is edited on the profile, not on Solve.
- **Current air stays off the zero.** On Solve, set the current temperature to `30 C` (or `86 F`). The hold changes and the page says the current air differs from the zero. The profile's zero temperature is still the value you saved, such as `15 C`. Pressure has Station and Sea level. Humidity takes `50%` or `0.5`. Density altitude is on the hold. Station pressure near `1013 hPa` with altitude above `500 m` shows a warning.
- **Hold.** Elevation and windage are large tabular numerals, with a direction and a click count. `U` (when a field is not focused) cycles mrad, MOA, IPHY, cm/100 m, and clicks. Wind direction accepts `3oc`, `9`, and degrees. Incline and spin drift start on. Coriolis, cant, and aerodynamic jump start off. Turning aerodynamic jump on warns that it is not applied. SG appears when twist and bullet length are both set.
- **Range card.** Build a card. The elevation column is the no-wind hold. The extra columns are crosswinds from 9 o'clock, parsed from the bracket field (`2 mps, 4 mps` or `5 mph, 10 mph`). Print uses the browser print dialog. Pinning the card keeps its length column in metres while the rest of the app follows the header.
- **Charts.** Check the active profile and up to two others. Drop and wind drift both draw, and a third overlay is not added. Each chart has a range axis and a drop or drift axis, with numbers on the ticks, light gridlines, and a zero line (the line of sight on the drop chart). The axis units follow the header preset: range in metres or yards, drop in centimetres or inches, wind drift in the hold unit (mrad, MOA, and the other angular units). The legend names each profile. Point at a chart, or focus it and use the arrow keys, to read the values at that range. Switch Metric and US, and light and dark, and tile the window narrow: the numbers stay readable.
- **Backup.** Settings writes `backups/holdover-….json` and can show the JSON. A rifle serial is empty in that file. Restore shows a preview first. Replacing the library is a second, explicit step.
- **Theme and type.** Follow Omarchy, then force Light and Dark. Change the Omarchy theme with Follow selected: the window follows without a restart. Larger text and reduced motion do what they say. `HOLDOVER_TEXT_SCALE=1.5` grows the type. This build reads that variable first.
- **Keyboard, on Solve, with no field focused.** `R` `W` `D` `A` focus range, wind speed, direction, and inclination. Up and down step the range (Shift is four steps, Ctrl is a tenth). Left and right step the clock. `+` and `-` step the wind. `Ctrl+P` focuses the profile. `Ctrl+K` opens the command list. `Ctrl+Shift+C` copies the hold. `F11` is field mode: the hold grows and the inputs hide. Shortcuts do nothing while you are typing in a field, except `Ctrl+U`, `Ctrl+K`, `Ctrl+P`, `Ctrl+Shift+C`, and `F11`.
- **Targets.** Settings lists the SJF moose rings `15 / 25 / 33 / 41.5` cm and the roe-buck rings `10 / 15 / 20` cm. The older `15 / 25 / 35 / 45` set is not offered.

## Not in this build

Point-blank range, lead, the impact view, truing, recoil, reticles, weather, chronograph statistics, and true-scale card PDF or PNG are later milestones. The rings above are the published diameters, not a target drawing.
