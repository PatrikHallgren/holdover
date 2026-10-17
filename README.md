# Holdover

Holdover is a personal, non-commercial, local-first ballistic calculator for the Omarchy 4 ("Quattro", 4.0.x) Linux desktop. An Android companion is planned later and is not in this milestone. Nothing here phones home, and nothing here is a substitute for live fire.

> All values are predictions from a mathematical model and your inputs. Confirm with live fire before relying on them. You are responsible for safe and lawful shooting.

The product plan is [`docs/PLAN.md`](docs/PLAN.md). This repository is Milestone 0: the solver library, its command-line front end, reference tests, and an Omarchy 4 theme-adapter proof. What the spike could and could not show on this machine is in [`docs/M0-SPIKE.md`](docs/M0-SPIKE.md). Choices the plan left open are in [`docs/DECISIONS.md`](docs/DECISIONS.md).

## Layout

| Path | Role |
| --- | --- |
| `crates/holdover-core` | Solver. No file or network I/O. |
| `crates/holdover-cli` | `holdover-cli`. Same JSON the later export will use. |
| `crates/holdover-theme` | Omarchy 4 template and `colors.toml` adapter. |
| `share/holdover` | `holdover.json.tpl` and the optional theme-set hook. |
| `app/` | Tauri 2 scaffold. Not compiled in CI. |
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

The Tauri shell is `holdover`. On Omarchy:

```sh
tools/desktop-spike.sh
```

That checks the Arch packages, prints the `sudo pacman` line without running it, builds the window, and checks a theme-directory rename. The human checklist (scaling, live theme, text size, tiling) is [`docs/M1-DESKTOP.md`](docs/M1-DESKTOP.md). Pass `--install` only if you want the script to run that pacman command.

## Command line

```sh
holdover-cli solve --profile examples/308-g7.json --range 437m --wind 3mps@9oc --json
```

`examples/308-g7.json` is a 175 grain .308 bullet, G7 BC 0.243, 800 m/s, 45 mm sight height, zeroed at 100 m in dry ICAO air. Spin drift is off in that file so the oracle comparison stays on drag and gravity.

`holdover-cli table` prints a range card as JSON or text. `holdover-cli atmosphere` prints density, the speed of sound and density altitude.

## Licence

[MIT](LICENSE). Copyright (c) 2026 Patrik Hallgren.
