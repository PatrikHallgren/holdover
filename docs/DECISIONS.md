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
- Spin drift uses Litz's `1.25·(SG+1.2)·TOF^1.83` inches, converted to metres. It is on when twist and bullet length are both known. A right-hand twist drifts to the right. The coefficient is the formula in the plan; it has not been checked against Litz's primary text.
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
- Tauri 2 remains the preferred shell. It is not a Cargo workspace member and CI does not compile it, because this environment has no WebKitGTK and no Omarchy session. Qt 6 Quick stays the documented fallback and was not started.
- SQLite begins in M2. M0 only drafts the JSON schema, including the sync fields.
- The theme adapter reimplements the Omarchy v4.0.4 colour mix (`int(a·(1−t) + b·t + 0.5)` in straight RGB). It does not shell out to Omarchy.
- py-ballisticcalc 3.1.1 is a CI oracle only.

## Targets

- `SJF_MOOSE_RING_DIAMETER_CM` is `[15, 25, 33, 41.5]`. The older 15/25/35/45 cm set is not a preset.
- Roe-buck rings `[10, 15, 20]` cm are published and still waiting on T4.
