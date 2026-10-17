# Bundled drag functions

These files are Cd-versus-Mach tables for the standard projectiles G1, G2, G5, G6, G7 and G8.

Source: JBM Ballistics downloads page, files `mcg1.txt`, `mcg2.txt`, `mcg5.txt`, `mcg6.txt`, `mcg7.txt` and `mcg8.txt`.

<https://www.jbmballistics.com/downloads.html>

JBM states that the programs and files on that page are free for public use. The Cd tables were obtained by JBM from the US Army Ballistic Research Laboratory (via Robert L. McCoy). They are the only bundled ballistic data in Holdover. No manufacturer bullet library is included.

The original tables are stored here unchanged (Mach, then Cd, whitespace-separated). Holdover interpolates them with a monotone PCHIP spline and clamps outside the tabulated Mach range.

GL is not in this set. On the same JBM page, "GL" is a Lowry (1965) drag function tabulated against velocity in feet per second, not a Cd–Mach curve, so it does not fit the Cd(M) formula in the plan. See `docs/DECISIONS.md`.
