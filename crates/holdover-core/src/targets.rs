//! Target library. Swedish figures first, then the US set.
//!
//! Published numbers are stored as published. Anything the plan still lists as
//! unmeasured is a flagged placeholder. This module does not invent an official
//! diameter, outline, or vital-zone size.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{Error, Result};

/// Current official SJF älgfigur diameters, centimetres, for rings 5¹, 5, 4 and 3.
///
/// Patrik confirmed these on 2026-10-17. The older 15/25/35/45 cm set is not a preset.
pub const SJF_MOOSE_RING_DIAMETER_CM: [f64; 4] = [15.0, 25.0, 33.0, 41.5];

/// SJF roe-buck ring diameters from the published rules, centimetres.
/// Item T4 is still open: confirm these on an owned target.
pub const SJF_ROE_BUCK_RING_DIAMETER_CM: [f64; 3] = [10.0, 15.0, 20.0];

/// Rough hit-area stand-in from plan §17.6. Not a measurement and not for scoring.
///
/// Origin is the ring centre. X is positive toward the rear of a left-facing figure.
/// The box is about 60 cm by 47 cm: 26 cm to the left edge, 35 cm to the rear apex,
/// 24 cm to the top, 22 cm to the bottom.
pub const MOOSE_HIT_AREA_CM: [[f64; 2]; 9] = [
    [-26.0, 4.0],
    [-22.0, -12.0],
    [-8.0, -22.0],
    [10.0, -22.0],
    [18.0, -12.0],
    [35.0, 16.0],
    [8.0, 24.0],
    [-14.0, 22.0],
    [-24.0, 12.0],
];

/// Decorative moose shape. The centimetres are not a measurement of the official figure.
const MOOSE_SILHOUETTE_CM: [[f64; 2]; 16] = [
    [-78.0, 6.0],
    [-66.0, 18.0],
    [-60.0, 34.0],
    [-48.0, 46.0],
    [-46.0, 22.0],
    [-34.0, 16.0],
    [8.0, 30.0],
    [48.0, 22.0],
    [64.0, 4.0],
    [58.0, -18.0],
    [16.0, -24.0],
    [-6.0, -48.0],
    [-18.0, -48.0],
    [-14.0, -20.0],
    [-36.0, -4.0],
    [-62.0, -2.0],
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RingDoc {
    pub diameter_cm: f64,
    pub label_primary: String,
    pub label_alt: String,
    pub points_primary: Option<i32>,
    pub points_alt: Option<i32>,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathDoc {
    pub role: String,
    pub points: Vec<[f64; 2]>,
    pub placeholder: bool,
    pub for_scoring: bool,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlagDoc {
    pub code: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TargetDoc {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub family: String,
    pub use_range_m: Option<f64>,
    pub distance_note: String,
    pub rings: Vec<RingDoc>,
    pub paths: Vec<PathDoc>,
    pub flags: Vec<FlagDoc>,
    pub hit_points_primary: Option<i32>,
    pub hit_points_alt: Option<i32>,
    pub hit_scoring: bool,
    pub spacing_measured: bool,
    pub official_bounds: bool,
    pub width_cm: Option<f64>,
    pub height_cm: Option<f64>,
    /// Grid cell, when this target is a sight-in grid. Not a scoring ring.
    pub cell_cm: Option<f64>,
    pub svg: Option<String>,
    pub user_edited: bool,
}

pub fn target_library() -> Vec<TargetDoc> {
    let mut out = vec![
        moose(),
        moose_calf(),
        at_range(
            gap(
                "sjf-vildsvin",
                "Vildsvinsfigur",
                "Wild boar figure",
                "sjf",
                "T5",
                "flag-T5-outline",
            ),
            "50 m",
            Some(50.0),
        ),
        at_range(
            gap(
                "sjf-vildsvinspasset",
                "Vildsvinspasset",
                "Yearling boar pass",
                "sjf",
                "T5",
                "flag-T5-all",
            ),
            "50 m standing, 30 m running",
            Some(50.0),
        ),
        roe("sjf-radjur-staende", "Stående råbock", "Standing roe buck"),
        roe("sjf-radjur-lopande", "Löpande råbock", "Running roe buck"),
        ringed(
            "sjf-rav",
            "Rävfigur",
            "Fox",
            "sjf",
            &[8.0, 12.0, 16.0],
            "T5",
            "flag-T5-outline",
        ),
        ringed(
            "sjf-tjader",
            "Tjäderfigur",
            "Capercaillie",
            "sjf",
            &[6.5, 11.0, 15.0],
            "T5",
            "flag-T5-outline",
        ),
        at_range(
            gap(
                "sjf-bjornpasset",
                "Björnpasset",
                "Bear pass",
                "sjf",
                "T5",
                "flag-T5-all",
            ),
            "80 m, 40 m and 20 m",
            Some(80.0),
        ),
        vital("practice-kronhjort", "Kronhjort", "Red deer, practice only"),
        vital(
            "practice-dovhjort",
            "Dovhjort",
            "Fallow deer, practice only",
        ),
        at_range(
            gap(
                "svsf-viltmal-50",
                "Viltmål 50 m",
                "Running boar, 10 m track",
                "svsf",
                "T5",
                "flag-T5-viltmal",
            ),
            "50 m",
            Some(50.0),
        ),
        at_range(
            gap(
                "svsf-viltmal-80",
                "Viltmål 80 m",
                "Running moose, 23 m track, 4.3 s",
                "svsf",
                "T5",
                "flag-T5-viltmal",
            ),
            "80 m",
            Some(80.0),
        ),
        enkel(),
        at_range(
            gap(
                "svsf-precision-300",
                "Precision 300 m",
                "International 300 m target",
                "svsf",
                "T5",
                "flag-T5-300",
            ),
            "300 m",
            Some(300.0),
        ),
    ];
    out.extend(field_figures());
    out.extend(us_set());
    out
}

pub fn target_by_id(id: &str) -> Option<TargetDoc> {
    let id = alias(id);
    target_library().into_iter().find(|item| item.id == id)
}

pub fn target_json(target: &TargetDoc) -> Value {
    serde_json::to_value(target).unwrap_or(Value::Null)
}

fn alias(id: &str) -> &str {
    match id {
        "moose" => "sjf-algfigur",
        "roe" => "sjf-radjur-staende",
        other => other,
    }
}

fn flag(code: &str, kind: &str, detail: &str) -> FlagDoc {
    FlagDoc {
        code: code.into(),
        kind: kind.into(),
        detail: detail.into(),
    }
}

fn ring(
    diameter_cm: f64,
    primary: &str,
    alt: &str,
    points: Option<i32>,
    alt_points: Option<i32>,
) -> RingDoc {
    RingDoc {
        diameter_cm,
        label_primary: primary.into(),
        label_alt: alt.into(),
        points_primary: points,
        points_alt: alt_points,
        verified: true,
    }
}

fn empty_doc(id: &str, name: &str, subtitle: &str, family: &str) -> TargetDoc {
    TargetDoc {
        id: id.into(),
        name: name.into(),
        subtitle: subtitle.into(),
        family: family.into(),
        use_range_m: None,
        distance_note: String::new(),
        rings: Vec::new(),
        paths: Vec::new(),
        flags: Vec::new(),
        hit_points_primary: None,
        hit_points_alt: None,
        hit_scoring: false,
        spacing_measured: false,
        official_bounds: false,
        width_cm: None,
        height_cm: None,
        cell_cm: None,
        svg: None,
        user_edited: false,
    }
}

fn moose() -> TargetDoc {
    let mut doc = empty_doc(
        "sjf-algfigur",
        "Älgfigur",
        "Swedish standard figure for running-moose shooting",
        "sjf",
    );
    doc.use_range_m = Some(80.0);
    doc.distance_note = "80 m".into();
    doc.rings = vec![
        ring(15.0, "5¹", "5", Some(5), Some(5)),
        ring(25.0, "5", "4", Some(5), Some(4)),
        ring(33.0, "4", "3", Some(4), Some(3)),
        ring(41.5, "3", "2", Some(3), Some(2)),
    ];
    doc.paths = vec![
        PathDoc {
            role: "hit_area".into(),
            points: MOOSE_HIT_AREA_CM.to_vec(),
            placeholder: true,
            for_scoring: false,
            closed: true,
        },
        PathDoc {
            role: "silhouette".into(),
            points: MOOSE_SILHOUETTE_CM.to_vec(),
            placeholder: true,
            for_scoring: false,
            closed: true,
        },
    ];
    doc.flags = vec![
        flag("T2", "placeholder", "flag-T2"),
        flag("T3", "placeholder", "flag-T3"),
    ];
    doc.hit_points_primary = Some(2);
    doc.hit_points_alt = Some(1);
    doc.hit_scoring = false;
    doc.spacing_measured = false;
    doc
}

fn moose_calf() -> TargetDoc {
    let mut doc = ringed(
        "sjf-algkalv",
        "Älgkalvfigur",
        "Moose calf",
        "sjf",
        &[11.2, 18.7, 26.2, 33.6],
        "T5",
        "flag-T5-outline",
    );
    doc.distance_note = "Natural size".into();
    doc
}

fn roe(id: &str, name: &str, subtitle: &str) -> TargetDoc {
    let mut doc = empty_doc(id, name, subtitle, "sjf");
    doc.use_range_m = Some(50.0);
    doc.distance_note = "50 m".into();
    doc.rings = vec![
        ring(10.0, "5", "10", Some(5), Some(10)),
        ring(15.0, "4", "9", Some(4), Some(9)),
        ring(20.0, "3", "8", Some(3), Some(8)),
    ];
    doc.paths = vec![PathDoc {
        role: "stand_in".into(),
        points: oval(16.0, 22.0, 16),
        placeholder: true,
        for_scoring: false,
        closed: true,
    }];
    doc.flags = vec![flag("T4", "unverified", "flag-T4")];
    doc
}

fn ringed(
    id: &str,
    name: &str,
    subtitle: &str,
    family: &str,
    diameters: &[f64],
    code: &str,
    detail: &str,
) -> TargetDoc {
    let mut doc = empty_doc(id, name, subtitle, family);
    doc.rings = diameters
        .iter()
        .map(|diameter| {
            let label = format_cm(*diameter);
            ring(*diameter, &label, "", None, None)
        })
        .collect();
    doc.flags = vec![flag(code, "placeholder", detail)];
    doc
}

fn gap(id: &str, name: &str, subtitle: &str, family: &str, code: &str, detail: &str) -> TargetDoc {
    let mut doc = empty_doc(id, name, subtitle, family);
    doc.flags = vec![flag(code, "unverified", detail)];
    doc
}

fn at_range(mut doc: TargetDoc, distance: &str, range_m: Option<f64>) -> TargetDoc {
    doc.distance_note = distance.into();
    doc.use_range_m = range_m;
    doc
}

fn vital(id: &str, name: &str, subtitle: &str) -> TargetDoc {
    let mut doc = empty_doc(id, name, subtitle, "practice");
    doc.flags = vec![flag("T6", "unverified", "flag-T6")];
    doc.distance_note = "No official figure found".into();
    doc
}

fn enkel() -> TargetDoc {
    let mut doc = empty_doc(
        "svsf-enkel-jagarkombination",
        "Enkel jägarkombination",
        "Running moose at 80 m and running roe at 50 m",
        "svsf",
    );
    doc.distance_note = "80 m and 50 m".into();
    doc.flags = vec![
        flag("T3", "placeholder", "flag-T3"),
        flag("T4", "unverified", "flag-T4"),
    ];
    doc
}

fn field_figures() -> Vec<TargetDoc> {
    const FIELD: &[(&str, &str, f64, f64, &str)] = &[
        ("svsf-c-15d", "C 15D", 15.0, 13.5, "125–175 m"),
        ("svsf-s-10-d-v", "S 10 D V", 10.0, 25.0, "150–200 m"),
        ("svsf-s-10-d-h", "S 10 D H", 25.0, 10.0, "125–175 m"),
        ("svsf-s-15-d-v", "S 15 D V", 15.0, 37.5, "175–225 m"),
        ("svsf-s-15-d-h", "S 15 D H", 37.5, 15.0, "160–220 m"),
        ("svsf-tunna-d", "Tunna D", 16.0, 20.0, "150–200 m"),
        ("svsf-c-20", "C 20", 20.0, 18.0, "185–235 m"),
        ("svsf-c-25", "C 25", 25.0, 22.5, "220–270 m"),
        ("svsf-c-30", "C 30", 30.0, 27.0, "270–320 m"),
        ("svsf-c-35", "C 35", 35.0, 31.5, "320–370 m"),
        ("svsf-c-40", "C 40", 40.0, 36.0, "360–410 m"),
        ("svsf-c-50", "C 50", 50.0, 45.0, "430–480 m"),
        ("svsf-s-20-v", "S 20 V", 20.0, 50.0, "275–325 m"),
        ("svsf-s-20-h", "S 20 H", 50.0, 20.0, "225–275 m"),
        ("svsf-s-25-v", "S 25 V", 25.0, 62.5, "340–390 m"),
        ("svsf-s-25-h", "S 25 H", 62.5, 25.0, "280–330 m"),
        ("svsf-1-7", "1/7", 20.0, 26.0, "225–280 m"),
        ("svsf-b-45", "B 45", 35.0, 45.0, "375–425 m"),
        ("svsf-b-65", "B 65", 50.0, 65.0, "475–550 m"),
        ("svsf-b-100", "B 100", 55.0, 100.0, "520–610 m"),
        ("svsf-1-8", "1/8", 37.0, 21.0, "200–265 m"),
        ("svsf-1-6-rak", "1/6 rak", 45.0, 26.0, "250–315 m"),
        ("svsf-1-6-hv", "1/6 H V", 45.0, 26.0, "260–325 m"),
        ("svsf-1-4-rak", "1/4 rak", 49.0, 33.0, "300–375 m"),
        ("svsf-1-4-hv", "1/4 H V", 60.0, 35.0, "325–400 m"),
        ("svsf-1-3", "1/3", 52.0, 48.0, "375–475 m"),
        ("svsf-tunna-v", "Tunna V", 40.0, 50.0, "410–490 m"),
        ("svsf-tunna-h", "Tunna H", 50.0, 40.0, "400–460 m"),
        ("svsf-l1", "L1", 19.0, 45.0, "210–260 m"),
        ("svsf-l2", "L2", 30.0, 70.0, "325–385 m"),
        ("svsf-l3", "L3", 43.0, 100.0, "425–500 m"),
    ];
    FIELD
        .iter()
        .map(|(id, name, w, h, distance)| {
            let mut doc = empty_doc(id, name, "Fältskytte figure, Gevär 6.5", "svsf");
            doc.distance_note = (*distance).into();
            doc.width_cm = Some(*w);
            doc.height_cm = Some(*h);
            doc.official_bounds = true;
            doc.paths = vec![PathDoc {
                role: "bounds".into(),
                points: rect(*w, *h),
                placeholder: true,
                for_scoring: false,
                closed: true,
            }];
            doc.flags = vec![flag("T5", "placeholder", "flag-T5-field")];
            doc
        })
        .collect()
}

fn us_set() -> Vec<TargetDoc> {
    let mut out = Vec::new();
    for (id, name, inches) in [
        ("us-plate-8in", "8 inch plate", 8.0),
        ("us-plate-10in", "10 inch plate", 10.0),
        ("us-plate-12in", "12 inch plate", 12.0),
    ] {
        out.push(plate(id, name, inches * 2.54, "us"));
    }
    for (id, name, cm) in [
        ("us-plate-20cm", "20 cm plate", 20.0),
        ("us-plate-30cm", "30 cm plate", 30.0),
        ("us-plate-40cm", "40 cm plate", 40.0),
    ] {
        out.push(plate(id, name, cm, "us"));
    }
    out.push(gap(
        "us-rect-plate",
        "Rectangular plate",
        "Common steel rectangle",
        "us",
        "US-RECT",
        "flag-US-RECT",
    ));
    out.push(gap(
        "us-ipsc",
        "IPSC/USPSA",
        "Cardboard silhouette",
        "us",
        "US-IPSC",
        "flag-US-IPSC",
    ));
    out.push(gap(
        "us-idpa",
        "IDPA",
        "Cardboard silhouette",
        "us",
        "US-IDPA",
        "flag-US-IDPA",
    ));
    out.push(gap(
        "us-steel-ipsc",
        "IPSC steel",
        "Steel silhouette",
        "us",
        "US-STEEL",
        "flag-US-STEEL",
    ));
    out.push(generic_bull());
    out.push(grid("grid-1cm", "Centimetre grid", 1.0, 20.0));
    out.push(grid("grid-1in", "Inch grid", 2.54, 25.4));
    out
}

fn plate(id: &str, name: &str, diameter_cm: f64, family: &str) -> TargetDoc {
    let mut doc = empty_doc(
        id,
        name,
        "Common steel plate. Not a competition rule.",
        family,
    );
    doc.rings = vec![ring(diameter_cm, "", "", None, None)];
    doc.rings[0].verified = false;
    doc.official_bounds = false;
    doc
}

fn generic_bull() -> TargetDoc {
    let mut doc = empty_doc(
        "generic-bullseye",
        "Practice bullseye",
        "Generic rings. Not an official target.",
        "generic",
    );
    for diameter in [4.0, 8.0, 12.0, 16.0, 20.0] {
        let mut item = ring(diameter, &format_cm(diameter), "", None, None);
        item.verified = false;
        doc.rings.push(item);
    }
    doc
}

fn grid(id: &str, name: &str, cell_cm: f64, span_cm: f64) -> TargetDoc {
    let mut doc = empty_doc(
        id,
        name,
        "Sight-in grid with corner marks. Not an official target.",
        "generic",
    );
    doc.width_cm = Some(span_cm);
    doc.height_cm = Some(span_cm);
    doc.cell_cm = Some(cell_cm);
    doc.official_bounds = true;
    let half = span_cm / 2.0;
    let mark = cell_cm.min(1.0);
    let mut fiducial = Vec::new();
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let x = sx * half;
        let y = sy * half;
        fiducial.push([x, y]);
        fiducial.push([x - sx * mark, y]);
        fiducial.push([x, y]);
        fiducial.push([x, y - sy * mark]);
    }
    doc.paths = vec![
        PathDoc {
            role: "bounds".into(),
            points: rect(span_cm, span_cm),
            placeholder: false,
            for_scoring: false,
            closed: true,
        },
        PathDoc {
            role: "fiducial".into(),
            points: fiducial,
            placeholder: false,
            for_scoring: false,
            closed: false,
        },
    ];
    doc
}

fn rect(width_cm: f64, height_cm: f64) -> Vec<[f64; 2]> {
    let x = width_cm / 2.0;
    let y = height_cm / 2.0;
    vec![[-x, -y], [x, -y], [x, y], [-x, y]]
}

fn oval(rx: f64, ry: f64, steps: usize) -> Vec<[f64; 2]> {
    (0..steps)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / steps as f64;
            [rx * angle.cos(), ry * angle.sin()]
        })
        .collect()
}

fn format_cm(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

/// Apply a user edit onto a library target, or build a custom target.
///
/// Edited geometry stays a placeholder. Saving does not turn it into an official measurement.
pub fn apply_target_edit(body: &Value) -> Result<TargetDoc> {
    let base_id = body.get("base_id").and_then(|v| v.as_str()).unwrap_or("");
    let mut doc = if base_id.is_empty() || base_id == "custom" {
        let mut custom = empty_doc(
            "custom",
            "Custom target",
            "User drawing. Not an official target.",
            "custom",
        );
        custom.user_edited = true;
        custom
    } else {
        target_by_id(base_id).ok_or_else(|| Error::Message(format!("unknown target {base_id}")))?
    };
    if let Some(label) = body.get("label").and_then(|v| v.as_str()) {
        if !label.trim().is_empty() {
            doc.name = label.trim().to_string();
        }
    }
    if let Some(paths) = body.get("paths").and_then(|v| v.as_array()) {
        let mut parsed = Vec::new();
        for path in paths {
            parsed.push(parse_path(path)?);
        }
        if !parsed.is_empty() {
            doc.paths = parsed;
            doc.user_edited = true;
        }
    }
    if let Some(diameter) = body.get("vital_diameter_cm").and_then(|v| v.as_f64()) {
        if diameter <= 0.0 || !diameter.is_finite() {
            return Err(Error::Invalid("vital-zone diameter must be positive"));
        }
        doc.rings.retain(|item| item.label_primary != "vital");
        let mut item = ring(diameter, "vital", "", None, None);
        item.verified = false;
        doc.rings.push(item);
        doc.user_edited = true;
    }
    if body.get("spacing_cm").is_some() {
        doc.spacing_measured = false;
        doc.user_edited = true;
    }
    if let Some(svg) = body.get("svg").and_then(|v| v.as_str()) {
        let clean = sanitize_svg(svg)?;
        let width = body
            .get("width_cm")
            .and_then(|v| v.as_f64())
            .ok_or(Error::Invalid("an imported drawing needs a width"))?;
        let height = body
            .get("height_cm")
            .and_then(|v| v.as_f64())
            .ok_or(Error::Invalid("an imported drawing needs a height"))?;
        if width <= 0.0 || height <= 0.0 {
            return Err(Error::Invalid("imported size must be positive"));
        }
        doc.svg = Some(clean);
        doc.width_cm = Some(width);
        doc.height_cm = Some(height);
        doc.user_edited = true;
        doc.family = "custom".into();
    }
    if doc.user_edited && !doc.flags.iter().any(|item| item.code == "user-edit") {
        doc.flags
            .push(flag("user-edit", "placeholder", "flag-user-edit"));
    }
    if let Some(id) = body
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        doc.id = id.to_string();
    }
    Ok(doc)
}

fn parse_path(value: &Value) -> Result<PathDoc> {
    let role = value
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("outline")
        .to_string();
    let raw = value
        .get("points")
        .and_then(|v| v.as_array())
        .ok_or(Error::Invalid("an outline needs points"))?;
    if raw.len() > 400 {
        return Err(Error::Invalid("an outline has too many points"));
    }
    let mut points = Vec::new();
    for point in raw {
        let pair = point
            .as_array()
            .filter(|row| row.len() == 2)
            .ok_or(Error::Invalid("an outline point must be x and y"))?;
        let x = pair[0]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or(Error::Invalid("an outline point must be a number"))?;
        let y = pair[1]
            .as_f64()
            .filter(|v| v.is_finite())
            .ok_or(Error::Invalid("an outline point must be a number"))?;
        points.push([x, y]);
    }
    if points.is_empty() {
        return Err(Error::Invalid("an outline needs points"));
    }
    Ok(PathDoc {
        role,
        points,
        placeholder: true,
        for_scoring: false,
        closed: value
            .get("closed")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    })
}

pub fn sanitize_svg(input: &str) -> Result<String> {
    if input.len() > 200_000 {
        return Err(Error::Invalid("the drawing is too large"));
    }
    let lower = input.to_ascii_lowercase();
    if !lower.contains("<svg") {
        return Err(Error::Invalid("the drawing must be an SVG"));
    }
    const BANNED: &[&str] = &[
        "<script",
        "javascript:",
        "onload",
        "onerror",
        "onclick",
        "onmouse",
        "<foreignobject",
        "<image",
        "<iframe",
        "<embed",
        "<object",
        "<use",
        "xlink:href",
        "href=",
        "url(",
        "<!entity",
        "<?xml-stylesheet",
    ];
    for token in BANNED {
        if lower.contains(token) {
            return Err(Error::Invalid(
                "the drawing has a script or an external reference",
            ));
        }
    }
    Ok(input.to_string())
}

pub fn mirror_paths(paths: &[PathDoc]) -> Vec<PathDoc> {
    paths
        .iter()
        .map(|path| {
            let mut copy = path.clone();
            for point in &mut copy.points {
                point[0] = -point[0];
            }
            copy
        })
        .collect()
}

pub fn bounds_cm(target: &TargetDoc) -> (f64, f64) {
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    let mut touch = |x: f64, y: f64| {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    };
    for ring in &target.rings {
        let r = ring.diameter_cm / 2.0;
        touch(-r, 0.0);
        touch(r, 0.0);
        touch(0.0, -r);
        touch(0.0, r);
    }
    for path in &target.paths {
        for point in &path.points {
            touch(point[0], point[1]);
        }
    }
    if let (Some(w), Some(h)) = (target.width_cm, target.height_cm) {
        touch(-w / 2.0, -h / 2.0);
        touch(w / 2.0, h / 2.0);
    }
    if !min_x.is_finite() || min_x > max_x {
        return (20.0, 20.0);
    }
    ((max_x - min_x).max(1.0), (max_y - min_y).max(1.0))
}

pub fn library_value() -> Value {
    json!(target_library().iter().map(target_json).collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(target: &TargetDoc) -> Vec<&str> {
        target.flags.iter().map(|flag| flag.code.as_str()).collect()
    }

    #[test]
    fn moose_preset_is_the_sjf_set_and_open_items_are_flagged() {
        assert_eq!(SJF_MOOSE_RING_DIAMETER_CM, [15.0, 25.0, 33.0, 41.5]);
        assert_eq!(SJF_ROE_BUCK_RING_DIAMETER_CM, [10.0, 15.0, 20.0]);
        let lib = target_library();
        for code in ["T2", "T3", "T4", "T5", "T6"] {
            assert!(
                lib.iter().any(|item| codes(item).contains(&code)),
                "missing {code}"
            );
        }
        let moose = target_by_id("moose").unwrap();
        assert_eq!(
            moose
                .rings
                .iter()
                .map(|ring| ring.diameter_cm)
                .collect::<Vec<_>>(),
            vec![15.0, 25.0, 33.0, 41.5]
        );
        assert!(moose.rings.iter().all(|ring| ring.verified));
        assert!(!codes(&moose).contains(&"T1"));
        assert!(codes(&moose).contains(&"T2"));
        assert!(codes(&moose).contains(&"T3"));
        assert!(moose.paths.iter().all(|path| !path.for_scoring));
        let hit = moose
            .paths
            .iter()
            .find(|path| path.role == "hit_area")
            .unwrap();
        let (min_x, max_x, min_y, max_y) = hit.points.iter().fold(
            (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
            |(a, b, c, d), point| {
                (
                    a.min(point[0]),
                    b.max(point[0]),
                    c.min(point[1]),
                    d.max(point[1]),
                )
            },
        );
        assert!((max_x - min_x - 61.0).abs() < 0.1);
        assert!((max_y - min_y - 46.0).abs() < 0.1);
        assert!(((max_x - min_x) - 60.0).abs() <= 3.0);
        assert!(((max_y - min_y) - 47.0).abs() <= 3.0);
        let roe = target_by_id("roe").unwrap();
        assert!(codes(&roe).contains(&"T4"));
        assert!(target_by_id("practice-kronhjort").unwrap().rings.is_empty());
        assert!(target_by_id("practice-dovhjort").unwrap().rings.is_empty());
        assert!(target_by_id("sjf-vildsvin").unwrap().rings.is_empty());
        assert!(target_by_id("us-ipsc").unwrap().rings.is_empty());
        assert!(target_by_id("svsf-precision-300").unwrap().rings.is_empty());
        let field = target_by_id("svsf-c-20").unwrap();
        assert_eq!(field.width_cm, Some(20.0));
        assert_eq!(field.height_cm, Some(18.0));
        assert!(field.official_bounds);
        assert!(codes(&field).contains(&"T5"));
    }

    #[test]
    fn svg_import_rejects_scripts_and_keeps_a_plain_drawing() {
        let bad = r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        assert!(sanitize_svg(bad).is_err());
        let good = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="4"/></svg>"#;
        assert!(sanitize_svg(good).is_ok());
        let edited = apply_target_edit(&json!({
            "base_id": "practice-kronhjort",
            "vital_diameter_cm": 25.0,
            "label": "My red deer"
        }))
        .unwrap();
        assert_eq!(edited.name, "My red deer");
        assert!(edited.user_edited);
        assert!(edited.flags.iter().any(|flag| flag.code == "T6"));
        assert!(edited.flags.iter().any(|flag| flag.code == "user-edit"));
        assert!(edited
            .rings
            .iter()
            .any(|ring| !ring.verified && (ring.diameter_cm - 25.0).abs() < 1e-9));
    }
}
