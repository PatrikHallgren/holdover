# Desktop spike checklist

This checklist was the Tauri 2 versus Qt 6 Quick decision. The result is below: keep Tauri 2. The shell in `app/` is a small window on `holdover-core`, not the Milestone 2 solve screen. CI compiles it. It does not launch a window.

## Command

On the Omarchy machine, from the repo root:

```sh
tools/desktop-spike.sh
```

Rust is mise's `rust@1.98.1` with no global default. The script runs `mise exec rust@1.98.1 -- cargo build --release -p holdover`.

The script prints the packages and this install command. It does not run `sudo` unless you pass `--install`:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
tools/desktop-spike.sh --install
```

Those four are what this spike needs to build and run. `curl`, `wget`, `file`, `appmenu-gtk-module`, `libappindicator-gtk3`, and `xdotool` are on the Tauri 2 Arch prerequisite list (checked 2026-08-20) and are not required here. A missing one is a warning. The script keeps going. On 2026-10-16 it stopped because `wget` was missing; that check no longer aborts the build.

What the script does:

1. Checks the required packages with `pacman -Q`. A missing optional package, including `wget`, is a warning and does not stop the script.
2. Builds `target/release/holdover`.
3. Launches that binary against a temporary theme directory, writes accent `#112233`, then renames `next-theme` to `theme` with accent `#abcdef`, and waits until the process writes the new accent to its palette status file.
4. Leaves a second window running on the real `~/.local/state/omarchy/current` directory.

The template is not copied into `~/.config/omarchy/themed/`. Without that file, a live Omarchy switch is followed by reading `colors.toml`. Installing the template is a separate step, only if you want it:

```sh
mkdir -p ~/.config/omarchy/themed
cp share/holdover/holdover.json.tpl ~/.config/omarchy/themed/
```

## What to look at

- **Scaling.** Set the display scale to 1.0, then 1.5, then 2.0 (Omarchy's display setting, or Hyprland's monitor scale). The hold and the input text should stay sharp.
- **Theme switch.** The script already checks a directory rename. Then, with the second window still open, change the Omarchy theme from the menu. The window's background, text, and accent should follow within about a second, without a restart. That second window is not using `HOLDOVER_THEME_DIR`, so this is the live path.
- **Text scaling.** Quit the window and start it again with `HOLDOVER_TEXT_SCALE=1.5 target/release/holdover`. The "Text scale 1.50" line and the body type should grow. Also try Omarchy's text-size control with the variable unset. This spike reads the variable first and does not claim that WebKitGTK follows GTK's text scale by itself.
- **Tiling.** Tile the window left and right, then float it and drag the edges. The range field and the hold should stay visible. Nothing should clip.
- **Typing.** Change the range (`400m`, `500m`) and a wind (`3mps@9oc`). The hold should update on each change. `300` without a suffix should show an error that mentions `100m`.

## Result

Checked on Patrik's Omarchy 4 machine on 2026-10-16 (Arch, Hyprland). `cargo build --release -p holdover` succeeded with mise Rust 1.98.1. The window launched on the live theme directory (`HOLDOVER_THEME_DIR` unset).

Every item in this checklist passed:

- live Omarchy theme switch
- display scaling
- text scaling
- tiling and float resize
- typing and hold updates, including the unit-suffix error

**Decision: keep Tauri 2.** Qt 6 Quick is not the shell. Range-card print and PDF were not part of this check.
