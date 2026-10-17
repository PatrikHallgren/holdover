//! Turn a stored profile and the current form into a solver shot.
//!
//! The zero atmosphere comes from the profile. Current air is whatever the
//! Solve screen sent. They are not the same record.

use holdover_core::{
    format_number, parse_quantity, parse_twist, parse_wind_direction, table, Atmosphere, BcBand,
    BcReference, Bullet, Click, Dimension, DragModel, DragSpec, Effects, Shot, TableSpec, Twist,
    TwistHand, Warning, Wind,
};
use serde_json::{json, Value};

use crate::store::{atmosphere_from_form, stored_atmosphere, Store};

pub fn solve(store: &Store, body: &Value) -> Result<Value, String> {
    let angular = body
        .get("angular")
        .and_then(|v| v.as_str())
        .unwrap_or("mrad");
    let (shot, meta) = build_shot(store, body)?;
    let range_m = qty(body, "range", Dimension::Length, "m")?;
    if range_m <= 0.0 {
        return Err("range must be positive".into());
    }
    let solution = holdover_core::solve(&shot, range_m).map_err(|err| err.to_string())?;
    let point = solution
        .points
        .first()
        .ok_or("the solver returned no point")?;
    let step = qty(body, "range_step", Dimension::Length, "m").unwrap_or(10.0);
    let wind_step = qty(body, "wind_step", Dimension::Velocity, "mps").unwrap_or(1.0);
    let sensitivity = nearby(store, body, range_m, step, wind_step, angular);
    let mut remembered = body.clone();
    if let Some(object) = remembered.as_object_mut() {
        object.insert("angular".into(), json!(angular));
    }
    store.remember_solve(&remembered)?;
    let units = store.units()?;
    Ok(json!({
        "profile_id": meta.profile_id,
        "profile_label": meta.profile_label,
        "zero_summary": meta.zero_summary,
        "zero_air": meta.zero_air,
        "current_differs": meta.differs,
        "range_m": point.range_m,
        "elevation": hold_json(&point.elevation, angular),
        "windage": hold_json(&point.windage, angular),
        "velocity_mps": point.velocity_mps,
        "velocity_display": shown(point.velocity_mps, &units.velocity),
        "velocity_unit": units.velocity,
        "energy_j": point.energy_j,
        "energy_display": shown(point.energy_j, &units.energy),
        "energy_unit": units.energy,
        "drop_m": point.drop_m,
        "drop_display": shown(point.drop_m.abs(), &units.small),
        "drop_unit": units.small,
        "path_m": point.path_m,
        "tof_s": point.tof_s,
        "mach": point.mach,
        "sg": point.sg,
        "sg_corrected": point.sg_corrected,
        "spin_drift_m": point.spin_drift_m,
        "density_altitude_m": solution.density_altitude_m,
        "density_altitude_display": shown(solution.density_altitude_m, &units.length),
        "density_altitude_unit": units.length,
        "station_pressure_note": meta.station_note,
        "warnings": solution.warnings.iter().map(warning_text).collect::<Vec<_>>(),
        "transonic_range_m": solution.transonic_range_m,
        "bore_angle_mrad": solution.bore_angle_rad * 1000.0,
        "sensitivity": sensitivity,
        "angular": angular,
        "disclaimer": holdover_core::DISCLAIMER,
    }))
}

pub fn range_card(store: &Store, body: &Value) -> Result<Value, String> {
    let (mut shot, meta) = build_shot(store, body)?;
    // The elevation column is the no-wind hold. Wind brackets are crosswinds.
    shot.wind = Wind {
        speed_mps: 0.0,
        from_clock_rad: 0.0,
    };
    let start = qty(body, "from", Dimension::Length, "m")?;
    let end = qty(body, "to", Dimension::Length, "m")?;
    let step = qty(body, "step", Dimension::Length, "m")?;
    if step <= 0.0 || end < start {
        return Err("the card needs a positive step and an end at or after the start".into());
    }
    let rows_guess = ((end - start) / step).floor() as i32;
    if rows_guess > 80 {
        return Err("a card can have at most 80 rows".into());
    }
    let brackets = wind_brackets(body)?;
    let base = table(
        &shot,
        TableSpec {
            start_m: start,
            end_m: end,
            step_m: step,
        },
    )
    .map_err(|err| err.to_string())?;
    let mut wind_columns = Vec::new();
    for (label, speed) in &brackets {
        shot.wind = Wind {
            speed_mps: *speed,
            from_clock_rad: std::f64::consts::FRAC_PI_2 * 3.0,
        };
        let solved = table(
            &shot,
            TableSpec {
                start_m: start,
                end_m: end,
                step_m: step,
            },
        )
        .map_err(|err| err.to_string())?;
        wind_columns.push((label.clone(), solved));
    }
    let units = store.units()?;
    let length_unit = if body
        .get("pinned_metric")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        "m"
    } else {
        units.length.as_str()
    };
    let angular = body
        .get("angular")
        .and_then(|v| v.as_str())
        .unwrap_or(units.angular.as_str());
    let mut rows = Vec::new();
    for (index, point) in base.points.iter().enumerate() {
        let mut winds = Vec::new();
        for (label, solved) in &wind_columns {
            if let Some(wind_point) = solved.points.get(index) {
                winds.push(json!({
                    "label": label,
                    "windage": hold_json(&wind_point.windage, angular),
                    "windage_m": wind_point.windage_m,
                }));
            }
        }
        rows.push(json!({
            "range_m": point.range_m,
            "range_display": shown(point.range_m, length_unit),
            "elevation": hold_json(&point.elevation, angular),
            "drop_display": shown(point.drop_m.abs(), if length_unit == "m" { "cm" } else { "in" }),
            "velocity_display": shown(point.velocity_mps, &units.velocity),
            "energy_display": shown(point.energy_j, &units.energy),
            "tof_s": point.tof_s,
            "mach": point.mach,
            "winds": winds,
        }));
    }
    Ok(json!({
        "profile_label": meta.profile_label,
        "zero_summary": meta.zero_summary,
        "length_unit": length_unit,
        "angular": angular,
        "velocity_unit": units.velocity,
        "energy_unit": units.energy,
        "rows": rows,
        "brackets": brackets.iter().map(|(label, _)| label).collect::<Vec<_>>(),
        "disclaimer": holdover_core::DISCLAIMER,
        "effects": body.get("effects").cloned().unwrap_or(json!({})),
    }))
}

pub fn chart(store: &Store, body: &Value) -> Result<Value, String> {
    let ids = body
        .get("profile_ids")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if ids.is_empty() || ids.len() > 3 {
        return Err("choose one profile and at most two overlays".into());
    }
    let start = qty(body, "from", Dimension::Length, "m").unwrap_or(0.0);
    let end = qty(body, "to", Dimension::Length, "m").unwrap_or(1000.0);
    let step = qty(body, "step", Dimension::Length, "m").unwrap_or(50.0);
    let mut series = Vec::new();
    for id in ids {
        let id = id.as_str().unwrap_or("");
        let mut request = body.clone();
        request["profile_id"] = json!(id);
        let (shot, meta) = build_shot(store, &request)?;
        let solved = table(
            &shot,
            TableSpec {
                start_m: start,
                end_m: end,
                step_m: step,
            },
        )
        .map_err(|err| err.to_string())?;
        let points: Vec<Value> = solved
            .points
            .iter()
            .map(|point| {
                json!({
                    "range_m": point.range_m,
                    "path_m": point.path_m,
                    "windage_m": point.windage_m,
                    "drop_m": point.drop_m,
                })
            })
            .collect();
        series.push(json!({
            "id": id,
            "label": meta.profile_label,
            "points": points,
            "transonic_range_m": solved.transonic_range_m,
        }));
    }
    Ok(json!({ "series": series }))
}

struct Meta {
    profile_id: String,
    profile_label: String,
    zero_summary: String,
    zero_air: Value,
    differs: bool,
    station_note: Option<String>,
}

fn build_shot(store: &Store, body: &Value) -> Result<(Shot, Meta), String> {
    let profile_id = body
        .get("profile_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if profile_id.is_empty() {
        return Err("choose a profile".into());
    }
    let bundle = store.profile_bundle(profile_id)?;
    let profile = &bundle["profile"];
    let rifle = &bundle["rifle"];
    let scope = &bundle["scope"];
    let load = &bundle["load"];
    let bullet = &bundle["bullet"];
    let zero_air_value = &bundle["zero_air"];
    let zero_atmo = stored_atmosphere(zero_air_value)?;
    let current = atmosphere_from_form(body.get("current_air").unwrap_or(&json!({})))?;
    let differs = atmospheres_differ(&zero_atmo, &current);
    let bands = bullet
        .get("bands")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let reference = if bullet.get("bc_reference").and_then(|v| v.as_str()) == Some("Army") {
        BcReference::ArmyStandardMetro
    } else {
        BcReference::Icao
    };
    let mut bc_bands = Vec::new();
    for band in bands {
        bc_bands.push(BcBand {
            bc_lb_per_in2: band.get("bc").and_then(|v| v.as_f64()).unwrap_or(0.0),
            min_mps: band.get("min_mps").and_then(|v| v.as_f64()).unwrap_or(0.0),
            reference,
        });
    }
    if bc_bands.is_empty() {
        return Err("the bullet has no BC".into());
    }
    let model = DragModel::parse(
        bullet
            .get("drag_model")
            .and_then(|v| v.as_str())
            .unwrap_or("G7"),
    )
    .map_err(|err| err.to_string())?;
    let mut shot = Shot::basic(
        load.get("mv_mps").and_then(|v| v.as_f64()).unwrap_or(0.0),
        Bullet {
            weight_kg: bullet
                .get("weight_kg")
                .and_then(|v| v.as_f64())
                .or_else(|| bullet.get("weight_si").and_then(|v| v.as_f64()))
                .unwrap_or(0.0),
            diameter_m: bullet
                .get("diameter_m")
                .and_then(|v| v.as_f64())
                .or_else(|| bullet.get("diameter_si").and_then(|v| v.as_f64()))
                .unwrap_or(0.0),
            length_m: bullet
                .get("length_si")
                .and_then(|v| v.as_f64())
                .or_else(|| bullet.get("length_m").and_then(|v| v.as_f64())),
            drag: DragSpec::Standard {
                model,
                bands: bc_bands,
            },
        },
        current,
    );
    shot.sight_height_m = rifle
        .get("sight_height_si")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    shot.zero_range_m = Some(
        profile
            .get("zero_range_m")
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0),
    );
    shot.zero_offset_m = profile
        .get("zero_offset_m")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    shot.zero_atmosphere = Some(zero_atmo);
    shot.mv_temperature_c = load.get("mv_temperature_si").and_then(|v| v.as_f64());
    shot.temperature_sensitivity_mps_per_c = load.get("sensitivity_si").and_then(|v| v.as_f64());
    shot.zero_powder_temperature_c = profile.get("zero_powder_si").and_then(|v| v.as_f64());
    shot.twist = twist_of(rifle)?;
    shot.click = Some(Click {
        value_rad: scope
            .get("click_rad")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0001),
        tracking_factor: scope
            .get("tracking_factor")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0),
        travel_up_rad: scope.get("travel_up_si").and_then(|v| v.as_f64()),
        travel_down_rad: scope.get("travel_down_si").and_then(|v| v.as_f64()),
        travel_left_rad: scope.get("travel_left_si").and_then(|v| v.as_f64()),
        travel_right_rad: scope.get("travel_right_si").and_then(|v| v.as_f64()),
    });
    shot.wind = wind_of(body)?;
    shot.inclination_rad = qty(body, "inclination", Dimension::Angle, "deg").unwrap_or(0.0);
    shot.cant_rad = qty(body, "cant", Dimension::Angle, "deg").unwrap_or(0.0);
    shot.latitude_rad = qty(body, "latitude", Dimension::Angle, "deg").unwrap_or(0.0);
    shot.azimuth_rad = qty(body, "azimuth", Dimension::Angle, "deg").unwrap_or(0.0);
    shot.effects = effects_of(body);
    let state = shot.atmosphere.state().map_err(|err| err.to_string())?;
    let units = store.units()?;
    let zero_summary = format!(
        "{} {} · {} {} · {} · {}%",
        shown(shot.zero_range_m.unwrap_or(0.0), &units.length),
        units.length,
        shown(zero_atmo.temperature_c, &units.temperature),
        units.temperature,
        pressure_kind_label(zero_atmo.pressure_kind),
        format_number(zero_atmo.humidity * 100.0)
    );
    let station_note = if state.pressure_kind_suspicious {
        Some(format!(
            "Station pressure is {} {}. Density altitude is {} {}.",
            shown(state.station_pressure_pa, &units.pressure),
            units.pressure,
            shown(state.density_altitude_m, &units.length),
            units.length
        ))
    } else {
        None
    };
    Ok((
        shot,
        Meta {
            profile_id: profile_id.into(),
            profile_label: profile
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("Profile")
                .into(),
            zero_summary,
            zero_air: zero_air_value.clone(),
            differs,
            station_note,
        },
    ))
}

fn nearby(
    store: &Store,
    body: &Value,
    range_m: f64,
    step: f64,
    wind_step: f64,
    angular: &str,
) -> Value {
    let mut items = Vec::new();
    for (label, range) in [("minus", range_m - step), ("plus", range_m + step)] {
        if range <= 0.0 {
            continue;
        }
        if let Ok((shot, _)) = build_shot(store, body) {
            if let Ok(solution) = holdover_core::solve(&shot, range) {
                if let Some(point) = solution.points.first() {
                    items.push(json!({
                        "kind": "range",
                        "label": label,
                        "range_m": range,
                        "elevation": hold_json(&point.elevation, angular),
                        "windage": hold_json(&point.windage, angular),
                    }));
                }
            }
        }
    }
    if let Ok((mut shot, _)) = build_shot(store, body) {
        let base = shot.wind.speed_mps;
        for (label, speed) in [
            ("slower", (base - wind_step).max(0.0)),
            ("faster", base + wind_step),
        ] {
            shot.wind.speed_mps = speed;
            if let Ok(solution) = holdover_core::solve(&shot, range_m) {
                if let Some(point) = solution.points.first() {
                    items.push(json!({
                        "kind": "wind",
                        "label": label,
                        "speed_mps": speed,
                        "elevation": hold_json(&point.elevation, angular),
                        "windage": hold_json(&point.windage, angular),
                    }));
                }
            }
        }
    }
    json!(items)
}

fn wind_brackets(body: &Value) -> Result<Vec<(String, f64)>, String> {
    let raw = body
        .get("brackets")
        .and_then(|v| v.as_str())
        .unwrap_or("2 mps, 4 mps, 6 mps");
    let unit = body
        .get("bracket_unit")
        .and_then(|v| v.as_str())
        .unwrap_or("mps");
    let mut out = Vec::new();
    for part in raw.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let speed =
            parse_quantity(part, Dimension::Velocity, Some(unit)).map_err(|err| err.to_string())?;
        out.push((part.to_string(), speed));
    }
    if out.len() > 6 {
        return Err("use at most six wind brackets".into());
    }
    Ok(out)
}

fn wind_of(body: &Value) -> Result<Wind, String> {
    let speed_text = field_text(body, "wind_speed");
    let speed = if speed_text.trim().is_empty() {
        0.0
    } else {
        qty(body, "wind_speed", Dimension::Velocity, "mps")?
    };
    let direction = body
        .get("wind_direction")
        .and_then(|v| v.as_str())
        .unwrap_or("12oc");
    let from_clock_rad = if direction.trim().is_empty() {
        0.0
    } else {
        parse_wind_direction(direction).map_err(|err| err.to_string())?
    };
    Ok(Wind {
        speed_mps: speed,
        from_clock_rad,
    })
}

fn effects_of(body: &Value) -> Effects {
    let effects = body.get("effects").cloned().unwrap_or(json!({}));
    Effects {
        incline: effects
            .get("incline")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        spin_drift: effects
            .get("spin_drift")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        coriolis: effects
            .get("coriolis")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        cant: effects
            .get("cant")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        aerodynamic_jump: effects
            .get("aerodynamic_jump")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn twist_of(rifle: &Value) -> Result<Option<Twist>, String> {
    let text = rifle
        .get("twist_text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Ok(None);
    }
    let hand = rifle
        .get("twist_hand")
        .and_then(|v| v.as_str())
        .unwrap_or("right");
    let text = if hand == "left"
        && !text.to_lowercase().contains("lh")
        && !text.to_lowercase().contains("left")
    {
        format!("LH {text}")
    } else {
        text
    };
    let mut twist = parse_twist(&text).map_err(|err| err.to_string())?;
    twist.hand = if hand == "left" {
        TwistHand::Left
    } else {
        twist.hand
    };
    Ok(Some(twist))
}

fn qty(body: &Value, key: &str, dim: Dimension, fallback: &str) -> Result<f64, String> {
    let value = body.get(key).ok_or_else(|| format!("{key} is required"))?;
    let (text, unit) = match value {
        Value::String(text) => (text.as_str(), fallback),
        Value::Object(map) => (
            map.get("text").and_then(|v| v.as_str()).unwrap_or(""),
            map.get("unit").and_then(|v| v.as_str()).unwrap_or(fallback),
        ),
        Value::Number(number) => {
            return number
                .as_f64()
                .ok_or_else(|| format!("{key} is not a number"))
        }
        _ => return Err(format!("{key} is required")),
    };
    if text.trim().is_empty() {
        return Err(format!("{key} is required"));
    }
    parse_quantity(text, dim, Some(unit)).map_err(|err| format!("{key}: {err}"))
}

fn field_text(body: &Value, key: &str) -> String {
    match body.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Object(map)) => map
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn hold_json(hold: &holdover_core::AngularHold, angular: &str) -> Value {
    let magnitude = match angular.to_ascii_lowercase().as_str() {
        "moa" => format!("{} MOA", format_number(hold.moa.abs())),
        "iphy" => format!("{} IPHY", format_number(hold.iphy.abs())),
        "cm@100m" | "cm/100m" => format!("{} cm/100 m", format_number(hold.cm_per_100m.abs())),
        "clicks" => match hold.clicks {
            Some(clicks) => format!("{clicks}"),
            None => "—".into(),
        },
        _ => format!("{} mrad", format_number(hold.mrad.abs())),
    };
    json!({
        "direction": direction_name(hold.direction),
        "mrad": hold.mrad,
        "moa": hold.moa,
        "iphy": hold.iphy,
        "cm_per_100m": hold.cm_per_100m,
        "clicks": hold.clicks,
        "residual_mrad": hold.residual_mrad,
        "primary": magnitude,
    })
}

fn direction_name(direction: holdover_core::Direction) -> &'static str {
    match direction {
        holdover_core::Direction::Up => "UP",
        holdover_core::Direction::Down => "DOWN",
        holdover_core::Direction::Left => "LEFT",
        holdover_core::Direction::Right => "RIGHT",
        holdover_core::Direction::None => "—",
    }
}

fn warning_text(warning: &Warning) -> String {
    match warning {
        Warning::DragClamped => "Drag was clamped to the end of the table.".into(),
        Warning::DefaultZeroAtmosphere => {
            "This zero has no atmosphere of its own, so ICAO was used.".into()
        }
        Warning::PressureKindSuspicious => {
            "This looks like sea-level pressure entered as station pressure above 500 m.".into()
        }
        Warning::AerodynamicJumpNotApplied => {
            "Aerodynamic jump was requested and not applied.".into()
        }
        Warning::SpinDriftMissingLength => "Spin drift needs a bullet length.".into(),
        Warning::ExceedsTravel { axis } => {
            format!("The {axis} hold is past the scope travel entered for this scope.")
        }
    }
}

fn atmospheres_differ(zero: &Atmosphere, current: &Atmosphere) -> bool {
    (zero.temperature_c - current.temperature_c).abs() > 0.05
        || (zero.pressure_pa - current.pressure_pa).abs() > 1.0
        || zero.pressure_kind != current.pressure_kind
        || (zero.humidity - current.humidity).abs() > 0.001
        || (zero.altitude_m - current.altitude_m).abs() > 0.5
}

fn pressure_kind_label(kind: holdover_core::PressureKind) -> &'static str {
    match kind {
        holdover_core::PressureKind::Station => "station",
        holdover_core::PressureKind::SeaLevel => "sea level",
    }
}

fn shown(si: f64, unit: &str) -> String {
    holdover_core::format_quantity(si, unit).unwrap_or_else(|_| format_number(si))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use uuid::Uuid;

    fn store() -> Store {
        let path = std::env::temp_dir().join(format!(
            "holdover-session-{}-{}.db",
            std::process::id(),
            Uuid::new_v4()
        ));
        let store = Store::open(&path).unwrap();
        let id = store.seed_example().unwrap();
        store.set_active(&id).unwrap();
        store
    }

    #[test]
    fn example_at_300_m_matches_the_oracle_when_spin_drift_is_off() {
        let store = store();
        let id = store.bootstrap().unwrap()["settings"]["active_profile_id"]
            .as_str()
            .unwrap()
            .to_string();
        let body = json!({
            "profile_id": id,
            "range": {"text": "300 m", "unit": "m"},
            "wind_speed": {"text": "0 mps", "unit": "mps"},
            "wind_direction": "12oc",
            "inclination": {"text": "0 deg", "unit": "deg"},
            "current_air": {
                "temperature": {"text": "15 C", "unit": "C"},
                "pressure": {"text": "1013.25 hPa", "unit": "hPa"},
                "pressure_kind": "station",
                "humidity": "0%",
                "altitude": {"text": "0 m", "unit": "m"}
            },
            "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false},
            "angular": "mrad",
            "range_step": {"text": "10 m", "unit": "m"},
            "wind_step": {"text": "1 mps", "unit": "mps"}
        });
        let solved = solve(&store, &body).unwrap();
        let mrad = solved["elevation"]["mrad"].as_f64().unwrap();
        assert!((mrad - 1.6097).abs() < 0.002, "{mrad}");
        assert!(solved["elevation"]["clicks"].as_i64().unwrap() > 0);
        let zero_text = store.profile_bundle(&id).unwrap()["zero_air"]["temperature_text"]
            .as_str()
            .unwrap()
            .to_string();
        let warm = json!({
            "profile_id": id,
            "range": {"text": "300 m", "unit": "m"},
            "wind_speed": {"text": "0 mps", "unit": "mps"},
            "wind_direction": "12oc",
            "inclination": {"text": "0 deg", "unit": "deg"},
            "current_air": {
                "temperature": {"text": "30 C", "unit": "C"},
                "pressure": {"text": "1013.25 hPa", "unit": "hPa"},
                "pressure_kind": "station",
                "humidity": "0%",
                "altitude": {"text": "0 m", "unit": "m"}
            },
            "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
        });
        let warm_solve = solve(&store, &warm).unwrap();
        assert!(warm_solve["current_differs"].as_bool().unwrap());
        let zero_after = store.profile_bundle(&id).unwrap()["zero_air"]["temperature_text"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(zero_after, zero_text);
        assert!((warm_solve["elevation"]["mrad"].as_f64().unwrap() - mrad).abs() > 1e-6);

        let mut moa = body.clone();
        moa["angular"] = json!("MOA");
        moa["wind_speed"] = json!({"text": "10 mps", "unit": "mps"});
        let labelled = solve(&store, &moa).unwrap();
        assert!(
            labelled["elevation"]["primary"]
                .as_str()
                .unwrap()
                .contains("MOA"),
            "{}",
            labelled["elevation"]["primary"]
        );
        let card = range_card(
            &store,
            &json!({
                "profile_id": id,
                "from": {"text": "300 m", "unit": "m"},
                "to": {"text": "300 m", "unit": "m"},
                "step": {"text": "300 m", "unit": "m"},
                "brackets": "4 mps",
                "wind_speed": {"text": "10 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "inclination": {"text": "0 deg", "unit": "deg"},
                "current_air": body["current_air"],
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false},
                "angular": "mrad"
            }),
        )
        .unwrap();
        let card_mrad = card["rows"][0]["elevation"]["mrad"].as_f64().unwrap();
        assert!((card_mrad - mrad).abs() < 0.002, "{card_mrad} vs {mrad}");
    }
}
