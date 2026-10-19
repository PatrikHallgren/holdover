# Milestone 3 desktop checklist

This is the field toolkit on the same Tauri 2 window as Milestone 2: a true-scale range card, point-blank range, moving-target lead, chart overlays and an impact view, truing, recoil, and chronograph statistics. CI compiles the app and runs the solver checks. It does not launch a window. The shell checks that already passed are in [`M1-DESKTOP.md`](M1-DESKTOP.md) and [`M2-DESKTOP.md`](M2-DESKTOP.md).

## Commands

On the Omarchy machine, from the repo root. Rust is mise's `rust@1.98.1` with no global default.

Packages required to build and run. This does not install them:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
```

`curl`, `wget`, `file`, `appmenu-gtk-module`, `libappindicator-gtk3`, and `xdotool` are not required. A missing `wget` must not stop the build.

```sh
mise exec rust@1.98.1 -- cargo build --release -p holdover
HOLDOVER_DB=/tmp/holdover-m3.db ./target/release/holdover
```

The library is `$XDG_DATA_HOME/holdover/holdover.db`, or `~/.local/share/holdover/holdover.db`, unless `HOLDOVER_DB` is set. Backups are written next to that file, under `backups/`. `/tmp/holdover-m3.db` is a throwaway library for this checklist.

The command-line checks use the same solver:

```sh
mise exec rust@1.98.1 -- cargo build --release -p holdover-cli
./target/release/holdover-cli pbr --profile examples/308-g7.json --zone 15cm
./target/release/holdover-cli chrono --shots "800, 800, 800, 800, 800, 800, 800, 900 mps"
./target/release/holdover-cli recoil --rifle "8 lb" --bullet "150 gr" --charge "45 gr" --mv "2800 fps"
```

## What to check

- **Card, true scale.** On Range card, choose A4. Build the card. The bar under the header is labelled `10 cm` and is drawn 100 mm wide. Print, or save as PDF, at **100%** (not fit-to-page, not shrink to fit). Measure the printed bar with a ruler: it should be 10 cm. A5 and Letter use the same 10 cm bar. A6 and Compact are narrower than 11 cm, so their bar is labelled `5 cm` and drawn 50 mm wide. The page size follows the choice (A4 is 210 × 297 mm). The footer includes “Predicted values; confirm with live fire.” CSV and JSON download from the same card. Extra temperatures, for example `-10 C, 30 C`, add no-wind elevation columns.
- **Point-blank range.** Open Point-blank. Moose inner is 15 cm and roe-buck inner is 10 cm; both can be edited. Compute. Near is before the optimal zero, and the optimal zero is before far. The chart shows the path between the two zone lines. Apply as zero shows the new zero and does not write it until Confirm zero.
- **Lead.** On Solve, open Moving target. Älgbana fills 5.3 m/s at 80 m. Viltmålsbana fills 2 m/s at 50 m. Vildsvinspasset can use the moose speed, 5.3 m/s, or the fast run, which stays blank because that speed is not published here. Direction uses the wind clock: 3 is to the right, 9 is to the left, 12 is away. A reaction time is labelled as your assumption. The hold shows the lead and the lead combined with the wind.
- **Charts and impact.** Charts accepts the active profile and up to five overlays (six series). Drop and wind start on; velocity, energy, time of flight, and Mach can be switched on. A shaded band starts at the first series’ transonic range. Point or use the arrow keys to read a range. Impact draws the published rings at true scale in centimetres, the predicted impact, wind-bracket marks, and the ± muzzle-velocity SD, ± range error, and ±1 m/s wind bars. Moose and roe show an “Outline not measured” badge. SVG, PNG, and CSV download from the chart page.
- **Truing.** Preview a muzzle-velocity fit, a drag fit, or a tracking fit. Nothing is written until Apply truing. A change past about 3% of muzzle velocity or about 10% of BC shows a warning. Muzzle-velocity guidance says “Before transonic”. Drag guidance says “True MV first”. Tracking says it affects clicks only. History can activate or revert, and each of those asks before it writes. The load’s stored muzzle velocity stays as it was; truing lives on the profile and is kept when the profile is saved again.
- **Recoil.** Equipment → Recoil. Rifle weight is the weight as fired. Charge weight is typed here and is not saved. The ejecta line names the 1219.2 m/s (4000 ft/s) convention and says it has not been confirmed against the SAAMI publication. A reduction other than zero is labelled an estimate.
- **Chronograph.** Paste `800, 800, 800, 800, 800, 800, 800, 900 mps`. Statistics show mean 812.5 m/s and flag the last shot. That shot stays in the mean and the sample standard deviation. Save, Set as load MV, and Apply slope each ask before they write.

## Not in this build

Group statistics stay in a later milestone. The moose and roe outlines are still unmeasured, so the impact view draws rings only. Aerodynamic jump can be requested and is still not applied. Spin drift is still the cited Litz equation and is still not marked verified against the book. There is no chronograph device import and no reloading recipe.
