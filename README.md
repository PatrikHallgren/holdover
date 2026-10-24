# Holdover

Holdover is a personal, non-commercial, local-first ballistic calculator for the Omarchy 4 ("Quattro", 4.0.x) Linux desktop. An Android companion is planned later and is not in this milestone. Nothing here phones home, and nothing here is a substitute for live fire.

> All values are predictions from a mathematical model and your inputs. Confirm with live fire before relying on them. You are responsible for safe and lawful shooting.

The product plan is [`docs/PLAN.md`](docs/PLAN.md). This tree prepares 1.3.1: the redesign fixes after the B dashboard. Check the window with [`docs/REDESIGN-FIX-1.3.1.md`](docs/REDESIGN-FIX-1.3.1.md). The 1.3.0 page checklist is [`docs/REDESIGN-PR2.md`](docs/REDESIGN-PR2.md). The guide is [`docs/USER-GUIDE.md`](docs/USER-GUIDE.md). Milestone 8 prepared 1.2.0: shot-group photo analysis ([`docs/M8-DESKTOP.md`](docs/M8-DESKTOP.md)). Milestone 7 prepared 1.1.0: a `.holdover` bundle with a merge preview ([`docs/M7-DESKTOP.md`](docs/M7-DESKTOP.md)). Milestone 6 prepared 1.0.0: compare, documents, the load log, archive, the Arch package, and About ([`docs/M6-DESKTOP.md`](docs/M6-DESKTOP.md)). Earlier checklists are [`docs/M2-DESKTOP.md`](docs/M2-DESKTOP.md), [`docs/M3-DESKTOP.md`](docs/M3-DESKTOP.md), [`docs/M4-DESKTOP.md`](docs/M4-DESKTOP.md), and [`docs/M5-DESKTOP.md`](docs/M5-DESKTOP.md). The Milestone 0 spike is [`docs/M0-SPIKE.md`](docs/M0-SPIKE.md). The Tauri decision is [`docs/M1-DESKTOP.md`](docs/M1-DESKTOP.md). Choices the plan left open are in [`docs/DECISIONS.md`](docs/DECISIONS.md).

## Layout

| Path | Role |
| --- | --- |
| `crates/holdover-core` | Solver. No file or network I/O. |
| `crates/holdover-cli` | `holdover-cli`. Same JSON the later export will use. |
| `crates/holdover-theme` | Omarchy 4 template and `colors.toml` adapter. |
| `share/holdover` | `holdover.json.tpl` and the optional theme-set hook. |
| `packaging/` | `PKGBUILD`, desktop entry, and icon. |
| `app/` | Tauri 2 window. Solve, range card, charts, equipment, compare, documents, sync, settings, About. |
| `locales/en.ftl` | English strings. The only shipped locale. |
| `docs/references/vectors.md` | Documented reference vectors. |
| `docs/schema/holdover.schema.json` | Draft library schema, including sync fields. |

Personal target photos belong in `holdover-refs/`, which git ignores. The official SJF moose-target photo is not in this repository.

## Build and test

Rust 1.85 or newer (stable is pinned in `rust-toolchain.toml`):

```sh
cargo test --workspace
cargo build --release --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The CI oracle is [py-ballisticcalc](https://github.com/o-murphy/py-ballisticcalc) 3.1.1. It is LGPL and is not vendored. Install it only for the comparison:

```sh
pip install py-ballisticcalc==3.1.1
python3 tools/oracle_check.py target/release/holdover-cli
```

GitHub Actions runs the same build, tests, clippy, the oracle, and a release timing gate (a full solve to 2000 m in under 20 ms). It also compiles the Tauri shell. See [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

The toolchain pin is Rust 1.98.1, the same version as `mise exec rust@1.98.1` on Omarchy.

## Desktop window

The Tauri app is `holdover`. On Omarchy, from the repo root:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
mise exec rust@1.98.1 -- cargo build --release -p holdover
./target/release/holdover
```

`wget` is not required. The human checklist for this window is [`docs/M6-DESKTOP.md`](docs/M6-DESKTOP.md). A throwaway library is `HOLDOVER_DB=/tmp/holdover-m6.db ./target/release/holdover`.

The Arch package, from a checkout:

```sh
cd packaging
mise exec rust@1.98.1 -- makepkg -si
```

The menu starts `/usr/bin/holdover`. A shell still runs `~/.local/bin/holdover` when that directory is earlier on `PATH`. Both use `~/.local/share/holdover/holdover.db`. Details are in the user guide.

`tools/desktop-spike.sh` is the earlier theme-rename check. It prints the same pacman line and does not run `sudo` unless you pass `--install`. The 2026-10-16 result (keep Tauri 2) is in [`docs/M1-DESKTOP.md`](docs/M1-DESKTOP.md).

## Command line

```sh
holdover-cli solve --profile examples/308-g7.json --range 437m --wind 3mps@9oc --json
```

`examples/308-g7.json` is a 175 grain .308 bullet, G7 BC 0.243, 800 m/s, 45 mm sight height, zeroed at 100 m in dry ICAO air. Spin drift is off in that file so the oracle comparison stays on drag and gravity.

`holdover-cli table` prints a range card as JSON or text. `holdover-cli atmosphere` prints density, the speed of sound and density altitude.

## Licence

[MIT](LICENSE). Copyright (c) 2026 Patrik Hallgren.
