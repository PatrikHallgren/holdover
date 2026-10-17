# Holdover app shell

Tauri 2 is still the preferred shell (plan §26.2). This directory is the scaffold only.

`src-tauri/Cargo.toml.scaffold` is not a Cargo workspace member, and CI does not compile it. This environment has no WebKitGTK and no Omarchy session, so the spike criteria that need a live window are open. See `docs/M0-SPIKE.md`.

`index.html` is a static token page: the disclaimer, the bundled dark palette, and the command that produces a solve. It is not the product UI.

Qt 6 Quick remains the fallback if the Tauri spike fails on Omarchy 4. It has not been started.
