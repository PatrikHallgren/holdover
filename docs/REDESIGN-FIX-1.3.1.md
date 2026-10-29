# Redesign fix 1.3.1

Version 1.3.1 is in the crate, the window title, `packaging/PKGBUILD`, and About (About reads the crate version). No tag and no release. Solver physics and target dimensions are unchanged.

## Done

- **PR1-1.** `figureParts` no longer treats `+-−` as a range, so holds at 10 and above keep the first digit. `app/ui/figure-parts.test.js` covers 17, 54, 12.35 MOA, 10.2 mrad, negatives, clicks, and every other UI character class.
- **PR1-2.** The big figure is 2 dp (1 dp for cm/100 m, integer clicks). The small exact value stays 4 dp. The hero scales with the readout width and never goes under 48 px.
- **PR1-3.** Below 1400 px the hero context wraps, so Copy hold stays on screen.
- **PR1-4.** An invalid quantity hint uses the shown unit with a space (`4 m/s`, `°C`).
- **PR1-5.** From 1100 px to 1399 px the Solve cards are three columns, so the notes sit on a 1280×800 screen.
- **PR1-6.** The Air grid is one column below 760 px and again from 1100 px to 1399 px, so pressure is not clipped in the three-column card.
- **PR1-7.** R / W / D / A call `preventDefault` before focus, and the focused field is selected.
- **PR1-8.** The current Nearby row uses primary text, weight 600, and a 2 px accent bar.
- **PR1-9.** The selected clock hour uses accent text for its fill and border.
- **PR1-10.** Below 1400 px the unit column keeps its left gap.
- **PR1-11.** Velocity, energy, and density altitude are whole numbers. Drop is 1 dp. Nearby holds use the same rounding as the hero.
- **PR1-12.** The profile card is label / value / unit. `Ctrl P` does not wrap. Not trued is a warning badge.
- **PR1-13.** Moving target has one direction field, one hint, and an `s` chip on reaction time. The stored `3oc` is shown as `3 o'clock`.
- **PR1-14.** The legal line is capped at 24 characters. Ctrl U, Ctrl P, and F11 hide from 760 px to 1399 px.
- **PR1-16.** Stepper minimum height is `calc(var(--hit) + 2px)`. Step amounts are unchanged.
- **PR1-17.** Weather explanation sits under About weather data. The one-line Open-Meteo credit stays outside.
- **PR1-18.** The status hint says inclination, the pressure label says Pressure kind, and the humidity hint is `40%`.
- **PR1-19.** Preset, angular unit, appearance, and onboarding are radios with a roving tabindex. The focus ring is `:focus-visible` only. Skip to content lands on the input rail.
- **PR2-MUST-1.** Point-blank ordinate uses the small unit (`cm`) at 2 dp. The chart opens on the zone.
- **PR2-MUST-2.** Reticle grid lines mirror as `2*c - at` and do not double the centre line. `100 m` is not repeated. The hold label sits clear of the rings.
- **PR2-MUST-3.** Groups distance uses the string Distance, not the raw key.
- **PR2-MUST-4.** The range card builds for the active profile. Headers carry direction and unit, cells are unsigned, and the table wraps instead of clipping the wind columns. CSV uses the same units and 2 dp.
- **PR2-MUST-5.** Compare keeps the scope's own click unit, adds clicks and SG, splits rifle weight from bullet weight, and says no scope / no twist / not set instead of a dash.
- **PR2-MUST-6.** Accent text and warning are lifted to 4.5:1 on the background, the card, and the raised fill. Latte accent text is `#355baf`. Primary buttons use that colour.
- **PR2-MUST-7.** A failed range card shows a sentence. The Rust error is inside Details.
- **PR2-SHOULD-card.** Edition date is a calendar date. Temperature is `°C`. Drop is marked bore. Click subscripts use primary text.
- **PR2-SHOULD-pbr.** Zone is the default span, numbers are 2 dp, the long notes are in a disclosure, and the zone lines are dashed.
- **PR2-SHOULD-targets.** Hit area, silhouette, and other outlines use different dashes and a legend. Still to measure lists only the open target. Diameters are untouched.
- **PR2-SHOULD-groups.** The file input fits the rail. The fixture path is gone. Tools stay disabled until a photo is imported. The offset heading uses the existing mean-point wording.
- **PR2-SHOULD-truing.** The duplicate tip is gone. Snapshot precision matches the fit. The rail no longer clips the stepper.
- **PR2-SHOULD-equipment.** The page opens on a pick-a-row prompt. Twist hand is its own label.
- **PR2-SHOULD-charts.** Readouts use UP/DOWN and LEFT/RIGHT, time of flight has `s`, speed is `m/s`, the cursor starts at 0 m, and the transonic band has a swatch.
- **PR2-SHOULD-onboarding.** Metric is preselected, the choices are radios, and the phone segmented control wraps.
- **PR2-SHOULD-empty.** Range card, Compare, Charts, and Point-blank with nothing to show include Open Equipment.
- **NICE.** Narrow tables use tabular figures. Clock steps, group tools, and the transonic swatch have tooltips. Unit columns keep a left gap.

## Follow-up on 0ceb481

- **Clicks sign.** When the hold has a direction, the big figure does not also show a minus. `figureSign` is tested for LEFT `-1` and an undirected minus.
- **Phone context.** Below 760 px the Solve context strip stacks, so Range, wind, inclination, and air stay inside a 400 px width.
- **Air at 1280.** The Air grid is one column from 1100 px to 1399 px, so 1013.25 is not cut to 101.
- **Range card at 900.** The card no longer forces `overflow-x: visible`, which was clipping the last wind columns. Headers and cells wrap, and the table can scroll only if it still does not fit.
- **Shown units.** The card wind default is `2 m/s, 4 m/s, 6 m/s`, the impact default is `2 m/s, 4 m/s`, and the temperature placeholder is `-10 °C, 30 °C`. The quantity parser already accepts `m/s` and `°C`; those forms are now in the unit test.
- **Tinted rows.** A badge on the current range-card row, a selected list row, or the checked onboarding choice uses primary text so it stays above 4.5:1 on the tint.
- **Reticle labels.** Ring numbers are fanned around the face at 12 px, and the wind label sits under its dot.

## Not started / remaining

No review item was left unstarted. PR screenshots were skipped. There is no tag and no release.

## Notes for resuming

- Local checks that passed before the pause: `node --test app/ui/figure-parts.test.js`, `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`. GitHub CI on `616ac58` passed, both the `test` job (parser, format, tests, release build, clippy, py-ballisticcalc, timing) and the Arch `package` job.
- The only unsafe character class was `figureParts`. Other classes keep the hyphen at the end (`[+-]`), which is literal. A regex written as `return /…/` is not seen as a regex by `invoke_args` (the previous token is a letter), and a `'` inside it then looks like an unclosed string. `clockOk` assigns the regex to a variable so that test still passes.
- PDF range-card cells are still the old hold text. That was an accepted departure. The on-screen card, the CSV, and the edition date did change.
- The edition date is produced in the window (`edition_date` inside the body JSON, which stays snake_case). If it is missing or unsafe, Rust uses the UTC calendar date `YYYY-MM-DD`, not a full timestamp.
- Catppuccin Latte accent text moved from `#2c5fcb` to `#355baf` so primary buttons clear 4.5:1 on the raised fill. Tokyo Night accent text stays `#7aa2f7`. Control borders `#6d728c` and `#7c7f92` were left on the old lift. Ok green stays `#476f50` on Latte.
- Tauri command arguments stay camelCase. `edition_date` is a field of the body, not a command argument.
- A live 1.3.1 window was opened on a throwaway library long enough to see 16.56 MOA (exact 16.5632) and 48 clicks at 1000 m, a range card headed UP MOA / bore / LEFT 2 m/s, compare click `0.10 mrad`, and a point-blank ordinate in cm. Those captures were not put in the PR. The 1.3.0 binary built for a before shot was not captured.
- `docs/REDESIGN-PR1.md` and `docs/REDESIGN-PR2.md` still describe those pulls. This file is the 1.3.1 list.
