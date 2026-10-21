# Milestone 5 desktop checklist

This is weather and the library file on the same Tauri 2 window as Milestone 4. CI compiles the app and checks the Open-Meteo parse, the offline and timeout failures, the stale age, and the library round-trip and rollback. It does not launch a window and it does not call Open-Meteo. Earlier shell checks are in [`M1-DESKTOP.md`](M1-DESKTOP.md), [`M2-DESKTOP.md`](M2-DESKTOP.md), [`M3-DESKTOP.md`](M3-DESKTOP.md), and [`M4-DESKTOP.md`](M4-DESKTOP.md).

## Commands

On the Omarchy machine, from the repo root. Rust is mise's `rust@1.98.1` with no global default.

Packages required to build and run. This does not install them:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel librsvg openssl
```

```sh
mise exec rust@1.98.1 -- cargo build --release -p holdover
HOLDOVER_DB=/tmp/holdover-m5.db ./target/release/holdover
```

The library is `$XDG_DATA_HOME/holdover/holdover.db`, or `~/.local/share/holdover/holdover.db`, unless `HOLDOVER_DB` is set. `/tmp/holdover-m5.db` is a throwaway library for this checklist.

## What to check

- **Manual air.** Open Solve. The badge under Current air says Manual. Type a temperature and a pressure. The hold updates. Header unit switch to US: temperature, pressure, and altitude on that page change unit. Switch back to Metric.
- **Place.** Settings. The first paragraph names appearance, weather, and the library file. Under Weather, type a place name, a latitude, and a longitude (Stockholm is about 59.33 and 18.07). Stale after stays 60. Save place. The line under the button says the place was saved.
- **Fetch.** Back on Solve. Fetch weather. The status says it is fetching, and you can still edit the air fields while that line is up. When the reply arrives, the panel shows temperature, humidity, station pressure marked as the surface value used for the solve, sea-level pressure labelled as not used, elevation, the grid cell, and model wind. The attribution line names Open-Meteo and CC BY 4.0, with both URLs, and says personal non-commercial use. Use station pressure. The badge says Fetched, with an age, the valid time, station pressure, and the elevation. The pressure kind control says Station. The hold recomputes.
- **Wind.** Use this wind speed. The wind speed changes. Direction stays what you typed. The note under the model wind says the degrees are where the wind comes from, not the shooting clock.
- **Sea level.** Fetch again and press Use sea-level pressure. The kind control says Sea level, and the badge says sea level rather than station. Fetch once more and use station pressure again.
- **Unit switch after a fetch.** With the fetch panel still open, switch the header to US. The panel’s temperature, station pressure, sea-level pressure, elevation, and wind speed change unit. The air fields do too. Switch back to Metric.
- **Stale.** Settings, set Stale after to 1, Save place. Wait a minute on Solve. The badge changes to Stale and still shows the age. Set Stale after back to 60 and save.
- **Edit clears the fetch.** Change the temperature by hand. The badge returns to Manual and the hold updates.
- **Offline.** Turn Wi-Fi off (or unplug Ethernet) so the machine has no route to the network. On Solve, Fetch weather. The red line says the weather could not be reached and that the numbers stay as typed. Type a new temperature. The hold still updates. Turn the network back on and fetch again to confirm it recovers.
- **Themes.** Settings, appearance Dark, then Solve. The fetched or manual badge, the attribution line, and the error line are readable on the dark background. Switch to Light and read the same lines.
- **Library preview.** Add a rifle with a serial on Equipment. Settings, Library file. Leave Include precise coordinates unchecked. Save backup. The path is under the buttons. Open the JSON: `schema_version` is 1, the rifle serial is empty, and there is no latitude. Save CSV. The first line is `format,schema_version,section,id,created_at,updated_at,revision,deleted_at,device_id,body`.
- **Nothing written yet.** Restore backup, choose that JSON. The preview says nothing has been written yet, and it lists counts and duplicates. Close the app without pressing Replace or Merge. Start again with the same `HOLDOVER_DB`. The rifle is still there, serial included, because the preview did not write.
- **Replace.** Start a second library with `HOLDOVER_DB=/tmp/holdover-m5-other.db ./target/release/holdover`, finish onboarding, and restore the JSON. Read the preview, then Replace library. The rifle from the first file is there. Its serial is still empty. Merge is the other button: incoming rows replace the same id, and rows only on this computer stay.
- **Not Holdover.** Restore backup and choose any other CSV (a spreadsheet export is enough). The message says Holdover imports only its own JSON and CSV, and the rifle list is unchanged.
- **Coordinates.** On the first library, check Include precise coordinates, save JSON again, and confirm `weather_latitude` is present. Uncheck it for the copies you keep.

## Still open

Rollback of a failed import is covered by `cargo test -p holdover import_failure_rolls_back`. A half-written file is not something the window can produce on purpose. A newer `schema_version` is refused with nothing written; that path is also a unit test.

The About screen that will repeat the Open-Meteo licence is Milestone 6. Spin drift is still the cited Litz equation and is still not marked verified against the book. Aerodynamic jump can be requested and is still not applied.

## Not in this build

No import from Strelok, a chronograph, or a Kestrel. No manufacturer bullet catalogue. No background weather refresh.
