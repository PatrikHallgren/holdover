# Reference vectors

These are the Milestone 0 checks. Closed-form rows are computed in the tests from the formulas below, not copied out of a previous solver run. The py-ballisticcalc rows are the CI oracle (`tools/oracle_check.py`, version 3.1.1, not vendored).

Common vacuum shot: 800 m/s, bore angle 0, no wind, no spin drift, `g = 9.80665 m/s²`. Path is the signed height in metres (negative is below the bore). Elevation is the positive-up hold, `atan(−path / range)` in milliradians.

| # | Case | Expected |
| --- | --- | --- |
| 1 | Vacuum, 100 m | `t = 0.125 s`, path `−0.076614453125 m`, hold `0.7661443813 mrad` |
| 2 | Vacuum, 300 m | `t = 0.375 s`, path `−0.689530078125 m`, hold `2.2984295464 mrad` |
| 3 | Vacuum, 1000 m | `t = 1.25 s`, path `−7.6614453125 m`, hold `7.6612954146 mrad` |
| 4 | Vacuum, 30° incline, `t = 0.5 s` | slant range `399.387084375 m`, path above the line of sight `−1.0616010033 m` |
| 5 | Vacuum zero at 200 m, sight height 0 | bore angle `½ arcsin(gR/v²) = 1.5322914610 mrad` |
| 6 | Same zero, line-of-sight crossing | absolute path error `≤ 0.1 mm` |
| 7 | Constant `Cd = 0.5`, `k = 1.3966124892624784×10⁻³`, no gravity, `t = 0.4 s` | speed `552.90010056 m/s` at `264.52175911 m` |
| 8 | Same constant drag, `t = 0.8 s` | speed `422.42395466 m/s` at `457.25087735 m` |
| 9 | G7 BC 0.243, 175 gr, .308 in, 800 m/s, 45 mm sight, 100 m zero, RK4 `dt = 0.002` vs `0.001` at 1000 m | elevation differs by `≤ 0.01 mrad` |
| 10 | Same shot, default Dormand–Prince vs RK4 `dt = 0.001` at 1000 m | elevation differs by `≤ 0.01 mrad` |
| 11 | Same shot, zero crossing at 100 m | absolute path error `≤ 0.1 mm` |
| 12 | 175 gr at 800 m/s, energy | `3628.73896 J` |
| 13 | Same state, energy | `2677.5896558979407 ft·lbf` (`grains·fps² / 450240`) |
| 14 | Miller SG, 175 gr, 0.308 in, length 1.24 in, twist 1:10 | `2.4603417624147976`. At 2800 fps, 59 °F, 29.92 inHg the correction factor is 1 |
| 15 | Dry ICAO sea level | density `1.2249781262066513 kg/m³`, speed of sound `340.2970287557621 m/s` |
| 16 | ISA station pressure at 1000 m from 101325 Pa | `89874.75552236482 Pa` |
| 17 | Saturated vapour pressure at 15 °C, Buck 1981 | `1705.1728361052074 Pa`. Wet density at 100% RH is `1.2171860761129472 kg/m³` |
| 18 | G1 Cd at Mach 1.0, table knot | `0.4805` |
| 19 | G1 Cd at Mach 0.93, PCHIP | `0.38017045` |
| 20 | G7 Cd at Mach 0.93, PCHIP | `0.17167206` |
| 21 | G1 at Mach 6 | clamped to the Mach 5 knot |
| 22 | 1 mrad | `(10.8/π) MOA = 3.4377467708 MOA`, `3.6` in/100 yd, `10 cm` at 100 m |
| 23 | 9 o'clock and 3 o'clock, 3 m/s, 400 m | windage magnitudes match and the signs oppose. Wind from the left drifts the bullet right |

Rows 1–11 and 23 are integration tests in `crates/holdover-core/tests/reference.rs`. Rows 12–22 are unit tests in the core crate. The path error on rows 1–3 is under `1×10⁻⁶ m`.

## py-ballisticcalc 3.1.1

Profile `examples/308-g7.json`: 175 gr, 0.308 in, G7 BC 0.243, 800 m/s, sight height 45 mm, zero 100 m, dry air at 15 °C and 1013.25 hPa, altitude 0, spin drift off. The oracle builds the same shot in py-ballisticcalc (twist 0, so its spin-drift term is off). Both zero angles are **1.2581 mrad**.

| # | Range | Holdover elevation | py-ballisticcalc | Δ mrad | Holdover m/s | py m/s | Holdover TOF | py TOF | Mach |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 24 | 100 m | 0.0000 | 0.0000 | 0.0000 | 738.74 | 738.73 | 0.1301 | 0.1301 | 2.171 |
| 25 | 200 m | 0.6759 | 0.6759 | 0.0000 | 680.22 | 680.21 | 0.2712 | 0.2712 | 1.999 |
| 26 | 300 m | 1.6097 | 1.6097 | 0.0000 | 624.55 | 624.53 | 0.4246 | 0.4246 | 1.835 |
| 27 | 500 m | 3.9682 | 3.9684 | −0.0002 | 520.58 | 520.55 | 0.7753 | 0.7753 | 1.530 |
| 28 | 800 m | 9.0111 | 9.0118 | −0.0007 | 380.80 | 380.75 | 1.4491 | 1.4492 | 1.119 |
| 29 | 1000 m | 13.9178 | 13.9195 | −0.0017 | 317.19 | 317.10 | 2.0335 | 2.0337 | 0.932 |

| # | Case | Result |
| --- | --- | --- |
| 30 | 3 m/s from 9 o'clock at 300 m | windage `0.14877 m` (Holdover) and `0.14879 m` (py-ballisticcalc). The angular difference of those two distances is `−0.0001 mrad` |

Plan §27.2 asks for ≤ 0.05 mrad supersonic and ≤ 0.1 mrad through the transonic region, and ≤ 0.5% on velocity and time of flight. The largest elevation difference in this set is 0.0017 mrad, at Mach 0.93. The largest velocity difference is about 0.03% (317.19 vs 317.10 m/s at 1000 m).

A release `holdover-cli solve` of this profile to 2000 m returned 60.1437 mrad, 224.70 m/s, 5.8587 s, Mach 0.660, in 0.91 ms median of five runs on the M0 build machine, including process start. Milestone 1 gates the in-process solve, after one warmup, at under 20 ms (`cargo test --release -p holdover-core --test timing`).

## Spin drift

`SD[in] = 1.25·(SG+1.2)·TOF^1.83` is cited as equation 6.1 of Bryan Litz, *Applied Ballistics for Long Range Shooting*, 3rd ed. (2015). The book is not in this repository, so the coefficient is **not marked verified against the primary text**. A page that quotes that equation number (<https://stonerphysics.net/spin-drift>, retrieved 2026-10-17) gives Sg 1.91 and 1.714 s as 10.4 inches. The formula returns 10.421 inches, which rounds to 10.4. Litz's own 2021 note gives about 8–9 inches at 1000 yards and a ceiling of 10–12, and does not print the closed form.

## Maker tables

| # | Source | Assumptions | Check |
| --- | --- | --- | --- |
| 31 | Berger Bullets, 7mm PRC 175 gr Elite Hunter product page (retrieved 2026-10-17). G7 BC 0.336, 2920 fps, 175 gr, 0.284 in. Drop at 100–600 yd: +1.5, 0, −6.48, −18.5, −36.66, −61.68 in. Velocity: 2779, 2641, 2507, 2377, 2251, 2129 fps. "24 inch barrel and sea level." | Sight height and temperature are not on the page. The test uses 1.5 in and 59 °F / 29.92 inHg dry, the pair that puts the 100-yard path on the published +1.5 in. Zero 200 yd. Spin drift off. Profile `examples/berger-7prc-175.json`. | Path within 0.02 mrad (measured about 0.005 mrad at 600 yd). Velocity within 0.3% (measured about 0.11%). |
| 32 | Federal Gold Medal .308 Win 175 gr SMK, GM308M2, velocity column as quoted with G1 BC 0.505: 2427, 2262, 2102, 1949, 1803 fps at 100–500 yd from 2600 fps. | Army Standard Metro (59 °F, 29.5275 inHg, 78% RH). Bore angle 0. The Federal page was not re-fetched; the column is the secondary quotation. Profile `examples/federal-gm308m2.json`. | Within 3 fps (measured 1.6 fps at 500 yd). |
