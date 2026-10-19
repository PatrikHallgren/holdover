# Milestone 4 desktop checklist

This is reticles and targets on the same Tauri 2 window as Milestone 3. CI compiles the app and checks second-focal-plane scaling, angular size, and the target badges. It does not launch a window. Earlier shell checks are in [`M1-DESKTOP.md`](M1-DESKTOP.md), [`M2-DESKTOP.md`](M2-DESKTOP.md), and [`M3-DESKTOP.md`](M3-DESKTOP.md).

## Commands

On the Omarchy machine, from the repo root. Rust is mise's `rust@1.98.1` with no global default.

Packages required to build and run. This does not install them:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
```

```sh
mise exec rust@1.98.1 -- cargo build --release -p holdover
HOLDOVER_DB=/tmp/holdover-m4.db ./target/release/holdover
```

The library is `$XDG_DATA_HOME/holdover/holdover.db`, or `~/.local/share/holdover/holdover.db`, unless `HOLDOVER_DB` is set. `/tmp/holdover-m4.db` is a throwaway library for this checklist.

## What to check

- **Reticle.** Open Reticle. The paragraph under the heading says the generic set is not a manufacturer reticle. The list is duplex, mil-dot, MOA hash, mil-hash, mil-tree, and two dot drawings. Pick mil-dot, set focal plane to second, calibrated magnification 10, and press 5×, then 10×, then 20×. The readout scale factor reads 2.000, then 1.000, then 0.500. At 5× and 20× a badge says the marks are not their nominal size. Switch to first focal plane and press the same three buttons: the factor stays 1.000 and the badge stays hidden. The drawing fills the page width and stays square.
- **Angular size.** Stay on mil-dot, first focal plane, 10×, range `100 m`, target Älgfigur. The readout gives an inner-ring pixel size. Change the range to `200 m`: that pixel size is about half. Change magnification to 20×: the ring grows. The solver check for this is within 1 px of `diameter / range` times the pixels per milliradian.
- **Hold, wind, and lead.** On Solve, open Moving target, set `5.3 mps` and `3oc`, and pick Älgfigur. The face under the lead numbers shows the rings, a dashed placeholder, and the badges. Back on Reticle, turn on Show the Solve hold. The target sits off the optical centre. Wind and Lead marks use the theme chart colours. Turn the checkbox off: those two marks go away and the target returns to the centre.
- **Editor.** Tool Line, snap `0.5`. Click two points: a line appears on the grid. Mirror left-right flips it. Undo removes the last mark. Save reticle stores a copy. The generic mil-dot entry is still in the list afterwards. Export JSON downloads a file. Import JSON loads it and does not write it until Save reticle.
- **Targets and badges.** Open Targets. The list at the top names T2, T3, T4, T5, and T6, plus the US items that have no confirmed rulebook size. Älgfigur shows rings 5¹, 5, 4, and 3, a dashed hit area, and a dashed silhouette, with Placeholder badges. Ring labels is Alternate labels: the numbers become 5, 4, 3, and 2, and they sit on the horizontal axis. Primary labels sit up and to the right. Uncheck Decorative silhouette: the large dashed moose goes away and the rings remain. Vildsvin, vildsvinspasset, björnpasset, viltmål, the 300 m target, IPSC/USPSA, IDPA, IPSC steel, and Rectangular plate say “No published size yet” and carry an Unverified badge. Red deer and fallow deer have an empty vital-zone field and no circle until you type one. After you type one and the picture updates, the T6 badge is still there.
- **Impact and the unit switch.** Charts → Impact. Target Älgfigur. The face fills the width, the rings are labelled, and T2 and T3 are on the picture. Change the target to Stående råbock: the face redraws without resizing the window. Direction Right-going mirrors the dashed oval. Header unit switch to US: the scale caption under the face changes from centimetres to inches. Switch back to Metric.
- **Themes.** Settings → appearance → Dark, then Targets. Ring numbers, the scale caption, and the dashed placeholder are light on the dark face. Switch to Light: the same strokes and words are dark on the light face.
- **True-size PDF.** On Targets, page size A4, Älgfigur, Decorative silhouette off. Open PDF. In the PDF viewer, print at **100% / Actual size**, paper **A4**, fit-to-page **off**. Measure the black bar: 10 cm. The moose rings are larger than the page, so the sheet says a scale other than life size. The bar is still 10 cm. A6 and Compact use a 5 cm bar. Do not measure a print of the Holdover window.

## Still open, and what to send

These stay badged until a measurement replaces them. Editing a shape does not clear the badge.

- **T2.** Moose hit-area outline. Photograph the official target straight on, with a ruler in the same plane, and all four corners in the frame. Or measure width, height, and the ring centre to the left edge, the rear apex, the top, and the bottom.
- **T3.** Moose silhouette: overall width and height, where the ring centre sits on the figure, and siamese spacing if you have that board.
- **T4.** Confirm roe-buck rings 10, 15, and 20 cm on an owned target, plus the hit-area outline and the figure size. The oval in the app is a stand-in.
- **T5.** Any other owned official target: moose calf, boar, fox, capercaillie, bear pass, SvSF viltmål, the 300 m target, and field-figure silhouettes. Field rectangles already use the published width and height. The shape inside them is not measured. IPSC, IDPA, and IPSC steel need the current rulebook page before any zone is drawn.
- **T6.** A vital-zone diameter you want as the practice default for kronhjort and dovhjort. There is no official figure in the sources checked, so the app does not guess one.

Keep photos in `holdover-refs/` on your machine. That folder stays out of the public repo.

## Not in this build

No manufacturer reticle is bundled. Group statistics stay in a later milestone. Spin drift is still the cited Litz equation and is still not marked verified against the book. Aerodynamic jump can be requested and is still not applied.
