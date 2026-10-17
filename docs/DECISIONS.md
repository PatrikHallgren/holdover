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
