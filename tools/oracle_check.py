#!/usr/bin/env python3
"""Compare holdover-cli with py-ballisticcalc 3.1.1.

py-ballisticcalc is LGPL-3.0. CI installs it; this repository does not vendor it.
The check uses examples/308-g7.json: 175 gr, .308 in, G7 BC 0.243, 800 m/s,
45 mm sight height, 100 m zero, dry ICAO at sea level, spin drift off.

Elevation is compared as a positive-up hold in milliradians. Windage is compared
in metres (impact to the right of the line of sight is positive). py-ballisticcalc's
windage angle is not used: its angle definition does not match Holdover's hold.
"""

from __future__ import annotations

import json
import math
import subprocess
import sys
from pathlib import Path

from py_ballisticcalc import (
    Ammo,
    Atmo,
    Calculator,
    DragModel,
    Shot,
    TableG7,
    Unit,
    Weapon,
    Wind,
)

ROOT = Path(__file__).resolve().parents[1]
PROFILE = ROOT / "examples" / "308-g7.json"
RANGES_M = (100, 200, 300, 500, 800, 1000)
WIND_RANGE_M = 300


def holdover(cli: str, args: list[str]) -> dict:
    completed = subprocess.run(
        [cli, *args],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def mrad_of(metres: float, range_m: float) -> float:
    return math.atan2(metres, range_m) * 1000.0


def index_by_range(fired) -> dict[int, object]:
    found: dict[int, object] = {}
    for point in fired:
        metres = float(point.distance >> Unit.Meter)
        found[round(metres)] = point
    return found


def py_shot(wind_mps: float | None) -> Shot:
    atmo = Atmo(
        altitude=Unit.Meter(0),
        pressure=Unit.hPa(1013.25),
        temperature=Unit.Celsius(15),
        humidity=0,
    )
    drag = DragModel(0.243, TableG7, weight=Unit.Grain(175), diameter=Unit.Inch(0.308))
    ammo = Ammo(drag, mv=Unit.MPS(800))
    # Twist of zero keeps the Litz spin-drift term off, matching the profile.
    weapon = Weapon(sight_height=Unit.Millimeter(45), twist=0)
    winds = []
    if wind_mps is not None:
        winds.append(Wind(velocity=Unit.MPS(wind_mps), direction_from=Unit.Degree(90)))
    return Shot(weapon=weapon, ammo=ammo, atmo=atmo, winds=winds)


def main() -> int:
    cli = sys.argv[1] if len(sys.argv) > 1 else "holdover-cli"
    failures: list[str] = []

    ours = {}
    bore = None
    for range_m in RANGES_M:
        data = holdover(cli, ["solve", "--profile", str(PROFILE), "--range", f"{range_m}m", "--json"])
        ours[range_m] = data["points"][0]
        bore = data["bore_angle_mrad"]

    calc = Calculator()
    calm = py_shot(None)
    calc.set_weapon_zero(calm, Unit.Meter(100))
    py_bore = float(calm.weapon.zero_elevation >> Unit.MRad)
    fired = calc.fire(
        calm,
        trajectory_range=Unit.Meter(1000),
        trajectory_step=Unit.Meter(100),
    )
    reference = index_by_range(fired)

    print(f"bore_angle_mrad  holdover={bore:.4f}  py-ballisticcalc={py_bore:.4f}")
    print(f"{'m':>6} {'elev mrad':>10} {'py mrad':>10} {'d mrad':>9} {'v m/s':>8} {'py v':>8} {'tof':>8} {'py tof':>8} {'mach':>6}")
    for range_m in RANGES_M:
        point = reference.get(range_m)
        if point is None:
            failures.append(f"py-ballisticcalc did not return {range_m} m")
            continue
        row = ours[range_m]
        py_elev = abs(float(point.drop_angle >> Unit.MRad))
        py_v = float(point.velocity >> Unit.MPS)
        py_t = float(point.time)
        d_elev = row["elevation_mrad"] - py_elev
        mach = row["mach"]
        limit = 0.05 if mach >= 1.2 else 0.1
        rel_v = abs(row["velocity_mps"] - py_v) / py_v
        rel_t = abs(row["tof_s"] - py_t) / py_t
        print(
            f"{range_m:6.0f} {row['elevation_mrad']:10.4f} {py_elev:10.4f} {d_elev:9.4f} "
            f"{row['velocity_mps']:8.2f} {py_v:8.2f} {row['tof_s']:8.4f} {py_t:8.4f} {mach:6.3f}"
        )
        if abs(d_elev) > limit:
            failures.append(f"{range_m} m elevation {d_elev:.4f} mrad exceeds {limit}")
        if rel_v > 0.005:
            failures.append(f"{range_m} m velocity differs by {rel_v:.3%}")
        if rel_t > 0.005:
            failures.append(f"{range_m} m time of flight differs by {rel_t:.3%}")

    wind = holdover(
        cli,
        ["solve", "--profile", str(PROFILE), "--range", f"{WIND_RANGE_M}m", "--wind", "3mps@9oc", "--json"],
    )["points"][0]
    breezy = py_shot(3.0)
    calc.set_weapon_zero(breezy, Unit.Meter(100))
    blown = index_by_range(
        calc.fire(breezy, trajectory_range=Unit.Meter(WIND_RANGE_M), trajectory_step=Unit.Meter(WIND_RANGE_M))
    )[WIND_RANGE_M]
    py_wind_m = float(blown.windage >> Unit.Meter)
    d_wind_mrad = mrad_of(wind["windage_m"] - py_wind_m, WIND_RANGE_M)
    print(
        f"wind {WIND_RANGE_M} m, 3 m/s from 9 o'clock: "
        f"holdover={wind['windage_m']:.5f} m  py={py_wind_m:.5f} m  "
        f"delta={d_wind_mrad:.4f} mrad"
    )
    if abs(d_wind_mrad) > 0.05:
        failures.append(f"crosswind windage delta {d_wind_mrad:.4f} mrad exceeds 0.05")

    if failures:
        print("ORACLE FAIL")
        for item in failures:
            print(f"  {item}")
        return 1
    print("ORACLE PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
