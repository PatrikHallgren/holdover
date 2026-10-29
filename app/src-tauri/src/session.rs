//! Turn a stored profile and the current form into a solver shot.
//!
//! The zero atmosphere comes from the profile. Current air is whatever the
//! Solve screen sent. They are not the same record.

use holdover_core::{
    card_page, chart_axis, chrono_stats, combine_offsets, convert_from_si, format_number, lead,
    parse_quantity, parse_twist, parse_wind_direction, pbr, pbr_fixed, powder_coefficient,
    print_sheet_css, recoil, render_card_pdf, scale_bar_mm, table, true_drag, true_mv,
    true_tracking, vertical_mv_spread_m, Atmosphere, BcBand, BcReference, Bullet, CardSheet,
    CdScale, ChartAxis, Click, Dimension, DragMode, DragModel, DragSpec, Effects, RecoilInput,
    Shot, Solution, TableSpec, TrajectoryPoint, Twist, TwistHand, Warning, Wind, DRAG_GUIDANCE,
    MV_GUIDANCE, TRACKING_GUIDANCE,
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
        "truing": meta.truing_label,
        "lead": lead_of(&shot, body, range_m, point, angular, &units.length)?,
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
    let page_id = body.get("page").and_then(|v| v.as_str()).unwrap_or("a4");
    let page = card_page(page_id).ok_or_else(|| format!("unknown card size {page_id}"))?;
    let conditions = condition_tables(&shot, body, start, end, step)?;
    let condition_labels: Vec<String> = conditions.iter().map(|(label, _)| label.clone()).collect();
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
        let mut condition_holds = Vec::new();
        for (label, solved) in &conditions {
            if let Some(condition_point) = solved.points.get(index) {
                condition_holds.push(json!({
                    "label": label,
                    "elevation": hold_json(&condition_point.elevation, angular),
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
            "conditions": condition_holds,
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
        "footer": "Predicted values; confirm with live fire.",
        "effects": body.get("effects").cloned().unwrap_or(json!({})),
        "truing": meta.truing_label,
        "date": edition_label(body),
        "page": {
            "id": page.id,
            "name": page.name,
            "width_mm": page.width_mm,
            "height_mm": page.height_mm,
        },
        "scale_bar_mm": scale_bar_mm(page),
        "scale_bar_label": if scale_bar_mm(page) >= 100.0 { "10 cm" } else { "5 cm" },
        "print_css": print_sheet_css(page),
        "conditions": condition_labels,
        "csv": card_csv(&rows, &brackets, angular, length_unit),
    }))
}

pub fn card_pdf_bytes(store: &Store, body: &Value) -> Result<Vec<u8>, String> {
    let mut owned = body.clone();
    if owned.get("page").and_then(|value| value.as_str()).is_none() {
        owned["page"] = json!(store.paper_id()?);
    }
    let page_id = owned
        .get("page")
        .and_then(|value| value.as_str())
        .unwrap_or("a4")
        .to_string();
    store.remember_paper(&page_id)?;
    let card = range_card(store, &owned)?;
    let page = card_page(&page_id).ok_or_else(|| format!("unknown card size {page_id}"))?;
    Ok(render_card_pdf(page, &sheet_from_card(&card)))
}

fn sheet_from_card(card: &Value) -> CardSheet {
    let mut headers = vec![
        "Range".to_string(),
        "Elevation".to_string(),
        "Drop".to_string(),
        "Velocity".to_string(),
        "Energy".to_string(),
        "TOF".to_string(),
        "Mach".to_string(),
    ];
    if let Some(brackets) = card.get("brackets").and_then(|value| value.as_array()) {
        for label in brackets {
            headers.push(label.as_str().unwrap_or("Wind").to_string());
        }
    }
    if let Some(conditions) = card.get("conditions").and_then(|value| value.as_array()) {
        for label in conditions {
            headers.push(label.as_str().unwrap_or("Temp").to_string());
        }
    }
    let length = card["length_unit"].as_str().unwrap_or("");
    let mut rows = Vec::new();
    if let Some(source) = card.get("rows").and_then(|value| value.as_array()) {
        for row in source {
            let mut cells = vec![
                format!("{} {length}", row["range_display"].as_str().unwrap_or("")),
                hold_cell(&row["elevation"]),
                row["drop_display"].as_str().unwrap_or("").to_string(),
                row["velocity_display"].as_str().unwrap_or("").to_string(),
                row["energy_display"].as_str().unwrap_or("").to_string(),
                format!("{:.3}", row["tof_s"].as_f64().unwrap_or(0.0)),
                format!("{:.2}", row["mach"].as_f64().unwrap_or(0.0)),
            ];
            if let Some(winds) = row.get("winds").and_then(|value| value.as_array()) {
                for wind in winds {
                    cells.push(hold_cell(&wind["windage"]));
                }
            }
            if let Some(conditions) = row.get("conditions").and_then(|value| value.as_array()) {
                for condition in conditions {
                    cells.push(hold_cell(&condition["elevation"]));
                }
            }
            rows.push(cells);
        }
    }
    let label = card["scale_bar_label"].as_str().unwrap_or("10 cm");
    CardSheet {
        title: card["profile_label"]
            .as_str()
            .unwrap_or("Range card")
            .to_string(),
        subtitle: card["zero_summary"].as_str().unwrap_or("").to_string(),
        meta: vec![
            format!(
                "Truing: {} · {}",
                card["truing"].as_str().unwrap_or(""),
                card["date"].as_str().unwrap_or("")
            ),
            format!("No-wind elevation. Crosswind brackets from 9 o'clock. Scale bar {label}."),
        ],
        headers,
        rows,
        footer: card["footer"]
            .as_str()
            .unwrap_or("Predicted values; confirm with live fire.")
            .to_string(),
        disclaimer: card["disclaimer"].as_str().unwrap_or("").to_string(),
        scale_label: label.to_string(),
    }
}

fn hold_cell(hold: &Value) -> String {
    format!(
        "{} {}",
        hold["direction"].as_str().unwrap_or(""),
        hold["primary"].as_str().unwrap_or("")
    )
}

pub fn chart(store: &Store, body: &Value) -> Result<Value, String> {
    let ids = body
        .get("profile_ids")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if ids.is_empty() || ids.len() > 6 {
        return Err("choose one profile and at most five overlays".into());
    }
    let start = qty(body, "from", Dimension::Length, "m").unwrap_or(0.0);
    let end = qty(body, "to", Dimension::Length, "m").unwrap_or(1000.0);
    let step = qty(body, "step", Dimension::Length, "m").unwrap_or(50.0);
    let units = store.units()?;
    let angular = body
        .get("angular")
        .and_then(|v| v.as_str())
        .unwrap_or(units.angular.as_str());
    let (hold_unit, hold_label, clicks_axis) = hold_scale(angular);
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
                    "range": shown_value(point.range_m, &units.length),
                    "path": shown_value(point.path_m, &units.small),
                    "wind_linear": shown_value(point.windage_m, &units.small),
                    "elevation": shown_value(point.elevation.rad, &hold_unit),
                    "windage": shown_value(point.windage.rad, &hold_unit),
                    "velocity": shown_value(point.velocity_mps, &units.velocity),
                    "energy": shown_value(point.energy_j, &units.energy),
                    "tof": point.tof_s,
                    "mach": point.mach,
                })
            })
            .collect();
        series.push(json!({
            "id": id,
            "label": meta.profile_label,
            "points": points,
            "transonic_range_m": solved.transonic_range_m,
            "transonic": solved.transonic_range_m.map(|range| shown_value(range, &units.length)),
        }));
    }
    let (range_lo, range_hi) = extent(&series, "range");
    let (path_lo, path_hi) = extent(&series, "path");
    let (wind_lo, wind_hi) = extent(&series, "windage");
    let (velocity_lo, velocity_hi) = extent(&series, "velocity");
    let (energy_lo, energy_hi) = extent(&series, "energy");
    let (tof_lo, tof_hi) = extent(&series, "tof");
    let (mach_lo, mach_hi) = extent(&series, "mach");
    let transonic = series
        .first()
        .and_then(|item| item.get("transonic"))
        .cloned()
        .unwrap_or(Value::Null);
    Ok(json!({
        "x": axis_json(&chart_axis(range_lo, range_hi, 6, true), &units.length),
        "drop": {
            "unit": units.small,
            "hold_unit": hold_label,
            "axis": axis_json(&chart_axis(path_lo, path_hi, 5, true), &units.small),
            "zero": "los",
        },
        "wind": {
            "unit": hold_label,
            "linear_unit": units.small,
            "hold_unit": hold_label,
            "clicks_axis": clicks_axis,
            "axis": axis_json(&chart_axis(wind_lo, wind_hi, 5, true), &hold_label),
            "zero": "zero",
        },
            "velocity": {
            "unit": pretty_unit(&units.velocity),
            "axis": axis_json(&chart_axis(velocity_lo, velocity_hi, 5, false), &pretty_unit(&units.velocity)),
        },
        "energy": {
            "unit": units.energy,
            "axis": axis_json(&chart_axis(energy_lo, energy_hi, 5, false), &units.energy),
        },
        "tof": {
            "unit": "s",
            "axis": axis_json(&chart_axis(tof_lo, tof_hi, 5, false), "s"),
        },
        "mach": {
            "unit": "Mach",
            "axis": axis_json(&chart_axis(mach_lo, mach_hi, 5, false), "Mach"),
        },
        "transonic": transonic,
        "series": series,
    }))
}

pub fn compare(store: &Store, body: &Value) -> Result<Value, String> {
    let ids = body
        .get("profile_ids")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if ids.len() > 6 {
        return Err("compare at most six profiles".into());
    }
    let units = store.units()?;
    let angular = body
        .get("angular")
        .and_then(|v| v.as_str())
        .unwrap_or(units.angular.as_str());
    let mut rows = Vec::new();
    for id in ids {
        let id = id.as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let mut request = body.clone();
        request["profile_id"] = json!(id);
        let (shot, meta) = build_shot(store, &request)?;
        let bundle = store.profile_bundle(&id)?;
        let range_m = qty(body, "range", Dimension::Length, "m")?;
        if range_m <= 0.0 {
            return Err("range must be positive".into());
        }
        let solution = holdover_core::solve(&shot, range_m).map_err(|err| err.to_string())?;
        let point = solution
            .points
            .first()
            .ok_or("the solver returned no point")?;
        let bullet = &bundle["bullet"];
        let bc = bullet
            .get("bands")
            .and_then(|v| v.as_array())
            .and_then(|bands| bands.first())
            .and_then(|band| band.get("bc"))
            .and_then(|v| v.as_f64());
        rows.push(json!({
            "id": id,
            "label": meta.profile_label,
            "rifle": text_of(&bundle["rifle"], "label"),
            "barrel": qty_line(&bundle["rifle"], "barrel_length", &units.small),
            "twist": text_of(&bundle["rifle"], "twist_text"),
            "twist_hand": text_of(&bundle["rifle"], "twist_hand"),
            "sight_height": qty_line(&bundle["rifle"], "sight_height", &units.small),
            "weight": qty_line(&bundle["rifle"], "weight", &units.mass),
            "scope": text_of(&bundle["scope"], "label"),
            "focal_plane": pretty_phrase(&text_of(&bundle["scope"], "focal_plane")),
            "click": qty_line(&bundle["scope"], "click", &units.angle),
            "bullet": text_of(bullet, "label"),
            "drag": text_of(bullet, "drag_model"),
            "bc": bc,
            "bullet_weight": qty_line(bullet, "weight", &units.mass),
            "load": text_of(&bundle["load"], "label"),
            "mv": qty_line(&bundle["load"], "mv", &units.velocity),
            "zero": meta.zero_summary,
            "truing": meta.truing_label,
            "drop": format!("{} {}", shown(point.drop_m.abs(), &units.small), pretty_unit(&units.small)),
            "drop_m": point.drop_m,
            "elevation": hold_json(&point.elevation, angular),
            "windage": hold_json(&point.windage, angular),
            "velocity": format!("{} {}", shown(point.velocity_mps, &units.velocity), pretty_unit(&units.velocity)),
            "energy": format!("{} {}", shown(point.energy_j, &units.energy), pretty_unit(&units.energy)),
            "tof_s": point.tof_s,
            "mach": point.mach,
            "sg": point.sg,
            "warnings": solution.warnings.iter().map(warning_text).collect::<Vec<_>>(),
        }));
    }
    Ok(json!({
        "rows": rows,
        "range_unit": units.length,
        "angular": angular,
        "small_unit": units.small,
        "velocity_unit": units.velocity,
    }))
}

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn qty_line(value: &Value, name: &str, unit: &str) -> String {
    if name == "click" {
        if let Some(text) = value.get("click_text").and_then(|v| v.as_str()) {
            let text = text.trim();
            if !text.is_empty() {
                return pretty_phrase(text);
            }
        }
    }
    let si = value
        .get(format!("{name}_si"))
        .and_then(|v| v.as_f64())
        .or_else(|| value.get(format!("{name}_m")).and_then(|v| v.as_f64()))
        .or_else(|| value.get(format!("{name}_mps")).and_then(|v| v.as_f64()))
        .or_else(|| value.get(format!("{name}_kg")).and_then(|v| v.as_f64()))
        .or_else(|| {
            value
                .get("click_rad")
                .filter(|_| name == "click")
                .and_then(|v| v.as_f64())
        });
    if let Some(si) = si {
        let label = pretty_unit(unit);
        if mass_unit(unit) {
            return format!("{:.2} {label}", shown_value(si, unit));
        }
        return format!("{} {label}", shown(si, unit));
    }
    value
        .get(format!("{name}_text"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn hold_scale(angular: &str) -> (String, String, bool) {
    match angular.to_ascii_lowercase().as_str() {
        "moa" => ("MOA".into(), "MOA".into(), false),
        "iphy" => ("IPHY".into(), "IPHY".into(), false),
        "cm@100m" | "cm/100m" => ("cm@100m".into(), "cm/100 m".into(), false),
        "clicks" => ("mrad".into(), "mrad".into(), true),
        _ => ("mrad".into(), "mrad".into(), false),
    }
}

fn shown_value(si: f64, unit: &str) -> f64 {
    convert_from_si(si, unit).unwrap_or(si)
}

fn sample_extent(samples: &[Value], key: &str) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for sample in samples {
        if let Some(value) = sample.get(key).and_then(|value| value.as_f64()) {
            lo = lo.min(value);
            hi = hi.max(value);
        }
    }
    if lo.is_finite() && hi.is_finite() {
        (lo, hi)
    } else {
        (0.0, 1.0)
    }
}

fn extent(series: &[Value], key: &str) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for item in series {
        let Some(points) = item.get("points").and_then(|value| value.as_array()) else {
            continue;
        };
        for point in points {
            if let Some(value) = point.get(key).and_then(|value| value.as_f64()) {
                lo = lo.min(value);
                hi = hi.max(value);
            }
        }
    }
    if lo.is_finite() && hi.is_finite() {
        (lo, hi)
    } else {
        (0.0, 1.0)
    }
}

fn axis_json(axis: &ChartAxis, unit: &str) -> Value {
    json!({
        "unit": unit,
        "min": axis.min,
        "max": axis.max,
        "step": axis.step,
        "ticks": axis.ticks,
        "labels": axis.labels,
    })
}

struct Meta {
    profile_id: String,
    profile_label: String,
    zero_summary: String,
    zero_air: Value,
    differs: bool,
    station_note: Option<String>,
    truing_label: String,
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
    let truing_label = apply_truing(&mut shot, profile);
    let state = shot.atmosphere.state().map_err(|err| err.to_string())?;
    let units = store.units()?;
    let zero_summary = format!(
        "{} {} · {} {} · {} · {}%",
        shown(shot.zero_range_m.unwrap_or(0.0), &units.length),
        pretty_unit(&units.length),
        shown(zero_atmo.temperature_c, &units.temperature),
        pretty_unit(&units.temperature),
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
            truing_label,
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
        .unwrap_or("2 m/s, 4 m/s, 6 m/s");
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
        out.push((pretty_phrase(part), speed));
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

fn apply_truing(shot: &mut Shot, profile: &Value) -> String {
    let Some(active) = profile
        .get("truing")
        .and_then(|value| value.get("active"))
        .filter(|value| value.is_object())
    else {
        return "Not trued".into();
    };
    if let Some(mv) = active.get("mv_mps").and_then(|value| value.as_f64()) {
        shot.mv_mps = mv;
    }
    if let Some(scales) = active.get("cd_scales").and_then(|value| value.as_array()) {
        shot.cd_scales = scales.iter().filter_map(scale_of).collect();
    }
    if let Some(factor) = active
        .get("tracking_factor")
        .and_then(|value| value.as_f64())
    {
        if let Some(click) = shot.click.as_mut() {
            click.tracking_factor = factor;
        }
    }
    active
        .get("label")
        .and_then(|value| value.as_str())
        .unwrap_or("Trued")
        .to_string()
}

fn scale_of(value: &Value) -> Option<CdScale> {
    Some(CdScale {
        mach_min: value.get("mach_min")?.as_f64()?,
        mach_max: value.get("mach_max")?.as_f64()?,
        multiplier: value.get("multiplier")?.as_f64()?,
    })
}

fn lead_of(
    shot: &Shot,
    body: &Value,
    range_m: f64,
    point: &TrajectoryPoint,
    angular: &str,
    length_unit: &str,
) -> Result<Value, String> {
    if body.get("target_speed").is_none() {
        return Ok(Value::Null);
    }
    let speed = qty(body, "target_speed", Dimension::Velocity, "mps")?;
    if speed <= 0.0 {
        return Ok(Value::Null);
    }
    let direction_text = field_text(body, "target_direction");
    let direction = parse_wind_direction(if direction_text.trim().is_empty() {
        "3oc"
    } else {
        direction_text.as_str()
    })
    .map_err(|err| err.to_string())?;
    let reaction = body
        .get("reaction_s")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let solved = lead(shot, range_m, speed, direction, reaction).map_err(|err| err.to_string())?;
    let combined = combine_offsets(
        point.windage.rad.tan() * solved.impact_range_m,
        solved.lead_m + solved.reaction_lead_m,
        solved.impact_range_m,
        false,
        shot.click.as_ref(),
    );
    Ok(json!({
        "lead_m": solved.lead_m,
        "lead_display": shown(solved.lead_m.abs(), "cm"),
        "reaction_lead_m": solved.reaction_lead_m,
        "reaction_s": reaction,
        "reaction_note": "Reaction time is your assumption. It is added only when you enter one.",
        "tof_s": solved.tof_s,
        "impact_range_m": solved.impact_range_m,
        "impact_range_display": shown(solved.impact_range_m, length_unit),
        "length_unit": length_unit,
        "iterations": solved.iterations,
        "mrad": solved.lead.mrad,
        "moa": solved.lead.moa,
        "cm": solved.lead_m * 100.0,
        "lead": hold_json(&solved.with_reaction, angular),
        "flight_lead": hold_json(&solved.lead, angular),
        "combined": hold_json(&combined, angular),
        "outline": lead_outline(body),
        "target": lead_target_doc(body),
    }))
}

fn lead_outline(body: &Value) -> &'static str {
    match body.get("lead_target").and_then(|value| value.as_str()) {
        Some(id) if !id.is_empty() && holdover_core::target_by_id(id).is_some() => {
            "The selected target is drawn at the impact range. Placeholder outlines are marked."
        }
        _ => "A target outline is not drawn. This is the numeric lead.",
    }
}

fn lead_target_doc(body: &Value) -> Value {
    let Some(id) = body.get("lead_target").and_then(|value| value.as_str()) else {
        return Value::Null;
    };
    let Some(mut doc) = holdover_core::target_by_id(id) else {
        return Value::Null;
    };
    if body.get("facing").and_then(|value| value.as_str()) == Some("right") {
        doc.paths = holdover_core::mirror_paths(&doc.paths);
    }
    holdover_core::target_json(&doc)
}

fn condition_tables(
    shot: &Shot,
    body: &Value,
    start: f64,
    end: f64,
    step: f64,
) -> Result<Vec<(String, Solution)>, String> {
    let text = body
        .get("conditions")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if out.len() >= 4 {
            return Err("a card can have at most four extra temperatures".into());
        }
        let temperature = parse_quantity(part, Dimension::Temperature, Some("C"))
            .map_err(|err| err.to_string())?;
        let mut trial = shot.clone();
        trial.atmosphere.temperature_c = temperature;
        trial.wind = Wind::default();
        let solved = table(
            &trial,
            TableSpec {
                start_m: start,
                end_m: end,
                step_m: step,
            },
        )
        .map_err(|err| err.to_string())?;
        out.push((part.to_string(), solved));
    }
    Ok(out)
}

fn card_csv(
    rows: &[Value],
    brackets: &[(String, f64)],
    angular: &str,
    length_unit: &str,
) -> String {
    let length_label = pretty_unit(length_unit);
    let angular_label = pretty_unit(angular);
    let mut header = format!("Range ({length_label}),Elevation ({angular_label})");
    for (label, _) in brackets {
        header.push_str(&format!(",Wind {} ({angular_label})", pretty_phrase(label)));
    }
    header.push('\n');
    for row in rows {
        let range = row["range_display"].as_str().unwrap_or("");
        let elevation = card_number(&row["elevation"], angular);
        header.push_str(&format!("{range},{elevation}"));
        if let Some(winds) = row.get("winds").and_then(|value| value.as_array()) {
            for wind in winds {
                header.push(',');
                header.push_str(&card_number(&wind["windage"], angular));
            }
        }
        header.push('\n');
    }
    header
}

pub fn point_blank(store: &Store, body: &Value) -> Result<Value, String> {
    let (shot, meta) = build_shot(store, body)?;
    let diameter = qty(body, "zone", Dimension::Length, "m")?;
    let mode = body
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("optimal");
    let mut shot = shot;
    if mode == "fixed" && body.get("zero").is_some() {
        shot.zero_range_m = Some(qty(body, "zero", Dimension::Length, "m")?);
        shot.bore_angle_rad = None;
    }
    let solved = if mode == "fixed" {
        pbr_fixed(&shot, diameter).map_err(|err| err.to_string())?
    } else {
        pbr(&shot, diameter).map_err(|err| err.to_string())?
    };
    let units = store.units()?;
    let samples: Vec<Value> = solved
        .samples
        .iter()
        .map(|sample| {
            json!({
                "range_m": sample.range_m,
                "path_m": sample.path_m,
                "range": shown_value(sample.range_m, &units.length),
                "path": shown_value(sample.path_m, &units.small),
            })
        })
        .collect();
    let (range_lo, range_hi) = sample_extent(&samples, "range");
    let (path_lo, path_hi) = sample_extent(&samples, "path");
    let radius_display = shown_value(solved.radius_m, &units.small);
    let y_lo = path_lo.min(-radius_display);
    let y_hi = path_hi.max(radius_display);
    let optional_display = |value: Option<f64>| {
        value
            .map(|metres| shown(metres, &units.length))
            .unwrap_or_else(|| "—".into())
    };
    Ok(json!({
        "profile_id": meta.profile_id,
        "profile_label": meta.profile_label,
        "mode": if mode == "fixed" { "fixed" } else { "optimal" },
        "radius_m": solved.radius_m,
        "near_m": solved.near_m,
        "far_m": solved.far_m,
        "optimal_zero_m": solved.optimal_zero_m,
        "near_zero_m": solved.near_zero_m,
        "far_zero_m": solved.far_zero_m,
        "starts_inside": solved.starts_inside,
        "exits_above": solved.exits_above,
        "ordinate_m": solved.ordinate_m,
        "ordinate_range_m": solved.ordinate_range_m,
        "near_display": shown(solved.near_m, &units.length),
        "far_display": shown(solved.far_m, &units.length),
        "zero_display": shown(solved.optimal_zero_m, &units.length),
        "near_zero_display": optional_display(solved.near_zero_m),
        "far_zero_display": optional_display(solved.far_zero_m),
        "ordinate_display": shown(solved.ordinate_m, &units.small),
        "ordinate_range_display": shown(solved.ordinate_range_m, &units.length),
        "radius_display": radius_display,
        "x": axis_json(&chart_axis(range_lo, range_hi, 6, true), &units.length),
        "y": axis_json(&chart_axis(y_lo, y_hi, 5, true), &units.small),
        "length_unit": units.length,
        "small_unit": units.small,
        "zone_m": diameter,
        "samples": samples,
    }))
}

pub fn apply_zero(store: &Store, body: &Value) -> Result<Value, String> {
    let (shot, meta) = build_shot(store, body)?;
    let diameter = qty(body, "zone", Dimension::Length, "m")?;
    let solved = pbr(&shot, diameter).map_err(|err| err.to_string())?;
    let confirm = body
        .get("confirm")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let current = shot.zero_range_m.unwrap_or(0.0);
    let next = solved.optimal_zero_m;
    let text = format!("{next:.1} m");
    if !confirm {
        return Ok(json!({
            "applied": false,
            "current_m": current,
            "next_m": next,
            "next_text": text,
            "profile_label": meta.profile_label,
        }));
    }
    let stored = text.clone();
    store.mutate_body("profile", &meta.profile_id, |record| {
        record.insert("zero_range_m".into(), json!(next));
        record.insert("zero_range_text".into(), json!(stored));
        Ok(())
    })?;
    Ok(json!({
        "applied": true,
        "next_m": next,
        "next_text": text,
    }))
}

pub fn truing(store: &Store, body: &Value) -> Result<Value, String> {
    let confirm = body
        .get("confirm")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let action = body
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or("fit");
    let profile_id = body
        .get("profile_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if profile_id.is_empty() {
        return Err("choose a profile".into());
    }
    if action == "activate" || action == "revert" {
        return truing_history(store, profile_id, body, action, confirm);
    }
    let (shot, meta) = build_shot(store, body)?;
    let kind = body
        .get("kind")
        .and_then(|value| value.as_str())
        .unwrap_or("mv");
    let (summary, patch, detail) = match kind {
        "mv" => fit_mv(&shot, body)?,
        "drag" => fit_drag(&shot, body)?,
        "tracking" => fit_tracking(body)?,
        _ => return Err("truing kind must be mv, drag, or tracking".into()),
    };
    let profile = store.profile_bundle(profile_id)?["profile"].clone();
    let before = active_snapshot(&profile);
    let after = merge_snapshot(&before, &patch);
    if !confirm {
        return Ok(json!({
            "applied": false,
            "kind": kind,
            "summary": summary,
            "before": before,
            "after": after,
            "detail": detail,
            "profile_id": meta.profile_id,
        }));
    }
    let entry = json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "kind": kind,
        "created_at": crate::store::now_iso(),
        "summary": summary.clone(),
        "before": before.clone(),
        "after": after.clone(),
        "detail": detail.clone(),
    });
    store.mutate_body("profile", profile_id, |record| {
        let mut truing = record
            .get("truing")
            .cloned()
            .unwrap_or_else(|| json!({ "history": [] }));
        if !truing
            .get("history")
            .map(|value| value.is_array())
            .unwrap_or(false)
        {
            truing["history"] = json!([]);
        }
        truing["history"]
            .as_array_mut()
            .expect("history array")
            .push(entry.clone());
        truing["active"] = after.clone();
        record.insert("truing".into(), truing);
        Ok(())
    })?;
    Ok(json!({
        "applied": true,
        "kind": kind,
        "summary": summary,
        "before": before,
        "after": after,
        "detail": detail,
        "profile_id": meta.profile_id,
    }))
}

fn truing_history(
    store: &Store,
    profile_id: &str,
    body: &Value,
    action: &str,
    confirm: bool,
) -> Result<Value, String> {
    let history_id = body
        .get("history_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let profile = store.profile_bundle(profile_id)?["profile"].clone();
    let entry = profile
        .get("truing")
        .and_then(|value| value.get("history"))
        .and_then(|value| value.as_array())
        .and_then(|rows| rows.iter().find(|row| row["id"] == history_id))
        .cloned()
        .ok_or_else(|| "that truing entry was not found".to_string())?;
    let snapshot = if action == "revert" {
        entry["before"].clone()
    } else {
        entry["after"].clone()
    };
    if !confirm {
        return Ok(json!({
            "applied": false,
            "action": action,
            "summary": entry["summary"],
            "after": snapshot,
        }));
    }
    store.mutate_body("profile", profile_id, |record| {
        let mut truing = record
            .get("truing")
            .cloned()
            .unwrap_or_else(|| json!({ "history": [] }));
        truing["active"] = snapshot.clone();
        record.insert("truing".into(), truing);
        Ok(())
    })?;
    Ok(json!({
        "applied": true,
        "action": action,
        "summary": entry["summary"],
        "after": snapshot,
    }))
}

fn fit_mv(shot: &Shot, body: &Value) -> Result<(String, Value, Value), String> {
    let range = qty(body, "observe_range", Dimension::Length, "m")?;
    let elevation = qty(body, "observe_elevation", Dimension::Angle, "mrad")?;
    let fit = true_mv(shot, range, elevation).map_err(|err| err.to_string())?;
    let summary = format!(
        "MV {:.1} → {:.1} m/s ({:+.1}%)",
        fit.previous_mv_mps,
        fit.mv_mps,
        fit.change_fraction * 100.0
    );
    let detail = json!({
        "guidance": MV_GUIDANCE,
        "large_change": fit.large_change,
        "before_transonic": fit.before_transonic,
        "mach": fit.mach,
        "residual_mrad": fit.residual_rad * 1000.0,
        "change_fraction": fit.change_fraction,
        "shots": body.get("shots").cloned().unwrap_or(Value::Null),
        "group_text": body.get("group_text").cloned().unwrap_or(Value::Null),
    });
    Ok((summary, json!({ "mv_mps": fit.mv_mps }), detail))
}

fn fit_drag(shot: &Shot, body: &Value) -> Result<(String, Value, Value), String> {
    let rows = body
        .get("observations")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "drag truing needs an observation".to_string())?;
    let mut observations = Vec::new();
    let mut recorded = Vec::new();
    for row in rows {
        let range = qty(row, "range", Dimension::Length, "m")?;
        let elevation = qty(row, "elevation", Dimension::Angle, "mrad")?;
        observations.push((range, elevation));
        recorded.push(json!({
            "range_m": range,
            "elevation_rad": elevation,
            "shots": row.get("shots").cloned().unwrap_or(Value::Null),
            "group_text": row.get("group_text").cloned().unwrap_or(Value::Null),
        }));
    }
    let mode = if body
        .get("piecewise")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        DragMode::MachBands
    } else {
        DragMode::Multiplier
    };
    let fit = true_drag(shot, &observations, mode).map_err(|err| err.to_string())?;
    let summary = format!(
        "BC ×{:.3} (Cd ×{:.3})",
        fit.bc_multiplier, fit.cd_multiplier
    );
    let pieces: Vec<Value> = fit
        .pieces
        .iter()
        .map(|piece| {
            json!({
                "mach_min": piece.mach_min,
                "mach_max": piece.mach_max,
                "multiplier": piece.multiplier,
            })
        })
        .collect();
    let detail = json!({
        "guidance": DRAG_GUIDANCE,
        "large_change": fit.large_change,
        "change_fraction": fit.change_fraction,
        "bc_multiplier": fit.bc_multiplier,
        "cd_multiplier": fit.cd_multiplier,
        "observations": recorded,
    });
    Ok((summary, json!({ "cd_scales": pieces }), detail))
}

fn fit_tracking(body: &Value) -> Result<(String, Value, Value), String> {
    let advertised = qty(body, "advertised", Dimension::Angle, "mrad")?;
    let observed = qty(body, "observed", Dimension::Angle, "mrad")?;
    let factor = true_tracking(advertised, observed).map_err(|err| err.to_string())?;
    let summary = format!("Tracking {factor:.3}");
    Ok((
        summary,
        json!({ "tracking_factor": factor }),
        json!({
            "guidance": TRACKING_GUIDANCE,
            "large_change": (factor - 1.0).abs() > 0.03,
            "tracking_factor": factor,
        }),
    ))
}

fn active_snapshot(profile: &Value) -> Value {
    profile
        .get("truing")
        .and_then(|value| value.get("active"))
        .cloned()
        .filter(|value| value.is_object())
        .unwrap_or_else(|| {
            json!({
                "mv_mps": null,
                "cd_scales": [],
                "tracking_factor": null,
                "label": "Not trued",
            })
        })
}

fn merge_snapshot(before: &Value, patch: &Value) -> Value {
    let mut after = before.clone();
    if let (Some(object), Some(patch)) = (after.as_object_mut(), patch.as_object()) {
        for (key, value) in patch {
            object.insert(key.clone(), value.clone());
        }
        object.insert("label".into(), json!(snapshot_label(object)));
    }
    after
}

fn snapshot_label(object: &serde_json::Map<String, Value>) -> String {
    let mut parts = Vec::new();
    if let Some(mv) = object.get("mv_mps").and_then(|value| value.as_f64()) {
        parts.push(format!("MV {mv:.0} m/s"));
    }
    if let Some(multiplier) = object
        .get("cd_scales")
        .and_then(|value| value.as_array())
        .and_then(|scales| scales.first())
        .and_then(|scale| scale.get("multiplier"))
        .and_then(|value| value.as_f64())
    {
        parts.push(format!("Cd ×{multiplier:.3}"));
    }
    if let Some(factor) = object
        .get("tracking_factor")
        .and_then(|value| value.as_f64())
    {
        parts.push(format!("tracking {factor:.3}"));
    }
    if parts.is_empty() {
        "Not trued".into()
    } else {
        parts.join(" · ")
    }
}

pub fn chrono_session(store: &Store, body: &Value) -> Result<Value, String> {
    let op = body
        .get("op")
        .and_then(|value| value.as_str())
        .unwrap_or("stats");
    let confirm = body
        .get("confirm")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    match op {
        "stats" | "save" => chrono_stats_op(store, body, op == "save", confirm),
        "set_mv" => set_mv_op(store, body, confirm),
        "fit" | "apply_powder" => powder_op(store, body, op == "apply_powder", confirm),
        _ => Err("unknown chronograph action".into()),
    }
}

fn chrono_stats_op(
    store: &Store,
    body: &Value,
    save: bool,
    confirm: bool,
) -> Result<Value, String> {
    let text = body
        .get("shots")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let speeds = shot_speeds(text)?;
    let stats = chrono_stats(&speeds).map_err(|err| err.to_string())?;
    let units = store.units()?;
    let temperature = body
        .get("temperature")
        .and_then(|_| qty(body, "temperature", Dimension::Temperature, "C").ok());
    let mut saved = false;
    if save {
        if !confirm {
            return Ok(chrono_json(&stats, &units.velocity, temperature, false));
        }
        let load_id = body
            .get("load_id")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if load_id.is_empty() {
            return Err("choose a load".into());
        }
        let label = body
            .get("label")
            .and_then(|value| value.as_str())
            .filter(|text| !text.is_empty())
            .unwrap_or("Chronograph")
            .to_string();
        let record = json!({
            "label": label,
            "load_id": load_id,
            "shots_text": text,
            "shots_mps": speeds,
            "mean_mps": stats.mean_mps,
            "sd_mps": stats.sd_mps,
            "es_mps": stats.es_mps,
            "min_mps": stats.min_mps,
            "max_mps": stats.max_mps,
            "outlier_indexes": stats.outliers,
            "temperature_c": temperature,
            "temperature_text": body.get("temperature").and_then(|v| v.get("text")).cloned().unwrap_or(Value::Null),
        });
        store.save_chrono_record(&record)?;
        saved = true;
    }
    let mut value = chrono_json(&stats, &units.velocity, temperature, saved);
    value["saved"] = json!(saved);
    Ok(value)
}

fn chrono_json(
    stats: &holdover_core::Chrono,
    unit: &str,
    temperature: Option<f64>,
    saved: bool,
) -> Value {
    json!({
        "count": stats.count,
        "mean_mps": stats.mean_mps,
        "sd_mps": stats.sd_mps,
        "es_mps": stats.es_mps,
        "min_mps": stats.min_mps,
        "max_mps": stats.max_mps,
        "outliers": stats.outliers,
        "mean_display": shown(stats.mean_mps, unit),
        "sd_display": shown(stats.sd_mps, unit),
        "es_display": shown(stats.es_mps, unit),
        "min_display": shown(stats.min_mps, unit),
        "max_display": shown(stats.max_mps, unit),
        "unit": unit,
        "temperature_c": temperature,
        "saved": saved,
        "outlier_note": "Flagged shots stay in the mean and the standard deviation.",
    })
}

fn set_mv_op(store: &Store, body: &Value, confirm: bool) -> Result<Value, String> {
    let load_id = body
        .get("load_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let mean = body
        .get("mean_mps")
        .and_then(|value| value.as_f64())
        .ok_or_else(|| "a mean speed is required".to_string())?;
    let temperature = body.get("temperature_c").and_then(|value| value.as_f64());
    if !confirm {
        return Ok(json!({
            "applied": false,
            "load_id": load_id,
            "mean_mps": mean,
            "temperature_c": temperature,
        }));
    }
    if load_id.is_empty() {
        return Err("choose a load".into());
    }
    let text = format!("{mean:.1} mps");
    store.mutate_body("load", load_id, |record| {
        record.insert("mv_mps".into(), json!(mean));
        record.insert("mv_text".into(), json!(text));
        if let Some(temperature) = temperature {
            record.insert("mv_temperature_si".into(), json!(temperature));
            record.insert(
                "mv_temperature_text".into(),
                json!(format!("{temperature:.1} C")),
            );
        }
        Ok(())
    })?;
    Ok(json!({ "applied": true, "mean_mps": mean }))
}

fn powder_op(store: &Store, body: &Value, apply: bool, confirm: bool) -> Result<Value, String> {
    let load_id = body
        .get("load_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if load_id.is_empty() {
        return Err("choose a load".into());
    }
    let sessions = store.live_bodies("chrono")?;
    let mut points = Vec::new();
    for session in sessions {
        if session.get("load_id").and_then(|value| value.as_str()) != Some(load_id) {
            continue;
        }
        if let (Some(temperature), Some(mean)) = (
            session
                .get("temperature_c")
                .and_then(|value| value.as_f64()),
            session.get("mean_mps").and_then(|value| value.as_f64()),
        ) {
            points.push((temperature, mean));
        }
    }
    let fit = powder_coefficient(&points).map_err(|err| err.to_string())?;
    if apply && confirm {
        let text = format!("{:.3} mps", fit.slope_mps_per_c);
        store.mutate_body("load", load_id, |record| {
            record.insert("sensitivity_si".into(), json!(fit.slope_mps_per_c));
            record.insert("sensitivity_text".into(), json!(text));
            Ok(())
        })?;
    }
    Ok(json!({
        "applied": apply && confirm,
        "slope_mps_per_c": fit.slope_mps_per_c,
        "intercept_mps": fit.intercept_mps,
        "r_squared": fit.r_squared,
        "n": fit.n,
    }))
}

fn shot_speeds(text: &str) -> Result<Vec<f64>, String> {
    let mut speeds = Vec::new();
    for part in text.split([',', '\n', ';', '\t']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        speeds.push(
            parse_quantity(part, Dimension::Velocity, Some("mps"))
                .map_err(|err| err.to_string())?,
        );
    }
    if speeds.is_empty() {
        return Err("paste at least one shot speed".into());
    }
    Ok(speeds)
}

pub fn recoil_calc(store: &Store, body: &Value) -> Result<Value, String> {
    let ejecta = if body.get("ejecta").is_some() {
        qty(body, "ejecta", Dimension::Velocity, "mps")?
    } else {
        holdover_core::DEFAULT_EJECTA_MPS
    };
    let reduction = body
        .get("reduction")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let result = recoil(RecoilInput {
        rifle_mass_kg: qty(body, "rifle", Dimension::Mass, "kg")?,
        bullet_mass_kg: qty(body, "bullet", Dimension::Mass, "g")?,
        charge_mass_kg: qty(body, "charge", Dimension::Mass, "g")?,
        mv_mps: qty(body, "mv", Dimension::Velocity, "mps")?,
        ejecta_mps: ejecta,
        reduction_fraction: reduction,
    })
    .map_err(|err| err.to_string())?;
    let units = store.units()?;
    Ok(json!({
        "velocity_mps": result.velocity_mps,
        "energy_j": result.energy_j,
        "velocity_fps": result.velocity_fps,
        "energy_ft_lbf": result.energy_ft_lbf,
        "velocity_display": shown(result.velocity_mps, &units.velocity),
        "energy_display": shown(result.energy_j, &units.energy),
        "velocity_unit": units.velocity,
        "energy_unit": units.energy,
        "convention": result.convention,
        "reduction_is_estimate": result.reduction_is_estimate,
        "ejecta_mps": result.ejecta_mps,
    }))
}

pub fn impact(store: &Store, body: &Value) -> Result<Value, String> {
    let ids = body
        .get("profile_ids")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if ids.is_empty() || ids.len() > 6 {
        return Err("choose one profile and at most five overlays".into());
    }
    let range_m = qty(body, "range", Dimension::Length, "m")?;
    let target = body
        .get("target")
        .and_then(|value| value.as_str())
        .unwrap_or("moose");
    let (rings_cm, placeholder, note, geometry) = if target == "custom" {
        let diameter_m = qty(body, "zone", Dimension::Length, "m")?;
        (
            vec![diameter_m * 100.0],
            false,
            "Custom diameter. This is not an official target.".to_string(),
            Value::Null,
        )
    } else {
        let mut doc = holdover_core::target_by_id(target)
            .ok_or_else(|| format!("unknown target {target}"))?;
        if body.get("facing").and_then(|value| value.as_str()) == Some("right") {
            doc.paths = holdover_core::mirror_paths(&doc.paths);
        }
        let placeholder = doc.flags.iter().any(|flag| flag.kind != "resolved");
        let rings = doc.rings.iter().map(|ring| ring.diameter_cm).collect();
        let note = if doc.flags.is_empty() {
            doc.subtitle.clone()
        } else {
            doc.flags
                .iter()
                .map(|flag| flag.detail.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let geometry = holdover_core::target_json(&doc);
        (rings, placeholder, note, geometry)
    };
    let mv_sd = if body.get("mv_sd").is_some() {
        qty(body, "mv_sd", Dimension::Velocity, "mps").unwrap_or(0.0)
    } else {
        0.0
    };
    let range_error = if body.get("range_error").is_some() {
        qty(body, "range_error", Dimension::Length, "m").unwrap_or(1.0)
    } else {
        1.0
    };
    let brackets = wind_brackets(body).unwrap_or_default();
    let mut pois = Vec::new();
    for id in &ids {
        let id = id.as_str().unwrap_or("");
        let mut request = body.clone();
        request["profile_id"] = json!(id);
        let (shot, meta) = build_shot(store, &request)?;
        let solved = holdover_core::solve(&shot, range_m).map_err(|err| err.to_string())?;
        let point = solved
            .points
            .first()
            .ok_or("the solver returned no point")?;
        let (mv_low, mv_high) = if mv_sd > 0.0 {
            vertical_mv_spread_m(&shot, range_m, mv_sd).map_err(|err| err.to_string())?
        } else {
            (point.path_m, point.path_m)
        };
        let path_at = |range: f64| -> f64 {
            holdover_core::solve(&shot, range.max(1.0))
                .ok()
                .and_then(|solution| solution.points.first().map(|item| item.path_m))
                .unwrap_or(point.path_m)
        };
        let mut from_left = shot.clone();
        from_left.wind = Wind {
            speed_mps: 1.0,
            from_clock_rad: std::f64::consts::FRAC_PI_2 * 3.0,
        };
        let mut from_right = shot.clone();
        from_right.wind = Wind {
            speed_mps: 1.0,
            from_clock_rad: std::f64::consts::FRAC_PI_2,
        };
        let wind_left = holdover_core::solve(&from_left, range_m)
            .ok()
            .and_then(|solution| solution.points.first().map(|item| item.windage_m))
            .unwrap_or(0.0);
        let wind_right = holdover_core::solve(&from_right, range_m)
            .ok()
            .and_then(|solution| solution.points.first().map(|item| item.windage_m))
            .unwrap_or(0.0);
        let mut bracket_points = Vec::new();
        for (label, speed) in &brackets {
            let mut trial = shot.clone();
            trial.wind = Wind {
                speed_mps: *speed,
                from_clock_rad: std::f64::consts::FRAC_PI_2 * 3.0,
            };
            if let Some(windage) = holdover_core::solve(&trial, range_m)
                .ok()
                .and_then(|solution| solution.points.first().map(|item| item.windage_m))
            {
                bracket_points.push(json!({
                    "label": label,
                    "windage_cm": windage * 100.0,
                }));
            }
        }
        pois.push(json!({
            "id": id,
            "label": meta.profile_label,
            "path_cm": point.path_m * 100.0,
            "windage_cm": point.windage_m * 100.0,
            "mv_low_cm": mv_low * 100.0,
            "mv_high_cm": mv_high * 100.0,
            "range_low_cm": path_at(range_m - range_error) * 100.0,
            "range_high_cm": path_at(range_m + range_error) * 100.0,
            "wind_left_cm": wind_left * 100.0,
            "wind_right_cm": wind_right * 100.0,
            "brackets": bracket_points,
        }));
    }
    Ok(json!({
        "range_m": range_m,
        "target": target,
        "rings_cm": rings_cm,
        "placeholder": placeholder,
        "note": note,
        "geometry": geometry,
        "range_error_m": range_error,
        "mv_sd_mps": mv_sd,
        "pois": pois,
        "measured": group_marks(store, body)?,
    }))
}

fn group_marks(store: &Store, body: &Value) -> Result<Vec<Value>, String> {
    let Some(id) = body
        .get("group_id")
        .and_then(|value| value.as_str())
        .filter(|id| !id.is_empty())
    else {
        return Ok(Vec::new());
    };
    let record = store
        .body("group", id)
        .map_err(|_| "that group was not found".to_string())?;
    let holes = record
        .pointer("/analysis/holes")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let mut marks = Vec::new();
    for hole in holes {
        let Some(relative) = hole.get("relative_mm").and_then(|value| value.as_array()) else {
            continue;
        };
        let x_cm = relative
            .first()
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0)
            / 10.0;
        let y_cm = relative
            .get(1)
            .and_then(|value| value.as_f64())
            .unwrap_or(0.0)
            / 10.0;
        let count = hole
            .get("count")
            .and_then(|value| value.as_u64())
            .unwrap_or(1);
        marks.push(json!({ "x_cm": x_cm, "y_cm": y_cm, "count": count }));
    }
    Ok(marks)
}

pub fn reticle_scene(store: &Store, body: &Value) -> Result<Value, String> {
    use holdover_core::{
        generic_by_id, marks_true_mrad, parse_reticle, sfp_warning, subtension_factor, to_mrad,
        Eyepiece, FocalPlane, ReticleUnit,
    };
    let mut reticle = if body.get("reticle").is_some() {
        parse_reticle(&body["reticle"]).map_err(|err| err.to_string())?
    } else {
        let id = body
            .get("reticle_id")
            .and_then(|value| value.as_str())
            .unwrap_or("generic-mil-dot");
        if let Some(found) = store
            .live_bodies("reticle")
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row["id"].as_str() == Some(id)))
        {
            parse_reticle(&found).map_err(|err| err.to_string())?
        } else {
            generic_by_id(id).ok_or_else(|| format!("unknown reticle {id}"))?
        }
    };
    if let Some(focal) = body.get("focal_plane").and_then(|value| value.as_str()) {
        reticle.focal_plane = if focal == "sfp" {
            FocalPlane::Sfp
        } else {
            FocalPlane::Ffp
        };
    }
    if let Some(calibrated) = body
        .get("calibrated_magnification")
        .and_then(|value| value.as_f64())
    {
        reticle.calibrated_magnification = calibrated;
    }
    let magnification = body
        .get("magnification")
        .and_then(|value| value.as_f64())
        .unwrap_or(10.0);
    let fov = body
        .get("apparent_fov_deg")
        .and_then(|value| value.as_f64())
        .unwrap_or(holdover_core::DEFAULT_APPARENT_FOV_DEG);
    let viewport = body
        .get("viewport_px")
        .and_then(|value| value.as_f64())
        .unwrap_or(1000.0);
    let factor = subtension_factor(
        reticle.focal_plane,
        reticle.calibrated_magnification,
        magnification,
    )
    .map_err(|err| err.to_string())?;
    let marks = marks_true_mrad(&reticle, magnification).map_err(|err| err.to_string())?;
    let eye = Eyepiece {
        magnification,
        apparent_fov_deg: fov,
        viewport_px: viewport,
    };
    let px_per_mrad = eye.px_per_mrad().map_err(|err| err.to_string())?;
    let range_m = if body.get("range").is_some() {
        qty(body, "range", Dimension::Length, "m")?
    } else {
        body.get("range_m")
            .and_then(|value| value.as_f64())
            .unwrap_or(100.0)
    };
    let cm = holdover_core::cm_per_mrad(range_m).map_err(|err| err.to_string())?;
    let elevation = body
        .get("elevation_mrad")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let windage = body
        .get("windage_mrad")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let lead_mrad = body
        .get("lead_mrad")
        .and_then(|value| value.as_f64())
        .unwrap_or(0.0);
    let target_id = body
        .get("target_id")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let geometry = if target_id.is_empty() {
        Value::Null
    } else if let Some(mut doc) = holdover_core::target_by_id(target_id) {
        if body.get("facing").and_then(|value| value.as_str()) == Some("right") {
            doc.paths = holdover_core::mirror_paths(&doc.paths);
        }
        holdover_core::target_json(&doc)
    } else if let Ok(saved) = store.saved_target(target_id) {
        saved
    } else {
        return Err(format!("unknown target {target_id}"));
    };
    let unit = match reticle.unit {
        ReticleUnit::Mrad => "mrad",
        ReticleUnit::Moa => "MOA",
    };
    Ok(json!({
        "px_per_mrad": px_per_mrad,
        "viewport_px": viewport,
        "sfp_factor": factor,
        "sfp_warning": sfp_warning(&reticle, magnification),
        "focal_plane": match reticle.focal_plane {
            FocalPlane::Ffp => "ffp",
            FocalPlane::Sfp => "sfp",
        },
        "unit": unit,
        "calibrated_magnification": reticle.calibrated_magnification,
        "magnification": magnification,
        "nominal_one_true_mrad": to_mrad(1.0, reticle.unit) * factor,
        "elements": marks,
        "target": geometry,
        "cm_per_mrad": cm,
        "range_m": range_m,
        "aim_mrad": {
            "x": -(windage + lead_mrad),
            "y": -elevation,
        },
        "elevation_mrad": elevation,
        "windage_mrad": windage,
        "lead_mrad": lead_mrad,
    }))
}

pub fn target_pdf_bytes(
    store: &Store,
    body: &Value,
) -> Result<holdover_core::PrintedTarget, String> {
    let id = body
        .get("target_id")
        .and_then(|value| value.as_str())
        .unwrap_or("sjf-algfigur");
    let mut doc = if let Some(found) = holdover_core::target_by_id(id) {
        found
    } else {
        let saved = store.saved_target(id)?;
        serde_json::from_value(saved).map_err(|err| err.to_string())?
    };
    if body
        .get("geometry_override")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        if let Some(rows) = body.get("paths").and_then(|value| value.as_array()) {
            doc.paths = rows.iter().filter_map(pdf_path).collect();
        }
        if let Some(rings) = body.get("rings_cm").and_then(|value| value.as_array()) {
            doc.rings = rings
                .iter()
                .filter_map(|value| value.as_f64())
                .filter(|diameter| *diameter > 0.0)
                .map(|diameter| holdover_core::RingDoc {
                    diameter_cm: diameter,
                    label_primary: String::new(),
                    label_alt: String::new(),
                    points_primary: None,
                    points_alt: None,
                    verified: true,
                })
                .collect();
        }
    } else {
        if body.get("facing").and_then(|value| value.as_str()) == Some("right") {
            doc.paths = holdover_core::mirror_paths(&doc.paths);
        }
        let show_silhouette = body
            .get("show_silhouette")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !show_silhouette {
            doc.paths.retain(|path| path.role != "silhouette");
        }
    }
    let page_id = body
        .get("page")
        .and_then(|value| value.as_str())
        .unwrap_or("a4");
    let page = card_page(page_id).ok_or_else(|| format!("unknown page {page_id}"))?;
    let notes = body
        .get("notes")
        .and_then(|value| value.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.as_str().map(|text| text.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let (width, height) = holdover_core::bounds_cm(&doc);
    let paths = doc
        .paths
        .iter()
        .map(|path| holdover_core::TargetPath {
            points: path
                .points
                .iter()
                .map(|point| (point[0], point[1]))
                .collect(),
            closed: path.closed,
            dashed: path.placeholder,
        })
        .collect();
    let sheet = holdover_core::TargetPrint {
        page_width_mm: page.width_mm,
        page_height_mm: page.height_mm,
        title: doc.name.clone(),
        notes,
        rings_cm: doc.rings.iter().map(|ring| ring.diameter_cm).collect(),
        paths,
        cell_cm: doc.cell_cm,
        span_cm: (width, height),
    };
    Ok(holdover_core::render_target_pdf(&sheet))
}

fn pdf_path(value: &Value) -> Option<holdover_core::PathDoc> {
    let points = value.get("points")?.as_array()?;
    let parsed = points
        .iter()
        .filter_map(|point| {
            let pair = point.as_array()?;
            Some([pair.first()?.as_f64()?, pair.get(1)?.as_f64()?])
        })
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        return None;
    }
    Some(holdover_core::PathDoc {
        role: "print".into(),
        points: parsed,
        placeholder: value
            .get("dashed")
            .and_then(|flag| flag.as_bool())
            .unwrap_or(true),
        for_scoring: false,
        closed: value
            .get("closed")
            .and_then(|flag| flag.as_bool())
            .unwrap_or(true),
    })
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

fn pretty_unit(unit: &str) -> String {
    match unit {
        "mps" => "m/s".into(),
        "C" => "°C".into(),
        "F" => "°F".into(),
        other => other.to_string(),
    }
}

fn mass_unit(unit: &str) -> bool {
    matches!(unit, "g" | "gr" | "kg" | "lb" | "oz")
}

fn pretty_phrase(text: &str) -> String {
    text.split_whitespace()
        .map(pretty_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn pretty_token(word: &str) -> String {
    let end = word.trim_end_matches([',', ';', '.']);
    let punct = &word[end.len()..];
    let body = match end {
        "mps" => "m/s",
        "C" => "°C",
        "F" => "°F",
        "ffp" => "FFP",
        "sfp" => "SFP",
        other => other,
    };
    format!("{body}{punct}")
}

fn edition_label(body: &Value) -> String {
    if let Some(text) = body
        .get("edition_date")
        .and_then(|v| v.as_str())
        .map(str::trim)
    {
        if !text.is_empty() && text.len() <= 40 && !text.chars().any(|ch| ch == '\n' || ch == '\r')
        {
            return text.to_string();
        }
    }
    crate::store::now_iso().chars().take(10).collect()
}

fn card_number(hold: &Value, angular: &str) -> String {
    let key = match angular.to_ascii_lowercase().as_str() {
        "moa" => "moa",
        "iphy" => "iphy",
        "cm@100m" | "cm/100m" => "cm_per_100m",
        "clicks" => "clicks",
        _ => "mrad",
    };
    if key == "clicks" {
        return hold
            .get("clicks")
            .and_then(|value| value.as_i64())
            .map(|count| count.abs().to_string())
            .unwrap_or_else(|| "—".into());
    }
    let digits = if key == "cm_per_100m" { 1 } else { 2 };
    match hold.get(key).and_then(|value| value.as_f64()) {
        Some(number) => format!("{:.*}", digits, number.abs()),
        None => "—".into(),
    }
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

    #[test]
    fn chart_axes_follow_the_preset_and_include_zero() {
        let store = store();
        let id = store.bootstrap().unwrap()["settings"]["active_profile_id"]
            .as_str()
            .unwrap()
            .to_string();
        let plotted = chart(
            &store,
            &json!({
                "profile_ids": [id],
                "from": {"text": "0 m", "unit": "m"},
                "to": {"text": "300 m", "unit": "m"},
                "step": {"text": "100 m", "unit": "m"},
                "wind_speed": {"text": "3 mps", "unit": "mps"},
                "wind_direction": "9oc",
                "inclination": {"text": "0 deg", "unit": "deg"},
                "current_air": {
                    "temperature": {"text": "15 C", "unit": "C"},
                    "pressure": {"text": "1013.25 hPa", "unit": "hPa"},
                    "pressure_kind": "station",
                    "humidity": "0%",
                    "altitude": {"text": "0 m", "unit": "m"}
                },
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false},
                "angular": "MOA"
            }),
        )
        .unwrap();
        assert_eq!(plotted["x"]["unit"], "m");
        assert_eq!(plotted["drop"]["unit"], "cm");
        assert_eq!(plotted["wind"]["unit"], "MOA");
        assert_eq!(plotted["x"]["step"].as_f64().unwrap(), 50.0);
        let x_labels = plotted["x"]["labels"].as_array().unwrap();
        assert!(x_labels.iter().any(|label| label == "0"));
        assert!(x_labels.iter().any(|label| label == "300"));
        assert!(plotted["drop"]["axis"]["labels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|label| label == "0"));
        assert!(plotted["wind"]["axis"]["labels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|label| label == "0"));
        let points = plotted["series"][0]["points"].as_array().unwrap();
        assert!(points.len() >= 3);
        assert!(!plotted["series"][0]["label"].as_str().unwrap().is_empty());
        let last_wind = points.last().unwrap()["windage"].as_f64().unwrap();
        assert!(last_wind.abs() > 0.0, "{last_wind}");
    }

    fn air() -> Value {
        json!({
            "temperature": {"text": "15 C", "unit": "C"},
            "pressure": {"text": "1013.25 hPa", "unit": "hPa"},
            "pressure_kind": "station",
            "humidity": "0%",
            "altitude": {"text": "0 m", "unit": "m"}
        })
    }

    #[test]
    fn card_scale_bar_truing_confirm_and_chrono_keep_every_shot() {
        let store = store();
        let id = store.bootstrap().unwrap()["settings"]["active_profile_id"]
            .as_str()
            .unwrap()
            .to_string();
        let card = range_card(
            &store,
            &json!({
                "profile_id": id,
                "from": {"text": "100 m", "unit": "m"},
                "to": {"text": "100 m", "unit": "m"},
                "step": {"text": "100 m", "unit": "m"},
                "brackets": "2 mps",
                "page": "a4",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        assert_eq!(card["scale_bar_mm"], 100.0);
        assert_eq!(card["scale_bar_label"], "10 cm");
        assert_eq!(card["page"]["width_mm"], 210.0);
        let print_css = card["print_css"].as_str().unwrap();
        assert!(
            print_css.contains("@page { size: 210mm 297mm; margin: 10mm; }"),
            "{print_css}"
        );
        assert!(
            print_css.contains("width: 100mm !important;"),
            "{print_css}"
        );
        assert!(!print_css.contains("px"), "{print_css}");
        assert!(card["csv"].as_str().unwrap().contains("Range (m)"));
        let compact = range_card(
            &store,
            &json!({
                "profile_id": id,
                "from": {"text": "100 m", "unit": "m"},
                "to": {"text": "100 m", "unit": "m"},
                "step": {"text": "100 m", "unit": "m"},
                "page": "compact",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false}
            }),
        )
        .unwrap();
        assert_eq!(compact["scale_bar_mm"], 50.0);

        let solved = solve(
            &store,
            &json!({
                "profile_id": id,
                "range": {"text": "400 m", "unit": "m"},
                "wind_speed": {"text": "0 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "inclination": {"text": "0 deg", "unit": "deg"},
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false},
                "angular": "mrad"
            }),
        )
        .unwrap();
        let observed = solved["elevation"]["mrad"].as_f64().unwrap() + 0.4;
        let request = json!({
            "profile_id": id,
            "kind": "mv",
            "observe_range": {"text": "400 m", "unit": "m"},
            "observe_elevation": {"text": format!("{observed} mrad"), "unit": "mrad"},
            "shots": 5,
            "group_text": "6 cm",
            "current_air": air(),
            "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
        });
        let mut preview = request.clone();
        preview["confirm"] = json!(false);
        let preview = truing(&store, &preview).unwrap();
        assert_eq!(preview["applied"], false);
        assert!(preview["detail"]["guidance"]
            .as_str()
            .unwrap()
            .contains("transonic"));
        let unchanged = store.profile_bundle(&id).unwrap()["load"]["mv_mps"]
            .as_f64()
            .unwrap();
        assert!((unchanged - 800.0).abs() < 1e-6);
        let mut apply = request.clone();
        apply["confirm"] = json!(true);
        let applied = truing(&store, &apply).unwrap();
        assert_eq!(applied["applied"], true);
        let again = solve(
            &store,
            &json!({
                "profile_id": id,
                "range": {"text": "400 m", "unit": "m"},
                "wind_speed": {"text": "0 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "inclination": {"text": "0 deg", "unit": "deg"},
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        assert!(
            (again["elevation"]["mrad"].as_f64().unwrap() - observed).abs() < 0.02,
            "{}",
            again["elevation"]["mrad"]
        );
        assert_ne!(again["truing"], "Not trued");

        let load_id = store.profile_bundle(&id).unwrap()["load"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let stats = chrono_session(
            &store,
            &json!({
                "op": "stats",
                "shots": "800, 800, 800, 800, 800, 800, 800, 900 mps"
            }),
        )
        .unwrap();
        assert!((stats["mean_mps"].as_f64().unwrap() - 812.5).abs() < 1e-6);
        assert_eq!(stats["outliers"][0], 7);
        let saved = chrono_session(
            &store,
            &json!({
                "op": "save",
                "confirm": true,
                "load_id": load_id,
                "label": "Bench",
                "shots": "800 mps, 802 mps",
                "temperature": {"text": "15 C", "unit": "C"}
            }),
        )
        .unwrap();
        assert_eq!(saved["saved"], true);
        let zone = point_blank(
            &store,
            &json!({
                "profile_id": id,
                "zone": {"text": "10 cm", "unit": "cm"},
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        let near = zone["near_m"].as_f64().unwrap();
        let zero = zone["optimal_zero_m"].as_f64().unwrap();
        let far = zone["far_m"].as_f64().unwrap();
        assert!(near < zero && zero < far, "{near} {zero} {far}");
        assert!(zone["near_zero_m"].as_f64().unwrap() > 1.0);
        assert!(zone["far_zero_m"].as_f64().unwrap() > zone["near_zero_m"].as_f64().unwrap());
        assert!(zone["x"]["labels"].as_array().unwrap().len() > 2);
        assert!(!zone["y"]["unit"].as_str().unwrap().is_empty());
        let fixed = point_blank(
            &store,
            &json!({
                "profile_id": id,
                "mode": "fixed",
                "zero": {"text": "100 m", "unit": "m"},
                "zone": {"text": "10 cm", "unit": "cm"},
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        assert_eq!(fixed["mode"], "fixed");
        assert!((fixed["far_zero_m"].as_f64().unwrap() - 100.0).abs() < 2.0);
        let pending = apply_zero(
            &store,
            &json!({
                "profile_id": id,
                "zone": {"text": "10 cm", "unit": "cm"},
                "confirm": false,
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        assert_eq!(pending["applied"], false);
        let still = store.profile_bundle(&id).unwrap()["profile"]["zero_range_m"]
            .as_f64()
            .unwrap();
        assert!((still - 100.0).abs() < 1e-6);

        let mut right = json!({
            "profile_id": id,
            "range": {"text": "80 m", "unit": "m"},
            "wind_speed": {"text": "0 mps", "unit": "mps"},
            "wind_direction": "12oc",
            "inclination": {"text": "0 deg", "unit": "deg"},
            "current_air": air(),
            "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false},
            "target_speed": {"text": "5.3 mps", "unit": "mps"},
            "target_direction": "3oc",
            "reaction_s": 0.0,
            "angular": "mrad"
        });
        let to_the_right = solve(&store, &right).unwrap();
        let lead = &to_the_right["lead"];
        assert!(lead["mrad"].as_f64().unwrap().abs() > 0.2, "{lead}");
        assert!(lead["moa"].as_f64().unwrap().abs() > lead["mrad"].as_f64().unwrap().abs());
        assert!(lead["cm"].as_f64().unwrap().abs() > 1.0);
        assert!(lead["tof_s"].as_f64().unwrap() > 0.0);
        assert!(lead["outline"].as_str().unwrap().contains("not drawn"));
        right["target_direction"] = json!("9oc");
        let to_the_left = solve(&store, &right).unwrap();
        assert!(
            to_the_left["lead"]["mrad"].as_f64().unwrap()
                * to_the_right["lead"]["mrad"].as_f64().unwrap()
                < 0.0
        );
        right["target_speed"] = json!({"text": "2 mps", "unit": "mps"});
        right["target_direction"] = json!("3oc");
        let slower = solve(&store, &right).unwrap();
        assert!(
            slower["lead"]["mrad"].as_f64().unwrap().abs()
                < to_the_right["lead"]["mrad"].as_f64().unwrap().abs()
        );
    }

    #[test]
    fn letter_pdf_page_is_612_by_792_and_the_bar_is_283_46_pt() {
        let store = store();
        let id = store.bootstrap().unwrap()["settings"]["active_profile_id"]
            .as_str()
            .unwrap()
            .to_string();
        let bytes = card_pdf_bytes(
            &store,
            &json!({
                "profile_id": id,
                "from": {"text": "100 m", "unit": "m"},
                "to": {"text": "200 m", "unit": "m"},
                "step": {"text": "100 m", "unit": "m"},
                "page": "letter",
                "brackets": "2 mps",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        let text = String::from_utf8_lossy(&bytes);
        let marker = "/MediaBox [0 0 ";
        let start = text.find(marker).unwrap() + marker.len();
        let end = text[start..].find(']').unwrap();
        let nums: Vec<f64> = text[start..start + end]
            .split_whitespace()
            .map(|part| part.parse().unwrap())
            .collect();
        assert!((nums[0] - 612.0).abs() < 0.01, "{}", nums[0]);
        assert!((nums[1] - 792.0).abs() < 0.01, "{}", nums[1]);
        let bar = text.find("% HOLDOVER_SCALE_BAR ").unwrap() + "% HOLDOVER_SCALE_BAR ".len();
        let bar_end = text[bar..].find('\n').unwrap();
        let commented: f64 = text[bar..bar + bar_end].trim().parse().unwrap();
        assert!((commented - 283.46).abs() < 0.01, "{commented}");
        let drawn: f64 = text[bar + bar_end..]
            .split(" re")
            .next()
            .unwrap()
            .split_whitespace()
            .filter_map(|token| token.parse().ok())
            .nth_back(1)
            .unwrap();
        assert!((drawn - 283.46).abs() < 0.01, "{drawn}");
        assert_eq!(store.paper_id().unwrap(), "letter");

        store
            .set_preferences(&json!({"preset": "metric", "paper": "a6"}))
            .unwrap();
        assert_eq!(store.paper_id().unwrap(), "a6");
        let compact = card_pdf_bytes(
            &store,
            &json!({
                "profile_id": id,
                "from": {"text": "100 m", "unit": "m"},
                "to": {"text": "100 m", "unit": "m"},
                "step": {"text": "100 m", "unit": "m"},
                "page": "compact",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false}
            }),
        )
        .unwrap();
        let compact_text = String::from_utf8_lossy(&compact);
        let compact_bar =
            compact_text.find("% HOLDOVER_SCALE_BAR ").unwrap() + "% HOLDOVER_SCALE_BAR ".len();
        let compact_end = compact_text[compact_bar..].find('\n').unwrap();
        let compact_pt: f64 = compact_text[compact_bar..compact_bar + compact_end]
            .trim()
            .parse()
            .unwrap();
        assert!((compact_pt - 141.73).abs() < 0.01, "{compact_pt}");
    }

    #[test]
    fn sfp_scene_scales_at_three_magnifications_and_moose_is_flagged() {
        let store = store();
        for (mag, factor) in [(5.0, 2.0), (10.0, 1.0), (20.0, 0.5)] {
            let scene = reticle_scene(
                &store,
                &json!({
                    "reticle_id": "generic-mil-dot",
                    "focal_plane": "sfp",
                    "calibrated_magnification": 10.0,
                    "magnification": mag,
                    "apparent_fov_deg": 22.0,
                    "viewport_px": 1000.0,
                    "range_m": 100.0,
                    "target_id": "sjf-algfigur"
                }),
            )
            .unwrap();
            assert!((scene["sfp_factor"].as_f64().unwrap() - factor).abs() < 1e-9);
            let px = scene["px_per_mrad"].as_f64().unwrap();
            let one = scene["nominal_one_true_mrad"].as_f64().unwrap();
            let pixels = one * px;
            let at_ten = {
                let apparent = 22.0_f64.to_radians() * 1000.0;
                1000.0 / (apparent / 10.0)
            };
            assert!((pixels - at_ten).abs() < 1.0, "{pixels} {at_ten}");
        }
        let impact = impact(
            &store,
            &json!({
                "profile_ids": [store.bootstrap().unwrap()["settings"]["active_profile_id"]],
                "range": {"text": "80 m", "unit": "m"},
                "target": "sjf-algfigur",
                "wind_speed": {"text": "0 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "inclination": {"text": "0 deg", "unit": "deg"},
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        let flags = impact["geometry"]["flags"].as_array().unwrap();
        assert!(flags.iter().any(|flag| flag["code"] == "T2"));
        assert!(flags.iter().any(|flag| flag["code"] == "T3"));
        assert!(impact["geometry"]["paths"].as_array().unwrap().len() >= 2);
        let diameter = 0.15_f64;
        let got = impact["rings_cm"].as_array().unwrap()[0].as_f64().unwrap() / 100.0;
        assert!((got - diameter).abs() < 1e-9);
    }

    #[test]
    fn compare_lists_two_loads_and_a_faster_one_drops_less() {
        let store = store();
        let id = store.bootstrap().unwrap()["settings"]["active_profile_id"]
            .as_str()
            .unwrap()
            .to_string();
        let bundle = store.profile_bundle(&id).unwrap();
        let before = solve(
            &store,
            &json!({
                "profile_id": id,
                "range": {"text": "300 m", "unit": "m"},
                "wind_speed": {"text": "0 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": true}
            }),
        )
        .unwrap();
        let load = store
            .save_load(&json!({
                "label": "Faster",
                "bullet_id": bundle["load"]["bullet_id"],
                "mv": {"text": "900 mps", "unit": "mps"}
            }))
            .unwrap();
        store
            .save_load_log(&json!({
                "label": "Notebook",
                "load_id": bundle["load"]["id"],
                "charge": {"text": "50 gr", "unit": "gr"}
            }))
            .unwrap();
        let after = solve(
            &store,
            &json!({
                "profile_id": id,
                "range": {"text": "300 m", "unit": "m"},
                "wind_speed": {"text": "0 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": true}
            }),
        )
        .unwrap();
        assert_eq!(before["drop_m"], after["drop_m"]);
        assert!(after["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap_or("").contains("not applied")));
        let faster = store
            .save_profile(&json!({
                "label": "Faster",
                "rifle_id": bundle["profile"]["rifle_id"],
                "scope_id": bundle["profile"]["scope_id"],
                "load_id": load["id"],
                "zero_range": {"text": "100 m", "unit": "m"},
                "zero_offset": {"text": "0 cm", "unit": "cm"},
                "zero_temperature": {"text": "15 C", "unit": "C"},
                "zero_pressure": {"text": "1013.25 hPa", "unit": "hPa"},
                "zero_pressure_kind": "station",
                "zero_humidity": "0%",
                "zero_altitude": {"text": "0 m", "unit": "m"}
            }))
            .unwrap();
        let compared = compare(
            &store,
            &json!({
                "profile_ids": [id, faster["id"]],
                "range": {"text": "300 m", "unit": "m"},
                "wind_speed": {"text": "4 mps", "unit": "mps"},
                "wind_direction": "9oc",
                "current_air": air(),
                "effects": {"incline": true, "spin_drift": false, "coriolis": false, "cant": false, "aerodynamic_jump": false}
            }),
        )
        .unwrap();
        let rows = compared["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows[0]["sight_height"].as_str().unwrap().contains("cm"));
        assert_eq!(rows[0]["bc"].as_f64().unwrap(), 0.243);
        let slow = rows[0]["drop_m"].as_f64().unwrap().abs();
        let fast = rows[1]["drop_m"].as_f64().unwrap().abs();
        assert!(fast < slow, "faster muzzle velocity should drop less");
        store.set_preferences(&json!({"preset": "us"})).unwrap();
        let us = compare(
            &store,
            &json!({
                "profile_ids": [id],
                "range": {"text": "300 m", "unit": "m"},
                "wind_speed": {"text": "0 mps", "unit": "mps"},
                "wind_direction": "12oc",
                "current_air": air(),
                "effects": {"spin_drift": false}
            }),
        )
        .unwrap();
        assert_eq!(us["velocity_unit"], "mph");
        assert!(us["rows"][0]["velocity"].as_str().unwrap().contains("mph"));
        assert!(us["rows"][0]["elevation"]["primary"]
            .as_str()
            .unwrap()
            .contains("MOA"));
        let click = us["rows"][0]["click"].as_str().unwrap();
        assert!(
            click.contains("mrad"),
            "click stays in the scope's unit, got {click}"
        );
    }
}
