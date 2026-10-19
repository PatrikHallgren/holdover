# Decisions recorded in Milestone 0

The product plan is `docs/PLAN.md`. Where that plan left a choice open, M0 resolved it as follows.

## Solver

- The integrator runs in the world frame: x downrange, y up, z to the right. Incline is a full trajectory, not a rifleman's-rule scale of a flat solution. Sight height is perpendicular to the line of sight, and the requested range is slant range along that line.
- `BC_SI = BC[lb/in²] × 0.45359237 / 0.0254²`. The plan's rounded 703.07 is the same constant; the code uses the exact factor.
- Standard drag curves are the JBM/BRL Cd–Mach tables for G1, G2, G5, G6, G7 and G8, interpolated with Fritsch–Carlson monotone cubic Hermite (PCHIP), the same method py-ballisticcalc 3.1.1 uses. Mach outside the table is clamped and a warning is returned. py-ballisticcalc extrapolates instead, so the oracles are not expected to match past the last knot.
- JBM's GL file is Lowry's 1965 velocity-indexed G function, not a Cd–Mach table. It is not bundled. GI, GS and RA4 are not in M0.
- A multi-BC model is a step in airspeed: the band with the highest `min_mps` that is still at or below the airspeed is used.
- A custom Cd table uses sectional density `mass / diameter²` in the same `k = ρ·Cd·π / (8·BC_SI)` relation, with `BC_SI` replaced by that sectional density.
- The stored BC reference atmosphere (ICAO or Army Standard Metro) is recorded and does not scale drag in M0. Army Standard Metro is 15 °C, 29.5275 inHg and 78% relative humidity.
- Density and the speed of sound are constant along a shot. Altitude-varying atmosphere is later work.
- The default integrator is adaptive Dormand–Prince 5(4), relative tolerance `1e-7`, absolute tolerance `1e-8`, maximum step 0.05 s, with a cubic Hermite step to the exact slant range. Fixed-step RK4 is available for the convergence check.
- Zeroing uses Brent's method on the bore angle so the scope-vertical miss equals the requested offset. The path error at the zero range must be at most 0.1 mm. The implementation stops at `1e-5` m.
- Gravity is the standard `9.80665 m/s²` and does not vary with latitude in M0.
- Spin drift uses Litz's `1.25·(SG+1.2)·TOF^1.83` inches, converted to metres. It is on when twist and bullet length are both known. A right-hand twist drifts to the right. Milestone 1 checked the citation and left the coefficient marked unverified against the book itself. See the Milestone 1 note below.
- Aerodynamic jump is accepted as a flag and is not applied. The coefficients are unverified, and a solve with the flag on returns `AerodynamicJumpNotApplied`.
- Coriolis uses `−2Ω×v` with `Ω = ω·(cos φ cos A, sin φ, −cos φ sin A)` and `ω = 7.2921e-5 rad/s`. It defaults off.
- Cant rotates the sight offset and the launch direction about the line of sight. It defaults off.
- Point-blank range, lead, muzzle-velocity truing and drag truing return `NotInMilestone` for M3. Group statistics return `NotInMilestone` for M8.
- Powder temperature defaults to the air temperature of the atmosphere being solved when the profile does not set one.
- Positive elevation hold is up. Positive windage hold is right (the correction that moves the impact left when the bullet has drifted right). Wind `12` is a headwind, `3` comes from the right and `9` comes from the left: `w = (−speed cos a, 0, −speed sin a)` with `a` clockwise from 12.
- `mil` means milliradian. `1 mrad = (10.8/π) MOA`, `3.6` inches per hundred yards, and `10 cm` at `100 m`.

## Atmosphere

- ICAO: 15 °C, 101325 Pa, lapse `0.0065 K/m`, dry-air constant `287.058 J/(kg·K)`, vapour constant `461.495 J/(kg·K)`, `γ = 1.4`. Saturation vapour pressure is Buck 1981.
- Sea-level pressure (QNH) is reduced to station pressure with the ISA troposphere, independent of the actual temperature. If a station pressure is within 5 hPa of 1013.25 hPa and the altitude is above 500 m, the atmosphere is marked suspicious.

## Interface

- `holdover-core` does no file or network I/O. `holdover-cli` is the I/O boundary and prints the same JSON shape a later export can use.
- English strings live in `locales/en.ftl` from this milestone. The solver disclaimer is that file's `disclaimer` message.
- The desktop shell is Tauri 2. On 2026-10-16 the checklist in `docs/M1-DESKTOP.md` passed on Patrik's Omarchy 4 machine: the release build with mise Rust 1.98.1, a window on the live theme directory, live theme switch, display scaling, text scaling, tiling and float resize, and typing with the unit-suffix error. Qt 6 Quick is not the fallback. CI compiles the shell and does not launch a window.
- SQLite begins in M2. M0 only drafts the JSON schema, including the sync fields.
- The theme adapter reimplements the Omarchy v4.0.4 colour mix (`int(a·(1−t) + b·t + 0.5)` in straight RGB). It does not shell out to Omarchy.
- py-ballisticcalc 3.1.1 is a CI oracle only.

## Targets

- `SJF_MOOSE_RING_DIAMETER_CM` is `[15, 25, 33, 41.5]`. The older 15/25/35/45 cm set is not a preset.
- Roe-buck rings `[10, 15, 20]` cm are published and still waiting on T4.

## Milestone 1

- The release timing gate is `cargo test --release -p holdover-core --test timing`. After one warmup solve, a full solve to 2000 m must finish in under 20 ms. Debug builds keep the looser 200 ms check.
- The py-ballisticcalc 3.1.1 comparison stays in CI.
- Maker vectors are the Berger 7mm PRC 175 gr Elite Hunter drop and velocity columns (product page, sea level, G7 0.336, 2920 fps) and the Federal GM308M2 velocity column (G1 0.505, Army Standard Metro). Sight height and temperature are not printed on the Berger page; the test uses 1.5 in and 59 °F / 29.92 inHg because that is what reproduces the published +1.5 in path at 100 yards. The Federal column was taken from a secondary quotation of Federal's table, not re-fetched from Federal.
- Spin drift stays on the cited equation 6.1 (`1.25`, `+1.2`, exponent `1.83`). The book is not in the repo. A secondary page quotes that equation number, and Litz's 2021 Applied Ballistics note gives the typical inches at 1000 yards without the closed form. `SPIN_DRIFT_PROVENANCE` says the coefficient is not marked verified against the primary text. An Sg of 1.91 at 1.714 s rounds to the quoted 10.4 inches.
- The desktop binary is `holdover`. The CLI stays `holdover-cli`. A length without a suffix still fails, and the error names `100m`.
- `tools/desktop-spike.sh` treats `webkit2gtk-4.1`, `base-devel`, `librsvg`, and `openssl` as required. `wget` and the rest of Tauri's published Arch list are warnings. A missing `wget` does not abort the build.
- CI and this machine's toolchain pin are Rust 1.98.1, matching Omarchy's `mise exec rust@1.98.1`.

## Milestone 2

- The desktop window is the Solve screen, the range card, charts, equipment forms, settings, and first-run onboarding. The checklist is `docs/M2-DESKTOP.md`. Qt 6 Quick stays closed.
- The UI is one HTML page, `app/ui/app.js`, and `app/ui/app.css`. It is not Svelte or React, and it does not use Node. CI stays a Cargo workflow. English strings are `locales/en.ftl`, parsed as `key = value` and sent with bootstrap. That is the translation file; a second locale can be added later without a new parser.
- SQLite, via bundled `rusqlite`, lives only in the Tauri crate. `holdover-core` still does no I/O. Schema `user_version` 1 stores each rifle, scope, bullet, load, atmosphere, and profile as a JSON document, with `id`, `created_at`, `updated_at`, `revision`, `deleted_at`, and `device_id` on the row. There is no earlier schema to migrate. The database is `$XDG_DATA_HOME/holdover/holdover.db`, or `HOLDOVER_DB`.
- A quantity is stored as an SI number plus the original text. The header preset and the per-field chip change what is shown. They do not rewrite a clean field. Display conversion starts from the stored SI value. A typed suffix wins over the chip. `gr` is grains and `g` is grams.
- The profile owns the zero atmosphere. Solve edits current air and does not write that zero record. Powder temperature at zero is `Shot.zero_powder_temperature_c` when the profile sets one. The current solve uses the load's powder temperature when that is set, otherwise the current air, and only when a temperature sensitivity is set.
- The range card's elevation column is a no-wind solve. Wind brackets are crosswinds from 9 o'clock. Print is the WebKit print dialog. True-scale A6, A5, A4, Letter, PDF, PNG, and CSV stay in M3 and M5.
- Charts draw path and windage for the selected profile plus at most two overlays, under the current air, each with its own zero. The plan's later chart set (six overlays, transonic shading, a cursor, impact view) is not in M2.
- Swedish rings on Settings are the official diameters only: moose `15 / 25 / 33 / 41.5` cm and roe buck `10 / 15 / 20` cm. The plan's MVP line also mentions them as PBR and impact-view targets, and then puts PBR and the impact view in v1.0 rather than the MVP. M2 shows the diameters and does not draw targets or run PBR.
- Backup and restore are the library JSON. A rifle serial is cleared on export. Restore previews, then replaces records and settings and keeps `device_id`. CSV library files are M5.
- Aerodynamic jump can be requested and is still not applied. The truing badge stays "Not trued". The atmosphere badge stays "Manual". Coriolis and cant default off. Incline and spin drift default on.
- Spin drift is still the cited equation and is still not marked verified against Bryan Litz's book. See the Milestone 1 note.
- The command palette switches page, preset, field mode, and copies the hold. Letter shortcuts do not run while a field is focused.
- The optional example profile matches `examples/308-g7.json` and is labelled as a repository example, not a catalogue. The Solve screen still starts with the plan's effect defaults, so spin drift is on until it is turned off. The file used by the oracle keeps spin drift off.

## Milestone 3

The Milestone 0 line that `pbr`, `lead`, `true_mv`, and `true_drag` return `NotInMilestone` applied until this milestone. Those four, plus recoil, chronograph statistics, and scope-tracking truing, are implemented. `group_stats` still returns `NotInMilestone` for M8. The checklist is `docs/M3-DESKTOP.md`.

- Point-blank range is a still-air search. Brent sets the bore so the apex reaches `+r`, then the far limit is the descending `−r` crossing. Near is 0 when the path already starts inside the zone, which is the case when the sight height is smaller than the zone radius. The screen also shows the near zero (first rise through the line of sight) and the far zero (the descending line-of-sight crossing). A fixed-zero mode keeps the typed zero and reports the band and maximum ordinate that zero produces. “Apply as zero” is only offered for the optimal search. It recomputes that zero and writes `zero_range_m` only after `confirm`.
- Lead is `V · sin(direction) · TOF`, three iterations, with along-track motion changing the impact range. Direction uses the wind clock: 12 away, 3 right, 9 left. Reaction time is added only when the user enters one, and the screen says it is their assumption. Älgbana is 5.3 m/s at 80 m. Viltmålsbana is 2 m/s at 50 m. Vildsvinspasset offers the moose speed at 30 m, or a fast run whose speed is left blank: the SvSF/ISSF fast-run number is not invented.
- Truing re-zeros on each trial. Muzzle velocity and a single Cd multiplier are fitted with Brent. A Mach-band mode stores one scale per observation. The active snapshot sits on the profile (`mv_mps`, `cd_scales`, `tracking_factor`) and is applied by Solve, the card, charts, point-blank range, and lead. It does not rewrite the load’s stored muzzle velocity. Saving the profile from Equipment keeps that snapshot. Preview does not write. Apply, activate, and revert require confirmation. Warnings fire above about 3% of muzzle velocity or about 10% of BC. Guidance strings are “Before transonic”, “True MV first”, and “Affects clicks only”.
- A Cd scale multiplies drag inside its Mach band. An empty scale list leaves the entered BC unchanged, so earlier solves stay the same.
- Recoil uses `v = (m_b·v_b + m_c·v_e) / M` and `E = ½Mv²`. The grain check is an 8 lb rifle, 150 gr bullet, 45 gr charge, 2800 ft/s, and 4000 ft/s ejecta, which is 600000/56000 ft/s. The default ejecta is 1219.2 m/s (4000 ft/s). The screen says that convention has not been confirmed against the SAAMI publication. Charge weight is an input to this calculation only; it is not stored and it is not derived. A non-zero reduction scales the energy and is labelled an estimate. Brakes and suppressors are not modelled.
- Chronograph strings are parsed by hand. Statistics are count, mean, sample standard deviation (`n−1`), extreme spread, and min/max. A shot beyond two standard deviations is flagged when `n ≥ 4` and stays in the mean and the deviation. “Set as load MV” writes the mean and the session temperature only after confirmation. A powder slope is ordinary least squares across saved sessions for that load, and applying it also requires confirmation.
- The range card that is measured with a ruler is a PDF written in Rust. Page sizes are compact 90×50 mm, A6, A5, A4, and Letter. The `MediaBox` and the scale bar are in points (`mm × 72/25.4`, rounded to 0.01 pt), so 10 cm is 283.46 pt and Letter is 612 × 792 pt. WebKitGTK’s print path follows the screen scale (`GDK_SCALE` and the monitor scale), so a CSS millimetre bar does not print at 10 cm there. Open PDF writes the file and opens it; print that file at 100% / Actual size, on the same paper, with fit-to-page off. The window print button remains only as a fallback. The default size is Letter when `LC_PAPER`, `LC_ALL`, or `LANG` starts with `en_US`, or when the length unit is yards, and A4 otherwise. The chosen size is stored and remembered. CSV and JSON download from the card. PNG and SVG download from the charts and the impact view. The scale bar is 100 mm and labelled 10 cm when the page is at least 110 mm wide. A6 and compact use a 50 mm bar labelled 5 cm, because a 100 mm bar does not fit. Multi-condition columns are up to four extra temperatures, no-wind elevation only.
- Charts draw up to six profiles, with palette colour and a dash pattern. Checking a profile redraws. Drop, wind drift, velocity, energy, time of flight, and Mach are separate charts, and checking one redraws that series with its own axis unit. The transonic shade uses the first series. Chart labels use the theme colour variables. The point-blank chart has 300 m, 800 m, and full-range windows, plus wheel zoom and drag pan; the ticks are recomputed for the visible window, and the preset labels follow the length unit. The impact view draws the published ring diameters, the predicted point of impact, wind brackets from 9 o’clock, and sensitivity for muzzle-velocity SD, range error, and ±1 m/s from 9 and 3. Ring numbers, ticks, and the 10 cm label use the theme text colour, and the drawing fills the page width at a square aspect. Moose and roe outlines are still unmeasured, so those targets carry a placeholder badge and no silhouette. The controls under Impact are labelled; they set the target and the bars.
- Chronograph sessions are a new record kind in the same schema version 1. Export adds `chronos`. Import treats a missing `chronos` key as empty, so an M2 backup still restores. A rifle serial is still cleared on export.
- Spin drift remains the cited equation and is still not marked verified against the book. Aerodynamic jump is still accepted and not applied.
