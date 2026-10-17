# Milestone 0 spike

Exit criteria from plan §30, and what this repository actually shows.

| Criterion | Result |
| --- | --- |
| MIT `LICENSE`, `Copyright (c) 2026 Patrik Hallgren` | Present on this branch. It is on `main` only after this pull request merges. |
| README, `.gitignore` excluding `holdover-refs/` and keystores | Present. |
| Workspace: `holdover-core`, `holdover-cli`, app scaffold | Present. The app crate is not compiled. |
| At least 20 documented reference vectors | `docs/references/vectors.md`. |
| v4 template proof | `share/holdover/holdover.json.tpl` renders with the Omarchy mix formula. `holdover-theme` tests the 8% and 16% mixes (`#252734`, `#313342`). |
| Live theme switch | Simulated. The adapter watches the parent `current/` directory and reads `theme/` after `next-theme` is renamed to `theme`. A corrupt file keeps the last valid palette; `colors.toml` is the fallback; bundled dark and light palettes meet 4.5:1. No Omarchy session was available, so a running desktop switch was not observed. |
| Recalculation feels instant | No solve screen yet. A debug solve to 2000 m stays under 200 ms in the test suite. On this machine a release `holdover-cli solve` of the 175 gr G7 profile to 2000 m took 0.91 ms median of five runs, including process start. CI does not yet fail a build that misses the 20 ms budget. |
| Crisp at 1.0 / 1.5 / 2.0 scaling | Not measured. WebKitGTK is not installed here. |
| Range-card print / PDF | Not measured. There is no card renderer in M0. |
| Omarchy text size | `text_scale_factor` reads `HOLDOVER_TEXT_SCALE`, then `gsettings get org.gnome.desktop.interface text-scaling-factor`, then 1.0. Whether WebKitGTK follows GTK's factor by itself was not measured. |

## What the spike did prove

- The parent-directory watch matches the v4 rename (`omarchy-theme-set` deletes `current/theme` and renames `next-theme` to `theme`).
- `holdover.json.tpl` uses `{{ key }}`, `_strip`, `_rgb` and `{{ mix a b 8% }}` the way `omarchy-theme-set-templates` does.
- Legacy `color0`–`color7` names fill `background`, `foreground` and the semantic colours.
- The optional hook `share/holdover/hooks/theme-set.d/holdover` only touches `$XDG_RUNTIME_DIR/holdover/theme-changed`. It does not edit Omarchy.

## Gaps carried forward

1. Closed 2026-10-16. The M1 checklist on Patrik's Omarchy 4 machine passed, and the shell is Tauri 2 (`docs/M1-DESKTOP.md`). Range-card print and PDF were not part of that check. The spike still does not claim that WebKitGTK follows GTK text scale by itself.
2. Closed 2026-10-16 for the live theme directory. The window followed an Omarchy theme switch with `HOLDOVER_THEME_DIR` unset. The template under `~/.config/omarchy/themed/` is still optional; without it the adapter reads `colors.toml`.
3. Aerodynamic-jump coefficients, and a check of the spin-drift coefficient against Litz's primary text.
4. GL as a Cd–Mach curve, if a trustworthy table turns up. The JBM GL download is not that table.
5. A CI timing gate for the 20 ms M1 budget. A local release measurement is already under that budget (0.91 ms median); the workflow does not record it.
