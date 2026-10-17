# Holdover: ballistic calculator and equipment app for Omarchy

Version: 0.4 · Prepared: 2026-10-16 · Status: planning document, no application code yet
Owner: Patrik Hallgren · Repository: <https://github.com/PatrikHallgren/holdover>

> **Changes since v0.3.** Patrik has answered the three remaining questions:
> - **Phone:** Phase 2 is **Android only**, installed by sideloading an APK. iOS is out of scope for now, so no Mac or Apple signing is needed.
> - **Licence:** **MIT.** Milestone M0 adds a `LICENSE` file (MIT, © 2026 Patrik Hallgren) to the repo.
> - **Moose target rings:** Patrik confirmed on 2026-10-17 that the current official SJF moose (älg) ring diameters are the moose preset: 5¹ = 15 cm, 5 = 25 cm, 4 = 33 cm, 3 = 41.5 cm. The 15/25/35/45 cm set is an older Swedish set no longer used much. To-collect item T1 is closed. Hit-area outline and silhouette measurements stay open (§17.6, §33).
> - **Moose drawing spec:** a drawing spec for the in-house moose figure is added, with a reference photo kept for personal use only (§17.6).
> - **To collect later:** a new, clearly visible list of measurements still to gather (§33).
>
> **Correction recorded when this plan was committed in M0 (2026-10-17).** Patrik confirmed that the Swedish SJF moose (älg) target ring diameters are the current official values (5¹ = 15 cm, 5 = 25 cm, 4 = 33 cm, 3 = 41.5 cm). The 15/25/35/45 cm set was an old one no longer used much in Sweden. SJF is the moose preset. The "unverified / user-provided" wording for rings 4 and 3 is removed, and to-collect item T1 is closed. The hit-area outline (T2), the silhouette measurements (T3) and items T4–T6 stay open. The official photo is not in this repository.
>
> **Changes in v0.3 (since v0.2).** Patrik has answered all ten open decisions, and the answers are now built into the plan. The main changes:
> - **Use:** personal, non-commercial.
> - **Language:** English only, with strings kept translation-ready.
> - **Units:** chosen at onboarding, with a unit switch on every page and on every field.
> - **Library and reticles:** user-entered bullet library; generic reticles plus an editor.
> - **Desktop target:** Omarchy 4 "Quattro" and later.
> - **Imports:** no import from other apps; only Holdover's own backup and export.
> - **Targets:** Swedish targets first, then a secondary US set.
> - **Later phase:** a phone companion with **photo-based shot-group analysis** and local-first sync.
>
> Material that only mattered for commercial distribution has been removed. The settled decisions are listed in §31.

> 📋 **To collect later:** physical measurements of the official targets are still needed for the outline and silhouette (T2, T3) and for T4–T6. Moose ring diameters (T1) are settled. See **§33** before building the target library in M4.

**Repository state (checked 2026-10-16 via the GitHub API, read-only):** `PatrikHallgren/holdover` is **public** and **empty**. It was created 2026-10-16 at 19:35 PDT, with default branch `main`. It has no commits with files, no description and **no licence file**. **Decision: MIT.** Milestone M0 adds `LICENSE` with the MIT text and the line `Copyright (c) 2026 Patrik Hallgren`. This plan only records that step; nothing has been changed in the repo.

---

## 1. Purpose and scope

Holdover is a personal, non-commercial **external-ballistics calculator and equipment notebook** for Omarchy (Arch Linux + Hyprland). It aims to be better-looking and easier to use than Strelok Pro, and is meant for legal sport shooting and hunting in Sweden and Washington State.

It covers:

- Saved profiles for rifles, scopes, loads and bullets.
- A 3DOF ballistic solver with optional advanced effects.
- Zeroing, holds, range tables and printable range (dope) cards.
- Point-blank range and moving-target lead.
- Trajectory and comparison charts.
- Truing, chronograph statistics and free recoil.
- Scope-matched reticles with holdover rendering.
- True-scale targets, prioritising **Swedish hunting and shooting targets**.
- Optional weather lookup.
- **Metric/imperial everywhere**, with conversion on every page.
- In a later phase: an **Android phone companion** and **shot-group photo analysis**. iOS is out of scope for now.

Strelok Pro is a functional reference only. Holdover copies none of its assets, reticles, databases or file formats. As checked on 2026-10-16, strelokpro.com lists mobile apps and a macOS/Windows desktop product, with no Linux build.

**Out of scope:**
- Importing from other apps (Strelok, chronograph apps, Kestrel).
- Bundled manufacturer data.
- Reloading recipe generation (§20).
- Any cloud service.

---

## 2. Product principles

1. **Local first.** Everything works offline, with no account and no cloud. Sync between desktop and phone is device-to-device or file-based (§23).
2. **The answer first.** The Solve screen leads with the hold in large type.
3. **Predictions, labelled as such.** Every output is a model prediction to be confirmed with live fire. Drivers of the result (atmosphere source, zero conditions, truing state) are always visible.
4. **Visible provenance.** Values are marked as user-entered, measured, fetched (weather), fitted (truing) or computed.
5. **No silent changes.** Truing and restores show a diff before anything is applied.
6. **Reversible.** Undo, archive, truing history and restorable backups.
7. **Units are a display choice.** Any input accepts any supported unit. Switching units never changes stored values (§22).
8. **Accessible by default.** Full keyboard operation, readable contrast and scalable type.
9. **Deliberate safety boundary.** Holdover never generates or optimises reloading recipes (§20).

---

## 3. Feature coverage

| Feature | Section | Milestone |
| --- | --- | --- |
| Rifles, scopes, bullets, loads, profiles | §6 | M2 |
| Ballistic solver (3DOF + effects) | §7 | M1 |
| Atmosphere (station vs sea-level pressure, DA, speed of sound) | §8 | M1 |
| Zero, drop, wind drift | §7, §9 | M1–M2 |
| Range tables and printable dope cards | §10 | M2–M3 |
| Point-blank range | §11 | M3 |
| Moving-target lead (incl. running moose/boar) | §12 | M3 |
| Charts and impact comparison | §13 | M3 |
| Truing | §14 | M3 |
| Recoil | §15 | M3 |
| Reticles (generic set + editor) | §16 | M4 |
| Targets (Swedish first, US secondary) | §17 | M4 |
| Weather (Open-Meteo, free tier) | §18 | M5 |
| User bullet/load library + own import/export | §19 | M2 (entry), M5 (library files) |
| Load log (reference only) | §20 | M6 |
| Chronograph statistics (manual entry) | §21 | M3 |
| Metric/imperial everywhere | §22 | M1 onward |
| Phone companion + sync | §23 | Phase 2 (M7 bundle, M9 phone, M10 LAN) |
| Shot-group photo analysis | §24 | Phase 2 (M8 desktop, M9 phone) |

---

## 4. Users and core jobs

| Job | Example |
| --- | --- |
| "Give me the hold, now" | Pick a rifle+load, enter 437 m and 3 m/s at 9 o'clock, read `UP 2.9 mil · LEFT 0.4 mil · 29 / 4 clicks` |
| "Print my dope card" | 100–600 m in 50 m steps with 2/4/6 m/s wind columns |
| "Moose test practice" | Älgbana, 80 m, figure crossing at about 5.3 m/s: lead in mil and in cm on the figure (§12) |
| "Longest PBR for roe deer" | 10 cm vital zone; the app suggests a zero and shows the PBR span |
| "Which load bucks wind better?" | Overlay loads on drift/drop charts |
| "My drops don't match at 600 m" | Enter observed dope, true MV then the drop scale, keep history |
| "Where do I hold on my reticle?" | Generic or self-drawn reticle at the current magnification, aim point marked |
| "How good was that group?" (Phase 2) | Photograph the target, mark the scale and holes, get ES / mean radius / offset in mil, saved to the load log |

---

## 5. Navigation and screens

Primary destinations: **Solve · Range Card · Charts · Reticle · Equipment · Compare · Documents · Settings**. **Groups** is added in Phase 2. Solve is the landing screen.

### 5.1 Global unit control (all pages)

- A **unit switch sits in the header of every page**. It offers Metric / Imperial / Custom presets and opens a per-dimension popover (e.g. metres + MOA + hPa).
- Switching converts everything on the page immediately: field labels, values, tables, charts, reticle readouts and exports.
- Stored values never change. Conversion always starts from the stored normalized value, never from a rounded display value.
- The page remembers the last preset globally. Individual screens can be pinned to a preset (e.g. range card always metric).
- Keyboard: `Ctrl+U` cycles presets, and `U` in Solve cycles the angular output unit (§5.2).

### 5.2 Solve (main calculator)

Three bands:

1. **Context bar.** Profile picker (rifle + scope + load), zero summary, atmosphere-source badge (Manual / Fetched 14:02 / Measured), truing badge and the page unit switch.
2. **Inputs panel.** Range, wind speed, wind direction (clock or degrees), inclination, target speed/direction (collapsible), temperature, pressure (with station/sea-level kind), humidity and altitude. Advanced toggles: spin drift and incline (both **on**), and Coriolis, cant and aerodynamic jump (all **off**).
   - **Every input accepts either unit system.** Type `437 m`, `478 yd`, `3 m/s`, `7 mph`, `12 °C`, `54 °F`, `29.92 inHg` or `1013 hPa`. Each field also has a **unit toggle chip** (e.g. `m | yd`) that switches that field's display and converts the shown value.
   - A bare number uses the field's current unit.
   - A typed suffix is converted and shown as `478 yd → 437 m` until the field loses focus.
3. **Hold panel (dominant).**
   - Elevation and windage in very large tabular numerals with direction words and clicks.
   - A secondary line with velocity, energy, TOF, Mach, linear drop and an SG badge.
   - A sensitivity strip showing the result at ±1 range step and ±1 wind step.
   - Output unit cycles between mrad, MOA, IPHY, cm@100 m and clicks, independently of input units.

**Field-use UX rules**

- The hold is readable from about 1 m on a 14″ laptop (digits at least 48 px by default, scalable).
- Results recalculate on every keystroke, in under 20 ms to 2000 m.
- Invalid input shows the last valid result dimmed with a reason. It never shows a blank or wrong value.

| Key | Action |
| --- | --- |
| `R` / `W` / `D` / `A` | Focus range / wind speed / wind direction / angle |
| `↑` `↓` | ± range step (`Shift` ×4, `Ctrl` fine) |
| `←` `→` | Wind direction ± 1 clock hour (or 15°) |
| `+` `-` | Wind speed ± step |
| `U` | Cycle angular output unit |
| `Ctrl+U` | Cycle unit preset (whole page) |
| `Ctrl+P` | Profile switcher |
| `Ctrl+K` | Command palette |
| `Ctrl+Shift+C` | Copy hold as text |
| `F11` | Field mode (high-contrast, hold-only, oversized) |

These shortcuts are app-local and never global. Omarchy 4 uses many `SUPER+…` combinations, so Holdover binds none.

### 5.3 Forms (Equipment, zero, truing, chrono, etc.)

The same rules apply to every quantity field in every form:

- typed unit suffix or bare number,
- per-field unit toggle chip,
- conversion preview,
- dimension check (mass can't go into a length field),
- decimal point or comma.

A form opened under the Imperial preset and saved shows identical stored values when reopened under Metric. This is an acceptance test.

### 5.4 Other screens

- **Range Card:** table, preview and print (§10).
- **Charts:** series, overlays and impact view (§13).
- **Reticle:** reticle at magnification with hold/lead/wind dots and a true-scale target (§16–17).
- **Equipment:** rifles, scopes, bullets, loads, profiles, reticles, targets, chrono sessions, load log and (Phase 2) groups.
- **Compare:** descriptive and ballistic comparison.
- **Documents:** attached PDFs/links, e.g. manufacturer manuals and load-data references.
- **Settings:** appearance (follow Omarchy / light / dark / larger text / reduced motion), units (onboarding preset, per-dimension defaults), solver defaults, weather, data (backup/restore/export) and privacy.

### 5.5 First-run onboarding

1. **Pick a unit preset:** Metric/Nordic, US, or Custom (per dimension). This can be changed at any time from any page header.
2. Optionally create the first rifle and load profile, guided step by step.
3. Read the prediction disclaimer (§27.4).

No account, no network and no telemetry.

---

## 6. Domain model

| Entity | Key fields |
| --- | --- |
| **Rifle** | Label, make/model, barrel length, twist rate and direction, sight height, weight (with/without scope), optional serial (private) |
| **Scope** | Make/model, magnification range, FFP/SFP, calibrated magnification (SFP), click value + unit (0.1 mrad, ¼ MOA, 1 cm@100 m…), tracking factor (tall-target test), travel limits, linked reticle |
| **Bullet** (user-entered) | Make/model, diameter, weight, length, drag model (G1/G7/G2/G5/G6/G8/GL/custom), BC(s) with velocity bands, BC reference atmosphere, optional custom Cd–Mach table, user's source note (e.g. "box label", "maker's website, 2026-10") |
| **Load / ammunition** | Factory product or own load record, bullet link, MV + measurement conditions, powder-temperature sensitivity, chrono links |
| **Profile** | Rifle + scope + load + zero + truing state |
| **Zero record** | Range, offset, separate zero atmosphere, powder temperature, date |
| **Atmosphere snapshot** | Temperature, pressure + kind, humidity, altitude, source, timestamp, location |
| **Truing record** | Observations → fitted parameters, before/after, active flag |
| **Chrono session** | Shots, stats, temperature, linked load |
| **Group** (Phase 2) | Photo, scale/transform, hole coordinates, distance, POA, statistics, linked profile/load/atmosphere/date |
| **Reticle** | Vector definition (§16) |
| **Target** | Vector outline at true size, scoring zones, source reference (§17) |
| **Location** | Name, lat/lon (precision-limited), elevation + source, privacy flag |

All quantities are stored normalized (SI) together with the original value, unit and precision. Every record carries a UUID, `created_at`, `updated_at`, a per-device revision counter and soft-delete tombstones **from the first schema onward**, so Phase 2 sync needs no migration of identity (§23).

---

## 7. Ballistic solver core

### 7.1 Model and frame

The solver uses a point-mass (3DOF) model with drag-function retardation. Spin drift and aerodynamic jump are empirical add-ons.

Frame: origin at the bore. x is downrange (the horizontal projection of the line of sight), y is up and z is right. Inclined shots resolve gravity in the line-of-sight frame. The state vector is (x, y, z, vx, vy, vz) with time as the independent variable.

### 7.2 Equations of motion

```
a = −k(M) · |v − w| · (v − w) + g + a_coriolis
k(M) = ρ · Cd_G(M) · π / (8 · BC_SI)        (BC_SI = BC[lb/in²] × 703.07 kg/m²)
M = |v − w| / c
a_coriolis = −2 Ω × v                       (only when enabled)
Custom Cd curve: k = ρ · Cd(M) · A / (2m),  A = π d²/4
```

Wind `w` includes the head/tail component. ρ and c are taken from §8 and held constant along the path in v1.

### 7.3 Drag functions and BC

- **Bundled standard tables:** G1 and G7 are required, plus G2, G5, G6, G8 and GL. These are public Cd–Mach data of BRL lineage, available from JBM's downloads page ("free for public use"). The source is recorded on the About screen. These function tables are the **only bundled ballistic data**.
- **Interpolation:** PCHIP or linear, with clamping and a warning outside the table.
- **Multi-BC:** velocity bands, applied as a step function by default, exactly as entered.
- **Custom Cd–Mach tables:** entered or pasted by the user, stored in the user library.
- **BC reference atmosphere:** a per-BC field, ICAO or Army Standard Metro, shown on the bullet form.

### 7.4 Integration

- Default integrator: adaptive Dormand–Prince 5(4) with dense output.
- Fixed-step RK4 as a reference for convergence tests.
- The solve terminates at max range, a velocity floor or a drop limit. The transonic crossing is reported as an event.
- Output is deterministic per build.

### 7.5 Zero solve

Brent's method finds the bore angle that crosses the line of sight at the zero range (plus offset), **using the zero atmosphere and powder temperature**, to a tolerance below 0.001 mrad. The current-conditions solve reuses that bore angle.

### 7.6 Inputs

| Input | Required | Notes |
| --- | --- | --- |
| MV (+ measurement temperature) | ✓ | Optional temperature table |
| BC + drag model, or custom Cd | ✓ | Multi-BC allowed |
| Bullet weight, diameter | ✓ | Length is needed for SG, spin drift and aerodynamic jump |
| Sight height | ✓ | |
| Zero range/offset + zero atmosphere | ✓ | Defaults to standard atmosphere with a warning |
| Twist rate/direction | for SG/spin drift | |
| Click value/unit, tracking factor | for clicks | |
| Range, wind | ✓ | Clock, degrees, or bearing + azimuth |
| Inclination | optional | Full solution |
| Cant | optional | |
| Latitude, azimuth | for Coriolis | |
| Target speed/direction | for lead | |

### 7.7 Advanced effects (settled defaults)

| Effect | Method | Default |
| --- | --- | --- |
| **Incline/decline** | Gravity resolved along and across the inclined line of sight. No rifleman's-rule approximations | **On** |
| **Spin drift** | Litz approximation `SD[in] = 1.25·(SG + 1.2)·TOF^1.83`, direction from twist hand | **On** (when twist and length are known) |
| **Coriolis + Eötvös** | `−2Ω×v` with `Ω = ω·(cosφ·cosA, sinφ, −cosφ·sinA)`, ω = 7.2921×10⁻⁵ rad/s | **Off**, available |
| **Cant** | Rotate sight offset and launch vector about the line of sight | **Off**, available |
| **Aerodynamic jump** | Litz empirical crosswind-to-vertical term. Verify coefficients against the primary text | **Off**, available |
| **MV temperature sensitivity** | Linear coefficient or table. Powder temperature defaults to air temperature | On if data present |
| **Stability (SG)** | Miller rule with velocity and atmosphere corrections (§7.8) | Always shown when computable |

Effect toggles are saved with each solve and printed on range cards.

### 7.8 Stability (Miller)

```
SG = 30·m / ( t² · d³ · l · (1 + l²) )     m: grains, d: in, t: calibers/turn, l: calibers
SG_corr = SG · (V/2800)^(1/3) · ((T_F + 460)/519) · (29.92 / P_inHg)     (station pressure)
```

### 7.9 Outputs

- Per range: drop, path, windage, elevation/windage correction in **mrad, MOA, IPHY, cm@100 m, linear and clicks** (rounded, with residual), plus velocity, energy, TOF, Mach, SG and spin-drift share.
- Angles use `atan(offset/range)`.
- Conversions: 1 mrad = 3.4377 MOA = 3.6 IPHY = 10 cm@100 m; 1 MOA = 1.0472 IPHY.
- Clicks = `round(θ / (click × tracking_factor))`, with a warning when the correction exceeds scope travel.
- Energy: `E[J] = ½mv²`; `E[ft·lbf] = W[gr]·v²/450 240`.

### 7.10 Library API and CLI

- `holdover-core` is a pure Rust library with no I/O and no UI types. Its functions: `solve_zero`, `solve`, `table`, `pbr`, `lead`, `true_mv`, `true_drag`, plus (Phase 2) `group_stats`.
- `holdover-cli` uses the same JSON schema as exports. Example: `holdover-cli solve --profile p.json --range 437m --wind 3mps@9oc --json`.

---

## 8. Atmosphere

- **Standard atmosphere:** ICAO (15 °C, 1013.25 hPa, 6.5 K/km, ρ₀ = 1.225 kg/m³). Army Standard Metro is available as a BC reference.
- **Station vs sea-level pressure is explicit.** Every pressure field has a kind selector. Sea-level (QNH) input needs an altitude and is converted to station pressure, with both values shown. If roughly 1013 hPa is entered as *station* pressure at an altitude above 500 m, the app asks "sea-level?".
- **Density:** `ρ = p_d/(R_d·T) + p_v/(R_v·T)`, with R_d = 287.058 and R_v = 461.495 J/(kg·K), and saturation vapour pressure from Buck.
- **Speed of sound:** `c = √(γ·R_d·T_v)`.
- **Density altitude** is shown and can optionally be used as an input.
- **Altitude source:** manual, Open-Meteo DEM elevation or a saved location (Phase 2: phone GPS/barometer). The source is labelled.
- **Tests:** ICAO table points, humidity density and station↔sea-level round trips.

---

## 9. Zeroing workflow

1. Create a zero with range, optional offset and **zero conditions** (a separate snapshot).
2. Current conditions are a separate snapshot. Both are shown when they differ.
3. **Re-zero for other ammo:** duplicate the profile and either confirm a new zero or enter the observed POI shift at the zero range.
4. A zero is flagged when the linked rifle, scope or load is edited after it was set.
5. Zero conversion helper: e.g. clicks to move from a 100 m to a 200 m zero.

---

## 10. Range tables and range cards

- Settings: start, end and step, chosen columns, units per column (following the page unit switch unless pinned), and precision tied to unit.
- Wind brackets: e.g. 2/4/6/8 m/s or 5/10/15 mph, optionally at half value.
- Card header: profile, zero, atmosphere, effects, truing and date. Footer: "Predicted values; confirm with live fire."
- Card sizes: compact stock/scope-cap card, A6, A5, A4 and Letter.
- Exports: PDF, PNG, CSV (unit-bearing headers) and the JSON solve record.
- Multi-condition cards: e.g. −10/+10/+30 °C, or Swedish lowland vs Washington mountain altitude.

---

## 11. Point-blank range

- **Input:** vital-zone diameter. Swedish presets come from §17 ring sizes, e.g. 15 cm for the moose inner ring and 10 cm for the roe buck inner ring, and remain editable.
- **Method:** optimise the bore angle so the apex reaches +r, then find where the path leaves −r.
- **Output:** near and far PBR, optimal zero range, maximum ordinate and its range.
- **Display:** ±r band chart. "Apply as zero" requires confirmation.

---

## 12. Moving-target lead

- `lead = V_t · sin(angle) · TOF(R′)`, iterated 2–3 times for along-line-of-sight motion. Output is in mrad, MOA and linear units at the target. The lead is drawn on the reticle and can be combined with the wind hold.
- **Swedish running-target presets** (speeds derived from published track specifications in §17):
  - **Älgbana, 80 m:** opening 22.75–23.25 m crossed in 4.3 ± 0.2 s, so roughly **5.3 m/s (≈19 km/h)**.
  - **Viltmålsbana, 50 m:** normal run of 10 m in 5 s, so **2 m/s**. The fast-run speed is to be taken from the SvSF/ISSF rules.
  - **Vildsvinspasset, 30 m running boar:** the speed depends on the track (50 m track at high speed, or half-screened älgbana at normal speed). The user selects which.
- Optional reaction/lock time is clearly labelled as a user assumption.

---

## 13. Charts and comparisons

- Series against range: drop/path, wind drift, velocity, energy, TOF and Mach, with the transonic region shaded.
- Up to 6 overlays (loads, rifles or conditions), distinguished by palette colour plus dash pattern.
- Keyboard-navigable cursor readout.
- **Impact view:** a true-scale target (§17) at a chosen range showing each load's predicted POI, a wind bracket spread and a sensitivity strip (±MV SD, ±range error, ±1 m/s wind). In Phase 2, measured groups (§24) can be overlaid on predictions.
- Export: PNG, SVG and CSV.

---

## 14. Truing

| Step | Fits | Data | Guidance |
| --- | --- | --- | --- |
| MV truing | Effective MV | Observed elevation at a supersonic mid-range | "Before transonic" |
| Drag truing | BC multiplier or per-Mach-band drop scale | Long-range observations | "True MV first" |
| Scope tracking | Tracking factor | Tall-target test | Affects clicks only |

- Fitting uses Brent's method. Drop scale is stored as a piecewise Cd multiplier against Mach.
- History with activate/revert.
- Guards: warn at MV changes above ~3% or BC changes above ~10%.
- Each observation records shots fired and group size. In Phase 2, a measured group (§24) can supply the group-centre offset directly.

---

## 15. Recoil

```
V_gun[ft/s] = ( W_b·V_b + W_c·V_e ) / ( 7000 · W_gun )      W_b, W_c: grains; W_gun: lb
E[ft·lbf] = W_gun · V_gun² / (2 · 32.174)
Metric: v = (m_b·v_b + m_c·v_e)/M ;  E = ½·M·v²
```

- Inputs: rifle weight *as fired*, bullet weight, charge weight (from the user's own load log or the box/spec sheet; never derived) and MV.
- Ejecta velocity is a named, editable convention. The SAAMI-style default must be confirmed against the SAAMI publication before shipping. The convention used is always displayed.
- Brakes and suppressors are not modelled. An optional "% reduction" is labelled as an estimate.

---

## 16. Reticles: generic set plus editor (settled)

- **Vector JSON format** in mrad or MOA, with elements line, hash, dot, circle, polyline and text. Fields include focal plane, calibrated magnification and a licence/source field (in-house or user-drawn).
- **FFP:** constant subtension. **SFP:** `actual = nominal × (calibrated_mag / current_mag)`, with a badge warning whenever the current magnification differs from the calibrated one.
- **Rendering:** apparent-FOV scaling. The view shows the hold point, wind dots, lead point and a true-angular-size target.
- **Generic set shipped with Holdover:** duplex, mil-dot, MOA hash, mil-hash, mil-tree and simple illuminated-dot variants, all drawn in-house.
- **Editor:** grid in mrad or MOA, snap to subtension, mirror and repeat tools, and preview at any magnification. Users can reproduce their own scope's reticle from its manual for personal use. Holdover ships no manufacturer-named reticles.
- Reticle files can be exported and imported **in Holdover's own format** (for backup or for moving between Holdover installs).

---

## 17. Targets: Swedish first, US secondary (settled)

### 17.1 Principles

- Every target is an in-house vector outline at **true physical size** with scoring zones, rendered at true angular size in Reticle, Charts/Impact and (Phase 2) Group analysis.
- **Target artwork may be protected.** The SJF rules describe several animal figures as *photographic images* ("fotografisk bild"), and printed targets are sold by Skyttetjänst. Holdover therefore **does not copy target images**. Ring diameters, track specifications and zone sizes come from the published rules. Outline shapes and ring positions are drawn generically. Anything not published as a number is marked **"to measure from the official target"**, for example by measuring a physical target.
- Swedish names are kept as the primary label (e.g. "Älgfigur, standard") with an English subtitle.

### 17.2 Swedish hunting targets (Svenska Jägareförbundet, SJF)

Sources: SJF *Tävlingsreglemente KM, LM, SM 2025–2028* §3 and *Märkesreglemente 2025–2028*, plus the SJF pages on the moose test and the practical big-game test.

| Target | Published data | Use / distance | Still to measure |
| --- | --- | --- | --- |
| **Älgfigur**, "Swedish standard figure for running-moose shooting" | **Current official SJF preset:** ring diameters **15, 25, 33, 41.5 cm** for rings 5¹/5/4/3. The older Swedish set 15/25/35/45 cm is not a preset. Two ringing schemes (larger: 5¹/5/4/3 with diagonal numbers + asymmetric hit area worth 2 points; smaller: 5/4/3/2 + hit area worth 1 point). SvSF 10-ring variant used at RM/SM/NM. Approved targets: Skyttetjänst art. 48442, 48444 and the siamese (double) figure. Hit-area boundary = anatomical vital area in the chest cavity | **Älgskyttemärket** and **practical big-game test** for the jägarexamen: 80 m, standing + running, series of 4 shots (standing → running after ~2 s, then the other direction) | Hit-area outline size and ring-centre position on the figure (T2, T3). Ring diameters are settled (T1 closed). See §17.6 and §33 |
| **Älgbana** (track) | Opening 22.75–23.25 m. Distance 79.5–80.5 m. Run time 4.3 ± 0.2 s | Running-moose lead preset (§12) | n/a |
| **Älgkalvfigur** | Natural size. Rings **11.2, 18.7, 26.2, 33.6 cm**. Skyttetjänst art. 48370 | Högviltdekalen alternative and elite class | Outline, ring position |
| **Vildsvinsfigur** | Photographic natural-size boar, *or* SvSF 50 m boar figure. Ringing 5¹/5/4/3 on the anatomical spot, or SvSF 10-ring | Competitions; 50 m viltmålsbana | Ring diameters and outline |
| **Vildsvinspasset** target | Yearling-size boar with anatomical hit area drawn | 50 m standing (2 stages) and 30 m running | All dimensions |
| **Stående råbock** | Photographic natural-size roe buck. Rings **10, 15, 20 cm** (SJF rules; to be confirmed on an owned target, §33). Swedish ringing (5/4/3 + hit area) or Danish ringing (10/9/8) | Competitions | Outline, ring position |
| **Löpande råbock** | Same ringing as standing roe buck (Swedish) | Running roe (SvSF: "löpande rådjur 50 m", Swedish roe-deer spot) | As above |
| **Rävfigur** (fox) | Rings **8, 12, 16 cm** | Competitions | Outline |
| **Tjäderfigur** (capercaillie) | Rings **6.5, 11, 15 cm** | Competitions | Outline |
| **Björnpasset** figures (bear side/front) | Side figure at 80 m and 40 m, side + front at 20 m; hit area defined on the approved board | Bear pass | All dimensions |
| **Red deer / fallow deer (kronhjort / dovhjort)** | **No official SJF/SvSF figure found** in the documents checked | Practice only | Generic vital-zone outline; **size to be chosen by Patrik** (§33) |

Note: the jägarexamen page says the practical test follows "each hunters' organisation's" moose test. The Jägarnas Riksförbund variant has not been reviewed.

### 17.3 Swedish sport shooting (Svenska Skyttesportförbundet, SvSF)

- **Viltmål (running target)** (SvSF discipline page):
  - 50 m: 10-ring moving **wild boar** figure in natural size on a 10 m track, .22 rifle with scope.
  - 80 m: 10-ring moving **moose** figure in natural size on a 23 m track, exposure time 4.3 s.
  
  Ring dimensions are **to measure from the official target** (ISSF/SvSF rules).
- **Enkel jägarkombination:** running moose at 80 m and running roe at 50 m (Swedish roe-deer spot), with 1–5 point rings.
- **Fältskytte (field rifle, Gevär 6.5)** (SvSF *Regelbok Nationellt skytte*, adopted 2026-03-02, §3.7): figure sizes (width × height, cm) and recommended distances (m) are published as follows.

| Figure | W×H cm | Distance m | Figure | W×H cm | Distance m |
| --- | --- | --- | --- | --- | --- |
| C 15D | 15×13.5 | 125–175 | 1/7 | 20×26 | 225–280 |
| S 10 D V / H | 10×25 / 25×10 | 150–200 / 125–175 | B 45 | 35×45 | 375–425 |
| S 15 D V / H | 15×37.5 / 37.5×15 | 175–225 / 160–220 | B 65 | 50×65 | 475–550 |
| Tunna D | 16×20 | 150–200 | B 100 | 55×100 | 520–610 |
| C 20 | 20×18 | 185–235 | 1/8 | 37×21 | 200–265 |
| C 25 | 25×22.5 | 220–270 | 1/6 rak | 45×26 | 250–315 |
| C 30 | 30×27 | 270–320 | 1/6 H V | 45×26 | 260–325 |
| C 35 | 35×31.5 | 320–370 | 1/4 rak | 49×33 | 300–375 |
| C 40 | 40×36 | 360–410 | 1/4 H V | 60×35 | 325–400 |
| C 50 | 50×45 | 430–480 | 1/3 | 52×48 | 375–475 |
| S 20 V / H | 20×50 / 50×20 | 275–325 / 225–275 | Tunna V / H | 40×50 / 50×40 | 410–490 / 400–460 |
| S 25 V / H | 25×62.5 / 62.5×25 | 340–390 / 280–330 | L1 / L2 / L3 | 19×45 / 30×70 / 43×100 | 210–260 / 325–385 / 425–500 |

  Bounding sizes come from the table. **Silhouette shapes are to be measured from the official figures.** The rulebook also publishes Korthåll (§3.6) and Kpist (§3.8) figure tables, which can be added later. These field figures are a good fit for a "hold at unknown distance" practice mode.
- **Precision (banskytte gevär 6.5):** the rulebook specifies the *international 300 m target*. Ring dimensions are to be taken from the ISSF rules.

### 17.4 US secondary set

- IPSC/USPSA and IDPA silhouettes, with dimensions to confirm against the current rulebooks.
- Common steel: round plates of 8″/10″/12″ and 20/30/40 cm, rectangular plates, and an IPSC-shaped steel silhouette.
- Generic round bullseyes and grid sight-in targets in cm and inch grids. The grid targets double as a scale reference for group photos (§24).

### 17.5 Custom targets

Users can import an SVG subset with a declared physical size. The SVG is sanitised: no scripts and no external references.

### 17.6 In-house drawing spec: SJF älgfigur (moose hunting-test figure)

**Reference image.** Patrik supplied a photo of the official target. It is stored at `/workspace/holdover-refs/sjf-algfigur-official-photo.jpg`, with a README in the same folder. **It is for personal reference only and must not be redistributed or committed to the public repo.** It is a photograph of copyrighted target artwork. Holdover draws its own figure; it does not trace or embed this image.

**What the photo shows:**
- A left-facing bull moose silhouette (head, antlers, neck and forequarters; the rear and legs are cropped in this photo).
- Four concentric rings on the chest/shoulder, labelled 5¹ (centre), 5, 4 and 3. The labels are diagonal, which per the SJF rules means the **larger ringing** (*större ringning*): both the 5¹ and 5 rings score 5, ring 4 scores 4, ring 3 scores 3, and the asymmetric hit area outside the rings scores 2.
- A thin rounded-triangle outline around the rings. This is the **asymmetric hit area** (*asymmetriskt träffområde*), described by the SJF rules as the boundary of the anatomically correct vital zone in the chest cavity (heart/lung).

**Ring values: official SJF preset.**

| Ring | Official SJF diameter (preset) | Older Swedish set (not a preset) |
| --- | --- | --- |
| 5¹ | 15 cm | 15 cm |
| 5 | 25 cm | 25 cm |
| 4 | 33 cm | 35 cm |
| 3 | 41.5 cm | 45 cm |

- The moose preset is the current official SJF set, confirmed by Patrik on 2026-10-17: 5¹ = 15 cm, 5 = 25 cm, 4 = 33 cm, 3 = 41.5 cm. These diameters are not labelled unverified or user-provided.
- The 15/25/35/45 cm set is an older Swedish set that is no longer used much. It is recorded as history only and is not a selectable preset.
- **Reading the values as radii is ruled out.** As radii, the inner ring would be 30 cm across, which contradicts both the SJF rules and the photo's proportions.
- **The reference photo agrees with the official SJF diameters.** Ring proportions were measured from pixel crossings on the horizontal and vertical lines through the centre. The rings are circular in the photo, so perspective distortion is negligible. The photo itself stays out of the repository.
- An unofficial Norwegian club page lists NJFF rings as 150/250/350/450 mm, the same numbers as the older Swedish set (source 18). That page does not change the Swedish preset.

**How the in-house figure reproduces the layout:**

1. **Units and origin.** Draw in true centimetres with the origin at the ring centre, x positive to the right (toward the moose's rear on a left-facing figure) and y positive up.
2. **Rings.** Concentric circles at the selected ring set's diameters. Score labels go on the diagonals (larger ringing) or on the horizontal and vertical axes (smaller ringing), to match the SJF convention.
3. **Hit-area outline.** A closed rounded-triangle path, built as three gently curved edges with large corner radii:
   - The **front (left) and lower edges** follow just outside ring 3.
   - The **upper edge** rises gently rearward from above the upper-left of ring 3.
   - It meets a **rounded apex up and to the right** (toward the back, behind the shoulder).
   - A **long, fairly straight rear edge** descends from that apex down and to the left, back to the bottom of ring 3.
   
   The zone is therefore asymmetric, extending mostly up and to the rear of the rings. Store it as a parametric path (control points in cm relative to the ring centre) so it can be corrected once measured.
4. **Placeholder geometry until measured** (rough estimate from the photo, scaled with SJF ring 3 = 41.5 cm, roughly ±3 cm; *not* for scoring):
   - bounding box about **60 cm wide × 47 cm high**,
   - ring centre about 26 cm from the left edge, 35 cm from the rear apex, 24 cm below the top and 22 cm above the bottom.
   
   Display it with a "placeholder outline" badge.
5. **Silhouette.** A simplified, generic moose outline (flat fill, no photographic texture or fur detail) positioned so the rings sit on the chest just behind the foreleg, as in the photo. The silhouette's overall size and the ring centre's position relative to it are **to be measured** (§33). Until then the silhouette is decorative only; scoring uses rings and hit area only.
6. **Direction variants.** Left-going (as photographed) and a mirrored right-going figure. A siamese (double) layout places both on one board, with spacing to be measured.
7. **Scoring zones** carry their point values (5¹ = 5, 5, 4, 3, hit area = 2 for larger ringing), so the impact view (§13) and group analysis (§24) can score predicted and measured hits.
8. **Provenance** stored with the target: "In-house drawing from the current official SJF ring diameters (15/25/33/41.5 cm); placeholder outline and silhouette (unmeasured, T2/T3)".

---

## 18. Weather and location

- **Manual entry** is the default and always available.
- **Open-Meteo free API.** Holdover is personal and non-commercial, so the free tier applies. Terms: CC-BY 4.0 attribution (shown on the About screen) and the fair-use limits (10,000 calls/day). No API key is needed.
  - Endpoint: `/v1/forecast?current=temperature_2m,relative_humidity_2m,surface_pressure,pressure_msl,wind_speed_10m,wind_direction_10m,wind_gusts_10m`.
  - Field mapping: **`surface_pressure` → station pressure**; `pressure_msl` is labelled sea-level; the response `elevation` (DEM-based) is the altitude.
  - Model wind at 10 m is **context, not a wind call**. It is never auto-filled into the hold without the user accepting it.
  - Values are labelled Fetched (valid time, retrieval time, grid-cell coordinates), Measured or Manual. A stale badge appears after a configurable age.
  - Request at most a few calls per refresh. Results are cached and there is no background polling.
- **Location:** a saved place is the default. Precise coordinates are excluded from exports by default. In Phase 2 the phone can supply GPS, barometer, compass and inclinometer readings (§23).
- Not planned: Kestrel or other device imports (settled: no imports from other apps or devices).

---

## 19. Bullet and load library: user-entered only (settled)

- **No manufacturer data is bundled.** The only bundled ballistic data is the standard drag-function tables (§7.3).
- Users enter bullets and loads themselves, with a free-text source note per value (e.g. "Lapua website, 2026-10-12" or "box label"). Provenance tags still distinguish user-entered, measured and fitted values.
- Data-entry helpers:
  - unit-flexible fields (§22),
  - "duplicate and edit",
  - sanity ranges, e.g. G7 BC typically 0.1–0.5 and G1 BC typically 0.1–1.0 lb/in² (warning only),
  - G1 vs G7 mismatch hints (e.g. a value entered as G7 that looks like a G1 BC).
- **Own import/export:** the library exports and imports as Holdover JSON (versioned schema), with optional CSV templates in Holdover's own column format. Import goes through preview, duplicate resolution, a transactional commit and rollback.

---

## 20. Load log: reference only (safety boundary)

- Users can attach or link manufacturer load-data documents (e.g. the Hodgdon Reloading Data Center or printed manuals) in Documents, with edition and date recorded.
- **Personal load log:** a record of the user's own loads (components, lots, charge as recorded, COAL, notes) linked to chrono sessions. The measured MV feeds the solver.
- **Not provided:** recipe generation, charge interpolation or extrapolation, optimisation, pressure prediction, or ladder/node suggestions. Overpressure can destroy firearms and injure people, and safe load development depends on published, pressure-tested data that Holdover cannot verify. This is stated once in the app.

---

## 21. Chronograph statistics

- Manual entry or paste of shot strings. Per session: count, mean, SD, ES, min/max and temperature. Outliers are flagged, never removed automatically.
- "Set as load MV" with the session temperature. With sessions at several temperatures, a powder-temperature coefficient is fitted by linear regression and shown with R² and n.
- SD is converted to a predicted vertical dispersion at range (shown in the impact view).
- **No device or app imports** (Garmin Xero, LabRadar etc.), per the settled decision. Plain paste of a column of numbers works for any source.

---

## 22. Units and numeric entry (settled: onboarding + switch everywhere)

The rules from v0.1 and v0.2 stand: normalized storage plus original input, dimension checks, decimal comma, offset-correct temperature conversion, and no false precision. Settled additions:

1. **Onboarding** asks for the default preset: Metric/Nordic, US or Custom.
2. **Every page has the unit switch in its header** (§5.1). It converts all displayed values automatically.
3. **Every quantity input accepts either system:**
   - **typed suffixes** (`m`, `yd`, `ft`, `cm`, `in`, `mm`, `gr`, `g`, `kg`, `lb`, `oz`, `m/s`, `fps`, `ft/s`, `km/h`, `mph`, `kn`, `°C`, `°F`, `K`, `hPa`, `mbar`, `inHg`, `mmHg`, `psi`, `mrad`, `mil`, `MOA`, `IPHY`, `cm@100m`, `J`, `ft·lbf`/`ftlb`, twist `1:8in`, `1:203mm`),
   - a **per-field unit toggle chip**,
   - a bare number interpreted in the field's displayed unit.
4. **Switching never changes stored values.** Property test: random values × random preset switches × save/reload give identical normalized values.
5. **Grains vs grams:** `gr` and `g` must never be confused. The unit is always shown, and an ambiguous `gr.` with locale-dependent meaning is rejected and prompts for clarification.
6. Pressure fields carry a **kind** (station or sea-level) alongside the unit.
7. Output units (angular corrections, energy etc.) are independent of input units.
8. **Presets:**
   - US (yd, MOA, °F, inHg, mph, gr, ft·lbf, in),
   - Metric/Nordic (m, mrad, °C, hPa, m/s, g, J, cm),
   - Custom (any mix, e.g. m + MOA scope + grains).

---

## 23. Phone companion and sync (Phase 2)

### 23.1 Why a phone app, and why Tauri 2

Strelok is primarily used on a phone at the range or in the stand. Tauri 2 targets **Android as well as desktop**, so the same Rust `holdover-core` (solver, units, group analysis) and most of the TypeScript UI components can be reused. This is now the main argument for Tauri 2 over Qt Quick (§26).

**Platform: Android only (settled).** The companion is built for Android and installed by sideloading. **iOS is out of scope for now.** There is no Mac build host, no Apple developer account and no TestFlight. Tauri 2 can also target iOS, so this could be revisited later without changing the core.

### 23.1a Android build and install

- Build with Tauri's Android tooling on the Omarchy machine (Android SDK + NDK and the Rust Android targets, per the Tauri prerequisites).
- Sign release APKs with Patrik's own keystore. The keystore and its passwords stay out of the repo and are kept backed up. Losing the key means the next APK cannot install as an update over the old one.
- Install by `adb install` over USB or by opening the APK file on the phone (with "install unknown apps" allowed for the file manager). No Play Store or F-Droid publication.
- Updates: install a newer APK with a higher `versionCode` over the old one. Data stays in the app's private storage, and bundle export (§23.3) is the backup.
- Target a reasonably recent Android version only (exact minimum SDK to be decided in the Phase 2 spike, based on Patrik's phone).

### 23.2 Phone scope

- **Solve** (same hold panel), range cards, and read/edit of profiles.
- **Field capture:**
  - quick atmosphere entry,
  - chrono entry,
  - observed dope for truing,
  - **shot-group photo analysis** (§24).
- **Phone sensors** (opt-in), each value labelled as *Measured (phone)*:
  - barometer (if the phone has one) → **station pressure**,
  - GPS → location and altitude,
  - compass → shooting azimuth for Coriolis,
  - accelerometer → inclination and cant.
- Offline by default. No account. No cloud.

### 23.3 Sync model (local-first, no mandatory cloud)

1. **Stage 1: export bundle (first).**
   - A `.holdover` file (zip containing versioned JSON, attachments and a SHA-256 manifest) can be exported from either device.
   - Move it any way you like: USB, LocalSend, KDE Connect, Syncthing, or a cable.
   - Importing a bundle shows a **merge preview** (new / changed / conflicting records) before committing.
2. **Stage 2: LAN sync (later).**
   - The desktop offers an opt-in "Pair phone" QR code containing a one-time key and its LAN address.
   - The devices exchange long-term keys and then sync over the local network (mDNS discovery, mutual TLS with pinned keys) only while both apps are open and the user starts a sync.
   - No relay, no port forwarding, no internet.
3. **Merge rules (both stages):**
   - Records are keyed by UUID, with per-field `updated_at` and device revision.
   - Last-writer-wins per field for simple fields. Concurrent edits to the same field are shown as a conflict to resolve. Deletes are tombstones.
   - Truing records, groups and chrono sessions are append-only, so they rarely conflict.
   - Photos are synced as content-addressed attachments.
4. **Schema discipline:** both apps embed the same schema version. An older app refuses a newer bundle with a clear message and never imports partially.

Phone-specific risks to verify in the Phase 2 spike:
- camera access from Tauri mobile (webview capture or plugin),
- sensor access via plugins,
- background limits,
- APK signing and sideload install flow on Patrik's phone (Android only; iOS out of scope).

---

## 24. Shot-group photo analysis (Phase 2)

**Decision: available on both phone and desktop.** The analysis is a pure function in `holdover-core`, so the desktop gets it through photo import at almost no extra cost. The desktop's large screen is also better for precise manual hole correction. Phone capture comes in M9, desktop import in M8.

### 24.1 Workflow

1. **Photo:** capture on the phone, or import a JPEG/PNG/HEIC on the desktop. EXIF location is stripped on import unless kept explicitly.
2. **Scale and perspective:**
   - **Preferred: four-point reference.** Mark four corners of a known rectangle (target grid square, sheet corners, or a known Holdover-printed target with corner marks). A **homography** gives a rectified, metric image and corrects skew and tilt.
   - **Fallback: two-point scale** (known distance on a ruler or grid) with an "orthogonal photo required" warning. The app estimates residual skew from grid lines when available and refuses to report sub-mm precision.
   - Holdover-printed targets (§17.4) include corner fiducials at known spacing for one-tap calibration.
3. **Point of aim:** tap the aim point, or use the target centre for known targets.
4. **Holes:**
   - Auto-detect (threshold + blob/circle detection on the rectified image, with the expected hole diameter set from the bullet diameter).
   - Manual add, move and delete. Overlapping holes can be split by count.
   - Shot count must match the user-entered count before stats are saved.
   - Hole positions are **centres**. Centre-to-centre measurement needs no bullet-diameter subtraction. If the user measures outside edges instead, the app subtracts one bullet diameter and labels the result.
5. **Distance and context:** shot distance (with unit), linked profile/load, atmosphere snapshot, date and optional chrono session.

### 24.2 Statistics

| Metric | Definition |
| --- | --- |
| Extreme spread (ES) | Max centre-to-centre distance between any two holes |
| Mean radius (MR) | Mean distance of holes from the group centre |
| Group centre offset | Vector from POA to group centre: linear, **and mrad/MOA at shot distance** (`atan(offset/distance)`), as UP/DOWN and LEFT/RIGHT |
| Windage / elevation spread | Horizontal and vertical extreme spread, plus SD of x and y |
| Radial SD | √(SD_x² + SD_y²) |
| CEP / R50 | Shown only for n ≥ 5, estimated from a circular-normal model (`CEP ≈ 1.1774·σ` with σ pooled from SD_x and SD_y) with a "small-sample estimate" label. The empirical median radius is also shown |
| Shot count, distance, ES & MR in mrad/MOA | Always shown |

- An **uncertainty note** is shown for small n. Statistics are never over-precise: precision is limited by the scale-reference accuracy.

### 24.3 Links

- Each group is saved in Equipment → Groups and linked to profile, load (and so the load log), atmosphere and chrono session.
- **Truing:** the group-centre offset at a known distance can be sent to truing as an observation (§14).
- **Charts:** measured groups can be overlaid on predicted impacts (§13).
- Photo and annotations are kept. The original photo is never modified; the rectification transform is stored separately.

---

## 25. Omarchy 4 "Quattro" design and theme integration (settled: v4+)

### 25.1 What was verified (2026-10-16)

- **Name and version:** Omarchy 4.0.0 is officially **"The Quattro Release"**, released 2026-08-14. The latest release at check time was **v4.0.4** (2026-09-15). The owner's "Qutro" refers to Quattro.
- **v4 changes relevant here:**
  - The whole desktop shell was rewritten in Quickshell (Waybar, Walker, Mako and others removed).
  - Omarchy internals moved from a git checkout to system packages under `/usr/share/omarchy`.
  - Hyprland config is now Lua.
  - Theme colouring expanded "from base 8 to 24", with a semantic colour system and normalized colour names.
  - There is a global text-size knob (`omarchy display text size`) that also sets GTK's text-scaling factor.
- **The theming mechanism still matches the plan,** with v4 details read from the v4.0.4 source:
  - Each theme has a `colors.toml`. v4 keys include `mode` (`"dark"`/`"light"`), `accent`, `selection`, `muted`, `background`, `dark_background`, `darker_background`, `lighter_background`, `foreground`, `dark_foreground`, `light_foreground`, `bright_foreground`, `red`, `yellow`, `orange`, `green`, `cyan`, `blue`, `magenta`, `brown` and `bright_*` variants.
  - User templates go in **`~/.config/omarchy/themed/*.tpl`** and take priority over built-ins.
  - Placeholders are `{{ key }}` plus `_strip` and `_rgb` modifiers. `color0`–`color15` aliases still exist, along with `{{ mix a b 30% }}`-style mixing.
  - On a theme switch, `omarchy-theme-set-templates` renders templates into `~/.local/state/omarchy/current/next-theme/`. `omarchy-theme-set` then **deletes `current/theme` and renames `next-theme` to `current/theme`**.
  - A **`theme-set` hook** runs scripts in `~/.config/omarchy/hooks/theme-set.d/` with the new theme name.

### 25.2 Adapter design for v4

- **Template:** `holdover.json.tpl` (installed to `~/.config/omarchy/themed/` only on user request, from `/usr/share/holdover/`). It emits all semantic keys plus `mode`, and a few mixes (e.g. `{{ mix background foreground 8% }}` for surface tints). Holdover maps these to its tokens: surface, text, secondary text, border, accent, focus, warning, error, a 6-colour chart palette and reticle/target ink, all with contrast checks.
- **Generated file location:** `~/.local/state/omarchy/current/theme/holdover.json`.
- **Change detection:** because the whole `current/theme` directory is replaced, watch **the parent `~/.local/state/omarchy/current/`** for the rename, then re-read. Watching the old file inode would miss the change.
- **Optional hook:** a tiny `theme-set.d/holdover` script that pokes the running app (e.g. touches a file in `$XDG_RUNTIME_DIR`) as a belt-and-braces signal.
- **Fallback reader:** if the template isn't installed, parse `~/.local/state/omarchy/current/theme/colors.toml` directly (colour values only; never executed).
- **Last resort:** the last valid palette, or a bundled accessible palette.
- **Light mode:** follow `mode` from the generated file or `colors.toml`.
- **Text size:** respect the user's Omarchy/GTK text scaling. The M0 spike checks whether WebKitGTK picks up GTK's text-scaling factor automatically, and otherwise reads it.
- **Never modify** Omarchy-owned files under `/usr/share/omarchy` or the Hyprland Lua config.
- **Visual language:** quiet surfaces, one accent, tabular numerals, no faux instruments. Light themes are first-class. Field mode overrides with maximum contrast.
- **Desktop behaviour:** `.desktop` entry, so it shows in the unified Omarchy menu (`SUPER+SPACE`, apps-only `SUPER+ALT+SPACE`). Handles tiling, resizing, fractional scaling and multi-monitor. No global shortcuts.
- **Compatibility:** support Omarchy ≥ 4.0. Omarchy 3.x is not targeted; if detected, the app falls back to the bundled palette without error.

---

## 26. Technical direction

### 26.1 Options

| Option | For | Against |
| --- | --- | --- |
| **Rust core + Tauri 2** (WebKitGTK) | Same core and UI on desktop **and Android** (Phase 2), so one codebase. SVG/canvas suits reticles, targets, charts, print and group-photo annotation. CSS variables suit theme tokens. Fast iteration with coding agents | Must prove WebKitGTK quality on Hyprland (scaling, GPU, text scaling) |
| Qt 6 Quick/QML | Mature and fast. Omarchy 4's own shell is Qt Quick (Quickshell), so the platform is well exercised | Mobile is possible but a separate, heavier path. C++/cxx-qt friction |
| GTK4/libadwaita | Native GNOME feel | Limited custom theming; no mobile path |
| Slint | Light, Rust-native | Smaller chart/print ecosystem |

### 26.2 Recommendation (unchanged, now stronger)

1. **`holdover-core`** (Rust): solver, atmosphere, units, statistics and group analysis. No I/O. Plus **`holdover-cli`**.
2. **UI: Tauri 2 + TypeScript** (Svelte or React), gated by the M0 spike:
   - recalculation while typing feels instant,
   - rendering is crisp at 1.0/1.5/2.0 scaling,
   - range-card print/PDF is correct,
   - live theme switch works via the v4 template,
   - Omarchy text size is respected.
   
   **Fallback:** Qt 6 Quick on the same core.
3. **Storage:** SQLite with WAL and versioned migrations, with an automatic backup before each migration. Data lives at `$XDG_DATA_HOME/holdover` (config at `$XDG_CONFIG_HOME/holdover`), never inside Omarchy directories. Attachments are content-addressed. UUID and revision fields are present from the first schema (for sync).
4. **Packaging (personal):**
   - a `PKGBUILD` in the repo, built and installed with `makepkg -si`;
   - publishing to the AUR is optional (Omarchy installs AUR packages via *Install > AUR*);
   - ships a `.desktop` file, icons and `/usr/share/holdover/holdover.json.tpl`;
   - licence: **MIT** (`LICENSE` in the repo root from M0). Third-party licences (Tauri, crates, npm packages; the CC-BY Open-Meteo attribution) are listed on the About screen. py-ballisticcalc (LGPL) is used only as a test oracle and is not distributed.
5. **Strings:** all UI text goes through an i18n layer (e.g. Fluent or i18next keys) from day one. English is the only shipped locale; no translation work is planned.

### 26.3 Module boundaries

`holdover-core` → `domain` (profiles, zero, truing, groups, provenance) → `storage` → `sync` (bundle, later LAN) → `providers` (Open-Meteo) → `app-shell` (Tauri commands) → `ui`. The UI never does physics or statistics.

---

## 27. Validation and testing

### 27.1 References

- JBM's online calculators are **retired** (per its About page), so they cannot be used for live comparison. The reference set is:
  - analytic checks (vacuum parabola, constant-Cd),
  - **py-ballisticcalc** (LGPL-3.0, 3DOF + spin drift) as a CI test oracle only (not linked into Holdover),
  - published maker tables and Litz worked examples with documented assumptions,
  - any archived JBM outputs,
  - the owner's live-fire dope.
- **Group analysis references:** synthetic images with known hole positions under known homographies, and printed calibration targets photographed at angles.

### 27.2 Tolerances

| Comparison | Quantity | Target |
| --- | --- | --- |
| vs independent 3DOF, identical inputs, supersonic to 1000 yd | drop, windage | ≤ 0.05 mrad |
| same, incl. transonic, to 1000 yd | drop, windage | ≤ 0.1 mrad |
| same | velocity, TOF | ≤ 0.5% |
| vs maker tables | drop | ≤ 0.2 mrad or explained |
| Integrator convergence | drop at max range | Δ ≤ 0.01 mrad |
| Zero solve | LOS crossing | ≤ 0.1 mm |
| Group analysis, synthetic, 4-point homography | hole position | ≤ 0.5 mm at A4 scale |
| Group analysis, synthetic | ES, MR, offset | ≤ 1% |

### 27.3 Test types

- **Unit tests:** atmosphere, Miller SG, unit parsing and conversion including every suffix in §22, click rounding, homography and statistics.
- **Property tests:**
  - drop monotonic in range,
  - more MV gives less drop; more BC gives less drop and drift,
  - mirrored wind gives mirrored drift,
  - unit switch → save → reload is lossless,
  - sync merge is commutative for non-conflicting edits.
- **Golden snapshots** of CLI outputs.
- **Truing recovery** on synthetic data.
- **UI checks:**
  - keyboard-only solve,
  - unit switch on every page,
  - per-field toggles,
  - field mode,
  - hold readable at 1 m,
  - card printed at true scale (a 10 cm target prints at 10 cm),
  - live theme switch on Omarchy 4.

### 27.4 In-app disclaimer

> "All values are predictions from a mathematical model and your inputs. Confirm with live fire before relying on them. You are responsible for safe and lawful shooting."

Energy figures are informational. Swedish and Washington hunting rules (e.g. Swedish ammunition classes, which the SJF rules reference) are not evaluated by the app.

---

## 28. Privacy and security

- No account, no telemetry, no cloud.
- Serial numbers, precise locations and photo EXIF are optional and excluded from exports and bundles by default.
- Imported bundles, SVGs, photos and theme files are untrusted data and are never executed.
- LAN sync (Phase 2) is opt-in, pairing-based and encrypted, and runs only on user action.
- Network access happens only for Open-Meteo, when enabled.
- Backups and restores are validated, with a preview before applying.

---

## 29. MVP definition (refreshed)

The MVP is what Patrik would use instead of a phone app at the bench and in the field, on Omarchy 4.

1. Onboarding unit preset. A **unit switch on every page** and per-field toggles with typed suffixes, all lossless.
2. Profiles: rifle, scope (click value, FFP/SFP), user-entered bullet (G1/G7 + multi-BC), load (MV + temperature).
3. Atmosphere with an explicit station/sea-level kind, humidity, altitude and DA.
4. Zero with a separate zero atmosphere, then the current-conditions solve.
5. **Solve screen:** mrad/MOA/clicks, wind by clock, velocity/energy/TOF/Mach, with incline and spin drift on and Coriolis, cant and aerodynamic jump available but off, plus SG.
6. Range table plus a **printable dope card** with wind brackets.
7. Drop and wind charts with at least 2 overlays.
8. **Swedish quick targets:** älgfigur ring set (official SJF preset 15/25/33/41.5 cm; the older 15/25/35/45 cm set is not a preset) and roe buck rings (10/15/20 cm, T4 still open) as PBR presets and impact-view targets, using generic or placeholder outlines until measured (§33).
9. SQLite storage, Holdover JSON backup/restore and library export/import.
10. Omarchy 4 theme following via the template, with the `colors.toml` fallback.
11. `holdover-cli` plus a CI validation suite meeting §27.2.
12. English UI with translation-ready strings.

Not in the MVP but in v1.0: PBR UI, lead (with Swedish running presets), truing, recoil, reticles plus editor, the full target library, weather and chrono statistics.

---

## 30. Milestones

The phases are ordered by dependency. Effort is estimated after M0.

| Milestone | Deliverables | Exit criteria |
| --- | --- | --- |
| **M0: Spike and references** | **`LICENSE` (MIT, `Copyright (c) 2026 Patrik Hallgren`) committed to github.com/PatrikHallgren/holdover**, README stub, `.gitignore` that excludes `holdover-refs/` and any keystores; initial repo scaffold (workspace: `holdover-core`, `holdover-cli`, app), Tauri spike on Omarchy 4, v4 template proof, reference vectors, JSON schema draft (with sync fields) | LICENSE present on `main`; §26.2 spike criteria pass; ≥20 documented reference vectors; live theme switch works |
| **M1: Solver core + CLI** | 3DOF, drag tables, multi-BC, custom Cd, integrators, atmosphere, zero, effects, SG, units library | §27.2 solver tolerances in CI; full solve to 2000 m < 20 ms |
| **M2: Solve MVP** | Storage and migrations, onboarding, unit switch on every page, Solve UI, forms, range table, charts, backup | MVP items 1–12 pass; unit round-trip test green |
| **M3: Field toolkit** | Dope card print, PBR, lead (Swedish presets), overlays and impact view, truing, recoil, chrono statistics | Card true-scale print verified; truing recovers synthetic parameters within 1%; PBR matches brute-force search |
| **M4: Reticles and targets** | Reticle format, editor, generic set, FFP/SFP rendering; Swedish target library (§17.2–17.3), then US set (§17.4) | SFP verified at 3 magnifications; target angular size within 1 px; every §33 item either resolved or visibly flagged in the app (placeholder/unverified badges) |
| **M5: Weather and library files** | Open-Meteo provider with labelling; library JSON/CSV import/export in Holdover's own format | Offline/timeout/stale handled; import rollback tested |
| **M6: Equipment depth → v1.0** | Compare, documents, load log, archive/history; accessibility pass; PKGBUILD; About/licences screen; user guide | Fresh install on current Omarchy 4.x; v1.0 tagged |
| **Phase 2: M7, sync bundle** | `.holdover` bundle export/import with merge preview (desktop to desktop first) | Round-trip lossless; conflict UI tested |
| **Phase 2: M8, group analysis (desktop)** | `group_stats` in core; desktop photo import, 4-point rectification, auto-detect + manual edit, statistics, links to truing and load log | Synthetic-image tolerances (§27.2) met; real-photo check against calliper measurement |
| **Phase 2: M9, Android companion** | Tauri Android app (sideloaded, self-signed APK): Solve, cards, profiles, field capture, group photo capture, opt-in sensors; bundle sync with desktop. iOS out of scope | Signed APK installs and updates in place on Patrik's Android phone; bundle round-trip with desktop; group results identical to desktop for the same photo |
| **Phase 2: M10, LAN sync** | QR pairing, mDNS, mutual TLS, field-level merge | Pair, sync and conflict flows tested; no internet needed |
| **Later / not planned** | Swedish UI (low priority); iOS companion (out of scope for now); altitude-varying atmosphere; Monte-Carlo hit probability. *Not planned:* imports from other apps/devices (Strelok, chronographs, Kestrel), bundled maker data or reticles | — |

---

## 31. Decisions: all settled (2026-10-16, updated in v0.4)

| # | Decision | Answer | Where it's applied |
| --- | --- | --- | --- |
| 1 | Distribution | **Personal, non-commercial.** Repo `PatrikHallgren/holdover`; name is "Holdover". Open-Meteo free tier applies | Header, §1, §18, §26.2 |
| 2 | Languages | **English first;** strings translation-ready only. Swedish maybe later, low priority | §26.2, §30 |
| 3 | Phone companion | **Yes, Phase 2,** with shot-group photo analysis and local-first sync | §23, §24, §30 |
| 4 | Advanced effects in v1 | Incline + spin drift **on**; Coriolis, cant, aerodynamic jump **available, off** | §5.2, §7.7, §29 |
| 5 | Units | **Onboarding preset + unit switch on every page + any-unit input per field;** stored values never change | §5.1–5.5, §22, §29 |
| 6 | Bullet library | **User-entered only;** no maker data bundled; own import/export kept | §7.3, §19 |
| 7 | Reticles | **Generic set + editor** | §16 |
| 8 | Omarchy | **v4 "Quattro" and later,** with theme support (verified mechanism) | §25 |
| 9 | Migration/imports | **None at launch;** only Holdover's own backup/restore/export. Strelok/device imports dropped | §1, §18, §19, §21, §30 |
| 10 | Targets | **Swedish first** (SJF hunting, SvSF viltmål/fält/precision), **US secondary** (IPSC/USPSA, IDPA, steel) | §17, §11, §12 |
| 11 | Phone platform | **Android only,** sideloaded self-signed APK; **iOS out of scope for now** | §1, §23, §30 |
| 12 | Licence | **MIT,** `Copyright (c) 2026 Patrik Hallgren`; `LICENSE` added in M0 | Header, §26.2, §30 |
| 13 | Moose preset rings | **Official SJF 15/25/33/41.5 cm** (5¹/5/4/3) as the moose preset. The older Swedish 15/25/35/45 cm set is not a preset. T1 closed 2026-10-17 | §17.2, §17.6, §29, §33 |

---

## 32. Sources

Checked on 2026-10-16:

1. [Omarchy manual: Making your own theme](https://omarchy.org/manual/making-your-own-theme/). Covers `colors.toml`, `~/.config/omarchy/themed/*.tpl`, placeholders and modifiers, `mode = "light"`.
2. [Omarchy manual: Other Packages](https://omarchy.org/manual/other-packages/). Repo and AUR installs via the Omarchy menu.
3. [Omarchy v4.0.0 release, "The Quattro Release"](https://github.com/basecamp/omarchy/releases/tag/v4.0.0) (2026-08-14) and the GitHub releases API (latest v4.0.4, 2026-09-15). Covers the Quickshell shell, packaged internals, Lua Hyprland config, 24-colour semantic theme system and text-size knob.
4. Omarchy v4.0.4 source: [`bin/omarchy-theme-set-templates`](https://github.com/basecamp/omarchy/blob/v4.0.4/bin/omarchy-theme-set-templates), [`bin/omarchy-theme-set`](https://github.com/basecamp/omarchy/blob/v4.0.4/bin/omarchy-theme-set), [`bin/omarchy-hook`](https://github.com/basecamp/omarchy/blob/v4.0.4/bin/omarchy-hook), [`config/omarchy/themed/alacritty.toml.tpl.sample`](https://github.com/basecamp/omarchy/blob/v4.0.4/config/omarchy/themed/alacritty.toml.tpl.sample), [`themes/tokyo-night/colors.toml`](https://github.com/basecamp/omarchy/blob/v4.0.4/themes/tokyo-night/colors.toml). Covers template rendering and output path, directory swap, hooks and v4 colour keys.
5. [Open-Meteo API docs](https://open-meteo.com/en/docs) and [terms](https://open-meteo.com/en/terms). Covers `current=` variables, `surface_pressure` vs `pressure_msl`, `elevation`, and non-commercial free use under CC-BY 4.0 with limits.
6. [JBM Ballistics: About](https://www.jbmballistics.com/about.html) (calculators retired) and [Downloads](https://www.jbmballistics.com/downloads.html) (G-function Cd tables, "free for public use").
7. [py-ballisticcalc](https://github.com/o-murphy/py-ballisticcalc). LGPL-3.0, 3DOF + spin drift.
8. [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/). `webkit2gtk-4.1` on Arch; Android setup (SDK, NDK, `JAVA_HOME`, Rust Android targets).
9. [strelokpro.com](https://www.strelokpro.com/). Product line (mobile; desktop for macOS/Windows).
10. [Hodgdon Reloading Data Center](https://hodgdonreloading.com/rldc/). Example of reference load data to link.
11. GitHub API, [`PatrikHallgren/holdover`](https://github.com/PatrikHallgren/holdover). Public, empty, no licence (read-only check, v0.3; not re-checked or modified for v0.4).
12. Svenska Jägareförbundet, [*Tävlingsreglemente KM, LM, SM 2025–2028*](https://jagareforbundet.se/contentassets/f162ad961efd4990a1faa675b57d2838/tavlingsreglemente-km-lm-sm-2025-28.pdf), §3.1–3.2. Target ringing and diameters (moose, moose calf, boar, roe buck, fox, capercaillie), älgbana specifications.
13. Svenska Jägareförbundet, [*Märkesreglemente 2025–2028*](https://jagareforbundet.se/contentassets/f162ad961efd4990a1faa675b57d2838/markesreglemente_2025-28.pdf). Älgskyttemärket, högviltdekalen, björnpasset, vildsvinspasset; approved Skyttetjänst article numbers.
14. Svenska Jägareförbundet, [Älgskytteprov](https://jagareforbundet.se/jakt/jakt-i-sverige/algskytteprov/) and [Praktiskt högviltprov/kulgevär](https://jagareforbundet.se/utbildning/jagarexamen/jagarexamensprov/praktiskt-hogviltprov-kulgevar/). 80 m, standing + running moose, series structure.
15. Svenska Skyttesportförbundet, [Viltmål 50 m och 80 m](https://www.skyttesport.se/vaara-idrottsgrenar/viltmaal/vaara-grenar-och-discipliner/viltmaal-50-m-och-80-m/). Running boar on a 10 m track, running moose on a 23 m track with 4.3 s exposure.
16. Svenska Skyttesportförbundet, [Bilaga 4, Enkel jägarkombination](https://skyttesport.se/media/cejn0wox/bilaga_4_enkel_jagarkombination.pdf). Running moose at 80 m, running roe at 50 m (Swedish roe-deer spot).
17. Svenska Skyttesportförbundet, [*Regelbok Nationellt skytte*, adopted 2026-03-02](https://www.skyttesport.se/media/duyfs1dj/svsf-regelbok-nationellt-skytte-2026-03-02.pdf), §3.1–3.8. Fältskyttemål Gevär 6.5 sizes and distances; precision target = international 300 m target.

18. [Rørøs DFS (Norwegian shooting club) table *"Maksavstand innertreff jaktfeltfigurer"*](https://mittdfs.no/globalassets/associationfiles/roros/andre-faste-dokumenter/skjemasamling/maksavstand.htm). Lists NJFF "løpende elg og fasthold" rings: inner 5 = 150 mm, outer 5 = 250 mm, 4 = 350 mm, 3 = 450 mm. This is an unofficial club page. Those millimetre values match the older Swedish 15/25/35/45 cm set, which is not the current Swedish preset.
19. User-supplied photo of the official SJF älgfigur (personal reference only, not redistributed). Kept at `/workspace/holdover-refs/sjf-algfigur-official-photo.jpg`. Ring proportions were measured from it (§17.6).

**Not verified, to confirm against primary sources before implementation:**

- Litz spin-drift and aerodynamic-jump coefficients.
- Miller constants.
- The SAAMI ejecta convention.
- ISSF running-target and 300 m ring dimensions, and the viltmål fast-run speed.
- Exact Swedish figure outlines and ring placements (to measure from official targets).
- The Jägarnas Riksförbund moose-test variant.
- Whether any official red/fallow-deer figures exist.
- Current IPSC/USPSA/IDPA dimensions.
- Tauri mobile camera and sensor plugin coverage.

The owner's installed Omarchy system was not inspected.

---

## 33. 📋 To collect later (open items)

These are physical measurements Patrik doesn't have to hand right now. They **block final target geometry in M4**, but nothing earlier. T1 (moose ring diameters) is closed. Until each remaining item is done, the app shows the matching "placeholder" badge. The ring diameters themselves are not unverified.

| # | Item | How | Unblocks |
| --- | --- | --- | --- |
| T1 | **Closed 2026-10-17.** Moose figure rings are the current official SJF diameters: 5¹ = 15 cm, 5 = 25 cm, 4 = 33 cm, 3 = 41.5 cm. The 15/25/35/45 cm set is an older Swedish set no longer used much, and is not a preset | Confirmed by Patrik | Moose preset values; no unverified badge on the rings |
| T2 | **Moose hit-area outline** (rounded triangle) | Photograph squarely with a ruler, or measure: total width and height, distances from the ring centre to the left edge, rear apex, top and bottom, plus 6–8 points along the edge | Replaces the placeholder outline (§17.6) |
| T3 | **Moose silhouette placement:** overall figure width and height, ring-centre position relative to the figure (e.g. from nose tip and from ground/hoof line), and siamese spacing if owned | Tape measure on the full target | True-scale silhouette in the Reticle and impact views |
| T4 | **Roe buck target** (standing/running): confirm rings 10/15/20 cm, hit-area outline, ring position, figure size | Ruler photo or measurements, as for T2 and T3 | Roe preset and figure |
| T5 | **Other owned official targets** (e.g. moose calf, wild boar / vildsvinspasset, fox, bear pass, SvSF field figures) | Ruler photo or measurements, as owned | Each target's outline and zones |
| T6 | **Red deer and fallow deer vital-zone sizes:** no official SJF/SvSF figure was found, so Patrik chooses the diameters to use (and an outline style) | Patrik's choice, e.g. based on hunting guidance he trusts | PBR presets and practice targets for kronhjort and dovhjort |

**Photo tips for T2–T5:**
- Shoot straight on, with the target flat and evenly lit.
- Lay the ruler in the same plane as the target.
- Include all four target corners in the frame, so the Phase 2 group-analysis rectification (§24) can also be tested on these photos.
- Keep the photos in `/workspace/holdover-refs/` (personal reference only; not for the public repo).

