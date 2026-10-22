//! Shot-group geometry and statistics.
//!
//! The desktop and the later phone app both call this module. It does not read
//! or write files. A photograph stays in the shell; only pixel positions and a
//! known rectangle come in, and millimetres come out.
//!
//! Plane coordinates are millimetres, x to the right and y up, matching the
//! target drawings. Image coordinates are pixels, origin at the top left, x to
//! the right and y down.

use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::targets::TargetDoc;

/// `CEP ≈ 1.1774 · σ` for a circular normal group. σ is the pooled sample
/// standard deviation of x and y.
pub const CEP_SIGMA: f64 = 1.1774;

const HOMOGRAPHY_PRECISION_MM: f64 = 0.1;
const TWO_POINT_PRECISION_MM: f64 = 1.0;
const SKEW_WARN_DEG: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneMm {
    pub x: f64,
    pub y: f64,
}

/// Maps image pixels onto the target plane. The original photograph is not
/// changed; this matrix is the rectification, stored beside the photo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography {
    matrix: [f64; 9],
}

impl Homography {
    pub fn matrix(self) -> [f64; 9] {
        self.matrix
    }

    pub fn map(self, point: ImagePoint) -> Result<PlaneMm> {
        let (x, y) = apply_matrix(&self.matrix, point.x, point.y)?;
        Ok(PlaneMm { x, y })
    }

    pub fn invert_map(self, point: PlaneMm) -> Result<ImagePoint> {
        let inverse = invert_matrix(&self.matrix)?;
        let (x, y) = apply_matrix(&inverse, point.x, point.y)?;
        Ok(ImagePoint { x, y })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoPointScale {
    pub origin: ImagePoint,
    pub mm_per_px: f64,
    pub precision_mm: f64,
    pub skew_deg: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeasureMode {
    Centres,
    OutsideEdges { bullet_diameter_mm: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupStats {
    pub n: usize,
    pub extreme_spread_mm: f64,
    pub mean_radius_mm: f64,
    pub centre: PlaneMm,
    pub offset_x_mm: f64,
    pub offset_y_mm: f64,
    pub horizontal_spread_mm: f64,
    pub vertical_spread_mm: f64,
    pub sd_x_mm: f64,
    pub sd_y_mm: f64,
    pub radial_sd_mm: f64,
    pub cep_mm: Option<f64>,
    pub median_radius_mm: Option<f64>,
    pub distance_m: f64,
    pub es_mrad: Option<f64>,
    pub es_moa: Option<f64>,
    pub mr_mrad: Option<f64>,
    pub mr_moa: Option<f64>,
    pub offset_x_mrad: Option<f64>,
    pub offset_y_mrad: Option<f64>,
    pub offset_x_moa: Option<f64>,
    pub offset_y_moa: Option<f64>,
    pub correction_elevation_mrad: Option<f64>,
    pub correction_windage_mrad: Option<f64>,
    pub vertical: String,
    pub horizontal: String,
    pub precision_mm: f64,
    pub reported_extreme_spread_mm: f64,
    pub reported_mean_radius_mm: f64,
    pub reported_offset_x_mm: f64,
    pub reported_offset_y_mm: f64,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreShot {
    pub label: String,
    pub points: Option<i32>,
    pub miss: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreReport {
    pub available: bool,
    pub reason: String,
    pub shots: Vec<ScoreShot>,
    pub total: Option<i32>,
    pub possible: Option<i32>,
    pub notes: Vec<String>,
    pub flags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub gray: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rectified {
    pub raster: Raster,
    pub pixels_per_mm: f64,
    pub height_mm: f64,
}

impl Rectified {
    pub fn plane_mm(&self, pixel: ImagePoint) -> PlaneMm {
        PlaneMm {
            x: pixel.x / self.pixels_per_mm,
            y: self.height_mm - pixel.y / self.pixels_per_mm,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticScene {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub gray: Vec<u8>,
    pub corners_px: [ImagePoint; 4],
    pub corners_mm: [PlaneMm; 4],
    pub holes_px: Vec<ImagePoint>,
    pub holes_mm: Vec<PlaneMm>,
    pub poa_mm: PlaneMm,
    pub distance_m: f64,
    pub bullet_diameter_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
}

impl SyntheticScene {
    pub fn raster(&self) -> Raster {
        Raster {
            width: self.width,
            height: self.height,
            gray: self.gray.clone(),
        }
    }

    /// Sidecar for the PNG. The numbers are the known plane, not a measurement
    /// of the pixels, so a desktop check has something to compare with.
    pub fn description(&self) -> Value {
        let stats = group_stats(
            &self.holes_mm,
            self.poa_mm,
            self.distance_m,
            MeasureMode::Centres,
            HOMOGRAPHY_PRECISION_MM,
        )
        .ok();
        json!({
            "order": "top-left, top-right, bottom-right, bottom-left of the rectangle",
            "width_mm": self.width_mm,
            "height_mm": self.height_mm,
            "distance_m": self.distance_m,
            "bullet_diameter_mm": self.bullet_diameter_mm,
            "poa_mm": [self.poa_mm.x, self.poa_mm.y],
            "poa_is_rectangle_centre": true,
            "corners_px": self.corners_px.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
            "corners_mm": self.corners_mm.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
            "holes_mm": self.holes_mm.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
            "holes_px": self.holes_px.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
            "extreme_spread_mm": stats.as_ref().map(|item| item.extreme_spread_mm),
            "mean_radius_mm": stats.as_ref().map(|item| item.mean_radius_mm),
            "offset_x_mm": stats.as_ref().map(|item| item.offset_x_mm),
            "offset_y_mm": stats.as_ref().map(|item| item.offset_y_mm),
            "shot_count": self.holes_mm.len(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticCheck {
    pub max_position_error_mm: f64,
    pub extreme_spread_ratio: f64,
    pub mean_radius_ratio: f64,
    pub offset_ratio: f64,
    pub passed: bool,
    pub detected: usize,
    pub expected_extreme_spread_mm: f64,
    pub measured_extreme_spread_mm: f64,
    pub expected_mean_radius_mm: f64,
    pub measured_mean_radius_mm: f64,
    pub expected_offset_x_mm: f64,
    pub expected_offset_y_mm: f64,
    pub measured_offset_x_mm: f64,
    pub measured_offset_y_mm: f64,
}

impl SyntheticCheck {
    pub fn json(&self) -> Value {
        json!({
            "passed": self.passed,
            "max_position_error_mm": self.max_position_error_mm,
            "position_limit_mm": 0.5,
            "extreme_spread_ratio": self.extreme_spread_ratio,
            "mean_radius_ratio": self.mean_radius_ratio,
            "offset_ratio": self.offset_ratio,
            "ratio_limit": 0.01,
            "detected": self.detected,
            "expected_extreme_spread_mm": self.expected_extreme_spread_mm,
            "measured_extreme_spread_mm": self.measured_extreme_spread_mm,
            "expected_mean_radius_mm": self.expected_mean_radius_mm,
            "measured_mean_radius_mm": self.measured_mean_radius_mm,
            "expected_offset_x_mm": self.expected_offset_x_mm,
            "expected_offset_y_mm": self.expected_offset_y_mm,
            "measured_offset_x_mm": self.measured_offset_x_mm,
            "measured_offset_y_mm": self.measured_offset_y_mm,
        })
    }
}

/// Four image corners and the rectangle they mark, in millimetres.
///
/// Corner order is top-left, top-right, bottom-right, bottom-left of that
/// rectangle as it sits in the photograph.
pub fn homography(image: [ImagePoint; 4], plane: [PlaneMm; 4]) -> Result<Homography> {
    if quad_area_image(&image) < 1.0 || quad_area_plane(&plane) < 1.0 {
        return Err(Error::Invalid(
            "the four corners do not define a flat rectangle",
        ));
    }
    let (src, src_t) = normalize_image(&image);
    let (dst, dst_t) = normalize_plane(&plane);
    let mut rows = [[0.0; 8]; 8];
    let mut rhs = [0.0; 8];
    for (index, (pix, mill)) in src.iter().zip(dst.iter()).enumerate() {
        let u = pix.x;
        let v = pix.y;
        let x = mill.x;
        let y = mill.y;
        let row = index * 2;
        rows[row] = [u, v, 1.0, 0.0, 0.0, 0.0, -u * x, -v * x];
        rhs[row] = x;
        rows[row + 1] = [0.0, 0.0, 0.0, u, v, 1.0, -u * y, -v * y];
        rhs[row + 1] = y;
    }
    let solved = solve_8(&mut rows, &mut rhs)
        .map_err(|_| Error::Invalid("the four corners do not define a flat rectangle"))?;
    let normalized = [
        solved[0], solved[1], solved[2], solved[3], solved[4], solved[5], solved[6], solved[7], 1.0,
    ];
    let dst_inv = invert_similarity(&dst_t)?;
    let composed = mul3(&mul3(&dst_inv, &normalized), &src_t);
    let scale = composed[8];
    if !scale.is_finite() || scale.abs() < 1e-14 {
        return Err(Error::Invalid(
            "the four corners do not define a flat rectangle",
        ));
    }
    let matrix = composed.map(|value| value / scale);
    let homography = Homography { matrix };
    for (pix, mill) in image.iter().zip(plane.iter()) {
        let mapped = homography.map(*pix)?;
        if (mapped.x - mill.x).abs() > 1e-4 || (mapped.y - mill.y).abs() > 1e-4 {
            return Err(Error::Invalid(
                "the four corners do not define a flat rectangle",
            ));
        }
    }
    Ok(homography)
}

/// Scale from two points a known distance apart.
///
/// This does not correct tilt. `parallel`, when present, is two grid segments
/// that should be parallel: the angle between them is the residual skew.
pub fn two_point_scale(
    start: ImagePoint,
    end: ImagePoint,
    known_mm: f64,
    parallel: Option<[ImagePoint; 4]>,
) -> Result<TwoPointScale> {
    if !known_mm.is_finite() || known_mm <= 0.0 {
        return Err(Error::Invalid("the known distance has to be positive"));
    }
    let pixels = (end.x - start.x).hypot(end.y - start.y);
    if pixels < 1.0 {
        return Err(Error::Invalid("the two scale points are the same"));
    }
    let skew_deg =
        parallel.map(|points| direction_angle_deg(points[0], points[1], points[2], points[3]));
    Ok(TwoPointScale {
        origin: start,
        mm_per_px: known_mm / pixels,
        precision_mm: TWO_POINT_PRECISION_MM,
        skew_deg,
    })
}

pub fn map_two_point(scale: &TwoPointScale, point: ImagePoint) -> PlaneMm {
    PlaneMm {
        x: (point.x - scale.origin.x) * scale.mm_per_px,
        y: (scale.origin.y - point.y) * scale.mm_per_px,
    }
}

/// Centre-to-centre group figures. `precision_mm` is how finely the result may
/// be shown. Two-point scale uses 1 mm and does not report a fraction of a
/// millimetre.
pub fn group_stats(
    points: &[PlaneMm],
    poa: PlaneMm,
    distance_m: f64,
    mode: MeasureMode,
    precision_mm: f64,
) -> Result<GroupStats> {
    if points.is_empty() {
        return Err(Error::Invalid("a group needs at least one hole"));
    }
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(Error::Invalid("a hole position is not a number"));
    }
    let deduct = match mode {
        MeasureMode::Centres => 0.0,
        MeasureMode::OutsideEdges { bullet_diameter_mm } => {
            if !bullet_diameter_mm.is_finite() || bullet_diameter_mm <= 0.0 {
                return Err(Error::Invalid(
                    "an outside-edge group needs the bullet diameter",
                ));
            }
            bullet_diameter_mm
        }
    };
    let n = points.len();
    let centre = centroid(points);
    let mut extreme: f64 = 0.0;
    for left in 0..n {
        for right in (left + 1)..n {
            extreme = extreme.max(distance_mm(points[left], points[right]));
        }
    }
    let mut radii = Vec::with_capacity(n);
    for point in points {
        radii.push(distance_mm(*point, centre));
    }
    let mean_radius = radii.iter().sum::<f64>() / n as f64;
    let (min_x, max_x, min_y, max_y) = bounds(points);
    let xs: Vec<f64> = points.iter().map(|point| point.x).collect();
    let ys: Vec<f64> = points.iter().map(|point| point.y).collect();
    let sd_x = sample_sd(&xs, centre.x);
    let sd_y = sample_sd(&ys, centre.y);
    let radial_sd = (sd_x * sd_x + sd_y * sd_y).sqrt();
    let cep = if n >= 5 {
        let sigma = ((sd_x * sd_x + sd_y * sd_y) / 2.0).sqrt();
        Some(CEP_SIGMA * sigma)
    } else {
        None
    };
    let median_radius = if n >= 5 {
        Some(median(&mut radii))
    } else {
        None
    };
    let offset_x = centre.x - poa.x;
    let offset_y = centre.y - poa.y;
    let extreme_spread = (extreme - deduct).max(0.0);
    let horizontal_spread = ((max_x - min_x) - deduct).max(0.0);
    let vertical_spread = ((max_y - min_y) - deduct).max(0.0);
    let step = if precision_mm > 0.0 {
        precision_mm
    } else {
        HOMOGRAPHY_PRECISION_MM
    };
    let angles = |mm: f64| angle_pair(mm, distance_m);
    let (es_mrad, es_moa) = angles(extreme_spread);
    let (mr_mrad, mr_moa) = angles(mean_radius);
    let (offset_x_mrad, offset_x_moa) = angles(offset_x);
    let (offset_y_mrad, offset_y_moa) = angles(offset_y);
    let mut warnings = Vec::new();
    if n < 5 {
        warnings.push("small-n".to_string());
    }
    if cep.is_some() {
        warnings.push("cep-small-sample".to_string());
    }
    if deduct > 0.0 {
        warnings.push("edge-adjusted".to_string());
    }
    if step >= 1.0 - 1e-9 {
        warnings.push("no-sub-mm".to_string());
    }
    Ok(GroupStats {
        n,
        extreme_spread_mm: extreme_spread,
        mean_radius_mm: mean_radius,
        centre,
        offset_x_mm: offset_x,
        offset_y_mm: offset_y,
        horizontal_spread_mm: horizontal_spread,
        vertical_spread_mm: vertical_spread,
        sd_x_mm: sd_x,
        sd_y_mm: sd_y,
        radial_sd_mm: radial_sd,
        cep_mm: cep,
        median_radius_mm: median_radius,
        distance_m,
        es_mrad,
        es_moa,
        mr_mrad,
        mr_moa,
        offset_x_mrad,
        offset_y_mrad,
        offset_x_moa,
        offset_y_moa,
        correction_elevation_mrad: offset_y_mrad.map(|value| -value),
        correction_windage_mrad: offset_x_mrad.map(|value| -value),
        vertical: sense(offset_y, step, "UP", "DOWN"),
        horizontal: sense(offset_x, step, "RIGHT", "LEFT"),
        precision_mm: step,
        reported_extreme_spread_mm: round_step(extreme_spread, step),
        reported_mean_radius_mm: round_step(mean_radius, step),
        reported_offset_x_mm: round_step(offset_x, step),
        reported_offset_y_mm: round_step(offset_y, step),
        warnings,
    })
}

/// Score hole positions against a target's published rings.
///
/// Positions are millimetres from the aim point, which the caller treats as
/// the ring centre. Rings that are not verified, and placeholder outlines,
/// are not used. No diameter is invented for a target that does not publish one.
pub fn score_holes(points_mm: &[PlaneMm], target: &TargetDoc, alternate: bool) -> ScoreReport {
    let flags = target
        .flags
        .iter()
        .map(|flag| flag.code.clone())
        .collect::<Vec<_>>();
    let mut notes = Vec::new();
    if target
        .paths
        .iter()
        .any(|path| path.role == "hit_area" && (path.placeholder || !path.for_scoring))
    {
        notes.push("placeholder-not-scored".to_string());
    }
    let rings: Vec<_> = target
        .rings
        .iter()
        .filter(|ring| ring.verified && ring.diameter_cm > 0.0)
        .collect();
    if rings.is_empty() {
        return ScoreReport {
            available: false,
            reason: "no-published-zones".to_string(),
            shots: Vec::new(),
            total: None,
            possible: None,
            notes,
            flags,
        };
    }
    let mut ordered = rings;
    ordered.sort_by(|left, right| {
        left.diameter_cm
            .partial_cmp(&right.diameter_cm)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let scoring_paths: Vec<_> = target
        .paths
        .iter()
        .filter(|path| {
            path.for_scoring && !path.placeholder && path.closed && path.points.len() >= 3
        })
        .collect();
    let mut shots = Vec::with_capacity(points_mm.len());
    let mut all_points = true;
    let mut total = 0;
    let best = ordered
        .last()
        .map(|ring| {
            if alternate {
                ring.points_alt
            } else {
                ring.points_primary
            }
        })
        .unwrap_or(None);
    for point in points_mm {
        let x_cm = point.x / 10.0;
        let y_cm = point.y / 10.0;
        let radius = x_cm.hypot(y_cm);
        if let Some(ring) = ordered
            .iter()
            .find(|ring| radius <= ring.diameter_cm / 2.0 + 1e-9)
        {
            let label = ring_label(ring, alternate);
            let points = if alternate {
                ring.points_alt
            } else {
                ring.points_primary
            };
            if points.is_none() {
                all_points = false;
            }
            total += points.unwrap_or(0);
            shots.push(ScoreShot {
                label,
                points,
                miss: false,
            });
            continue;
        }
        if let Some(path) = scoring_paths
            .iter()
            .find(|path| inside_polygon(x_cm, y_cm, &path.points))
        {
            let points = if alternate {
                target.hit_points_alt
            } else {
                target.hit_points_primary
            };
            let _ = path;
            if points.is_none() {
                all_points = false;
            }
            total += points.unwrap_or(0);
            shots.push(ScoreShot {
                label: "hit".to_string(),
                points,
                miss: false,
            });
            continue;
        }
        shots.push(ScoreShot {
            label: "miss".to_string(),
            points: Some(0),
            miss: true,
        });
    }
    if ordered.iter().any(|ring| {
        if alternate {
            ring.points_alt.is_none()
        } else {
            ring.points_primary.is_none()
        }
    }) {
        notes.push("rings-without-points".to_string());
        all_points = false;
    }
    let possible = best.map(|points| points * points_mm.len() as i32);
    ScoreReport {
        available: true,
        reason: String::new(),
        shots,
        total: if all_points { Some(total) } else { None },
        possible: if all_points { possible } else { None },
        notes,
        flags,
    }
}

pub fn detect_blobs(
    raster: &Raster,
    threshold: u8,
    min_diameter_px: f64,
    max_diameter_px: f64,
) -> Vec<ImagePoint> {
    let width = raster.width;
    let height = raster.height;
    let pixels = (width as usize).saturating_mul(height as usize);
    if raster.gray.len() < pixels || width == 0 || height == 0 {
        return Vec::new();
    }
    let mut seen = vec![false; pixels];
    let mut found = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let start = index(width, x, y);
            if seen[start] || raster.gray[start] > threshold {
                continue;
            }
            let mut stack = vec![(x, y)];
            seen[start] = true;
            let mut weight_sum = 0.0;
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut area = 0.0;
            let mut min_x = x;
            let mut max_x = x;
            let mut min_y = y;
            let mut max_y = y;
            let mut touches = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
            while let Some((cx, cy)) = stack.pop() {
                let gray = raster.gray[index(width, cx, cy)];
                let weight = 255.0 - f64::from(gray);
                area += 1.0;
                weight_sum += weight;
                sum_x += (f64::from(cx) + 0.5) * weight;
                sum_y += (f64::from(cy) + 0.5) * weight;
                min_x = min_x.min(cx);
                max_x = max_x.max(cx);
                min_y = min_y.min(cy);
                max_y = max_y.max(cy);
                for ny in cy.saturating_sub(1)..=cy.saturating_add(1).min(height - 1) {
                    for nx in cx.saturating_sub(1)..=cx.saturating_add(1).min(width - 1) {
                        if nx == 0 || ny == 0 || nx + 1 == width || ny + 1 == height {
                            touches = true;
                        }
                        let next = index(width, nx, ny);
                        if seen[next] || raster.gray[next] > threshold {
                            continue;
                        }
                        seen[next] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            if touches || area < 12.0 || weight_sum <= 0.0 {
                continue;
            }
            let diameter = 2.0 * (area / std::f64::consts::PI).sqrt();
            if diameter < min_diameter_px || diameter > max_diameter_px {
                continue;
            }
            let box_w = f64::from(max_x - min_x) + 1.0;
            let box_h = f64::from(max_y - min_y) + 1.0;
            let aspect = box_w.max(box_h) / box_w.min(box_h).max(1.0);
            let fill = area / (box_w * box_h);
            if aspect > 1.65 || fill < 0.5 {
                continue;
            }
            found.push(ImagePoint {
                x: sum_x / weight_sum,
                y: sum_y / weight_sum,
            });
        }
    }
    found.sort_by(|left, right| {
        left.x
            .partial_cmp(&right.x)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left.y
                    .partial_cmp(&right.y)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    found
}

pub fn rectified_rectangle(
    source: &Raster,
    homography: &Homography,
    width_mm: f64,
    height_mm: f64,
    pixels_per_mm: f64,
) -> Result<Rectified> {
    if !width_mm.is_finite()
        || !height_mm.is_finite()
        || !pixels_per_mm.is_finite()
        || width_mm <= 0.0
        || height_mm <= 0.0
        || pixels_per_mm <= 0.0
    {
        return Err(Error::Invalid("the reference rectangle is empty"));
    }
    let width_px = limited_px(width_mm * pixels_per_mm)?;
    let height_px = limited_px(height_mm * pixels_per_mm)?;
    let mut gray = vec![255u8; (width_px as usize) * (height_px as usize)];
    for y in 0..height_px {
        for x in 0..width_px {
            let plane = PlaneMm {
                x: (f64::from(x) + 0.5) / pixels_per_mm,
                y: height_mm - (f64::from(y) + 0.5) / pixels_per_mm,
            };
            let Some(pixel) = homography.invert_map(plane).ok() else {
                continue;
            };
            gray[index(width_px, x, y)] = sample(source, pixel.x, pixel.y);
        }
    }
    Ok(Rectified {
        raster: Raster {
            width: width_px,
            height: height_px,
            gray,
        },
        pixels_per_mm,
        height_mm,
    })
}

pub fn locate_holes(
    source: &Raster,
    homography: &Homography,
    width_mm: f64,
    height_mm: f64,
    bullet_diameter_mm: f64,
    threshold: u8,
) -> Result<Vec<PlaneMm>> {
    let pixels_per_mm = rectify_scale(width_mm, height_mm);
    let rectified = rectified_rectangle(source, homography, width_mm, height_mm, pixels_per_mm)?;
    let (min_d, max_d) = diameter_window(bullet_diameter_mm, pixels_per_mm);
    let blobs = detect_blobs(&rectified.raster, threshold, min_d, max_d);
    Ok(blobs
        .into_iter()
        .map(|pixel| rectified.plane_mm(pixel))
        .collect())
}

/// Run the synthetic A4-scale photograph through rectification and detection.
pub fn check_synthetic() -> Result<SyntheticCheck> {
    let scene = synthetic_scene();
    let mapped = homography(scene.corners_px, scene.corners_mm)?;
    let found = locate_holes(
        &scene.raster(),
        &mapped,
        scene.width_mm,
        scene.height_mm,
        scene.bullet_diameter_mm,
        90,
    )?;
    if found.len() != scene.holes_mm.len() {
        let found_text = found
            .iter()
            .map(|point| format!("({:.1},{:.1})", point.x, point.y))
            .collect::<Vec<_>>()
            .join(" ");
        let truth_text = scene
            .holes_mm
            .iter()
            .map(|point| format!("({:.1},{:.1})", point.x, point.y))
            .collect::<Vec<_>>()
            .join(" ");
        return Err(Error::Message(format!(
            "detected {} holes, expected {}: found {found_text}; truth {truth_text}",
            found.len(),
            scene.holes_mm.len()
        )));
    }
    let mut used = vec![false; found.len()];
    let mut max_error: f64 = 0.0;
    let mut matched = Vec::with_capacity(found.len());
    for truth in &scene.holes_mm {
        let mut best_index = None;
        let mut best = f64::MAX;
        for (index, hole) in found.iter().enumerate() {
            if used[index] {
                continue;
            }
            let error = (hole.x - truth.x).hypot(hole.y - truth.y);
            if error < best {
                best = error;
                best_index = Some(index);
            }
        }
        let index = best_index.ok_or(Error::Invalid("a synthetic hole was not matched"))?;
        used[index] = true;
        max_error = max_error.max(best);
        matched.push(found[index]);
    }
    let expected = group_stats(
        &scene.holes_mm,
        scene.poa_mm,
        scene.distance_m,
        MeasureMode::Centres,
        HOMOGRAPHY_PRECISION_MM,
    )?;
    let measured = group_stats(
        &matched,
        scene.poa_mm,
        scene.distance_m,
        MeasureMode::Centres,
        HOMOGRAPHY_PRECISION_MM,
    )?;
    let extreme_spread_ratio =
        relative_error(measured.extreme_spread_mm, expected.extreme_spread_mm);
    let mean_radius_ratio = relative_error(measured.mean_radius_mm, expected.mean_radius_mm);
    let offset_ratio = relative_error(measured.offset_x_mm, expected.offset_x_mm)
        .max(relative_error(measured.offset_y_mm, expected.offset_y_mm));
    let passed = max_error <= 0.5
        && extreme_spread_ratio <= 0.01
        && mean_radius_ratio <= 0.01
        && offset_ratio <= 0.01;
    Ok(SyntheticCheck {
        max_position_error_mm: max_error,
        extreme_spread_ratio,
        mean_radius_ratio,
        offset_ratio,
        passed,
        detected: found.len(),
        expected_extreme_spread_mm: expected.extreme_spread_mm,
        measured_extreme_spread_mm: measured.extreme_spread_mm,
        expected_mean_radius_mm: expected.mean_radius_mm,
        measured_mean_radius_mm: measured.mean_radius_mm,
        expected_offset_x_mm: expected.offset_x_mm,
        expected_offset_y_mm: expected.offset_y_mm,
        measured_offset_x_mm: measured.offset_x_mm,
        measured_offset_y_mm: measured.offset_y_mm,
    })
}

/// A perspective photograph of a 200 mm by 280 mm rectangle on an A4-sized
/// sheet, with five holes at known centres. The rectangle is the scale.
pub fn synthetic_scene() -> SyntheticScene {
    let width_mm = 200.0;
    let height_mm = 280.0;
    let width = 960u32;
    let height = 1280u32;
    let corners_px = [
        ImagePoint { x: 70.0, y: 50.0 },
        ImagePoint { x: 890.0, y: 130.0 },
        ImagePoint {
            x: 850.0,
            y: 1210.0,
        },
        ImagePoint {
            x: 120.0,
            y: 1130.0,
        },
    ];
    let corners_mm = [
        PlaneMm {
            x: 0.0,
            y: height_mm,
        },
        PlaneMm {
            x: width_mm,
            y: height_mm,
        },
        PlaneMm {
            x: width_mm,
            y: 0.0,
        },
        PlaneMm { x: 0.0, y: 0.0 },
    ];
    let poa_mm = PlaneMm {
        x: width_mm / 2.0,
        y: height_mm / 2.0,
    };
    // Zero-mean cluster plus a known offset, so the mean point of impact is
    // (22, -16) mm from the aim point. Centres stay more than two bullet
    // diameters apart, or rectification would merge the black circles.
    let relative = [
        [62.0, 9.0],
        [-23.0, -8.0],
        [30.0, -56.0],
        [40.0, 2.0],
        [1.0, -27.0],
    ];
    let holes_mm: Vec<PlaneMm> = relative
        .iter()
        .map(|point| PlaneMm {
            x: poa_mm.x + point[0],
            y: poa_mm.y + point[1],
        })
        .collect();
    let mapped = homography(corners_px, corners_mm).expect("synthetic corners define a plane");
    let holes_px: Vec<ImagePoint> = holes_mm
        .iter()
        .map(|point| {
            mapped
                .invert_map(*point)
                .expect("synthetic hole is on the plane")
        })
        .collect();
    let mut rgba = vec![255u8; (width as usize) * (height as usize) * 4];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    for index in 0..4 {
        let next = (index + 1) % 4;
        stroke(
            &mut rgba,
            width,
            height,
            corners_px[index],
            corners_px[next],
            [20, 20, 20, 255],
        );
    }
    for corner in &corners_px {
        cross(&mut rgba, width, height, *corner, 8.0, [0, 0, 0, 255]);
    }
    let bullet_diameter_mm = 7.82;
    for (pixel, mill) in holes_px.iter().zip(holes_mm.iter()) {
        let radius_px = hole_radius_px(&mapped, *mill, bullet_diameter_mm / 2.0);
        fill_circle(
            &mut rgba,
            width,
            height,
            pixel.x,
            pixel.y,
            radius_px,
            [0, 0, 0, 255],
        );
    }
    let gray = rgba_to_gray(&rgba);
    SyntheticScene {
        width,
        height,
        rgba,
        gray,
        corners_px,
        corners_mm,
        holes_px,
        holes_mm,
        poa_mm,
        distance_m: 100.0,
        bullet_diameter_mm,
        width_mm,
        height_mm,
    }
}

/// Dark round marks, returned in image pixels so markers sit on the original
/// photograph. A missing bullet diameter uses a wide window and is not a caliber.
pub fn detect_image_holes(
    raster: &Raster,
    calibration: &Value,
    bullet_diameter_mm: Option<f64>,
    threshold: u8,
) -> Result<Value> {
    let mode = calibration
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let image_points = read_points(calibration.get("image_points")).unwrap_or_default();
    let mut warnings: Vec<String> = Vec::new();
    let diameter = bullet_diameter_mm.filter(|value| value.is_finite() && *value > 0.0);
    if diameter.is_none() {
        warnings.push("no-diameter".into());
    }
    let points = if mode == "homography" || mode == "four" {
        if image_points.len() != 4 {
            return Err(Error::Invalid(
                "mark the four corners before searching for holes",
            ));
        }
        let width_mm = finite_positive(calibration.get("width_mm"))?;
        let height_mm = finite_positive(calibration.get("height_mm"))?;
        let image = [
            image_points[0],
            image_points[1],
            image_points[2],
            image_points[3],
        ];
        let plane = [
            PlaneMm {
                x: 0.0,
                y: height_mm,
            },
            PlaneMm {
                x: width_mm,
                y: height_mm,
            },
            PlaneMm {
                x: width_mm,
                y: 0.0,
            },
            PlaneMm { x: 0.0, y: 0.0 },
        ];
        let mapped = homography(image, plane)?;
        let found = located_plane(raster, &mapped, width_mm, height_mm, diameter, threshold)?;
        let mut image_holes = Vec::with_capacity(found.len());
        for point in found {
            image_holes.push(mapped.invert_map(point)?);
        }
        image_holes
    } else if mode == "two-point" || mode == "two" {
        if image_points.len() != 2 {
            return Err(Error::Invalid(
                "mark the two scale points before searching for holes",
            ));
        }
        let known_mm = finite_positive(calibration.get("known_mm"))?;
        let scale = two_point_scale(image_points[0], image_points[1], known_mm, None)?;
        let (min_d, max_d) = match diameter {
            Some(mm) => {
                let px = mm / scale.mm_per_px;
                (px * 0.45, px * 2.2)
            }
            None => (4.0, 80.0),
        };
        detect_blobs(raster, threshold, min_d, max_d)
    } else {
        return Err(Error::Invalid(
            "calibrate the photograph before searching for holes",
        ));
    };
    Ok(json!({
        "holes": points.iter().map(|point| json!({"x": point.x, "y": point.y})).collect::<Vec<_>>(),
        "warnings": warnings,
    }))
}

fn located_plane(
    raster: &Raster,
    mapped: &Homography,
    width_mm: f64,
    height_mm: f64,
    diameter: Option<f64>,
    threshold: u8,
) -> Result<Vec<PlaneMm>> {
    if let Some(mm) = diameter {
        return locate_holes(raster, mapped, width_mm, height_mm, mm, threshold);
    }
    let pixels_per_mm = rectify_scale(width_mm, height_mm);
    let rectified = rectified_rectangle(raster, mapped, width_mm, height_mm, pixels_per_mm)?;
    Ok(detect_blobs(
        &rectified.raster,
        threshold,
        2.0 * pixels_per_mm,
        20.0 * pixels_per_mm,
    )
    .into_iter()
    .map(|pixel| rectified.plane_mm(pixel))
    .collect())
}

/// JSON the shell can show while the user is still marking holes.
pub fn analyze_document(input: &Value, target: Option<&TargetDoc>) -> Result<Value> {
    let calibration = input.get("calibration").unwrap_or(&Value::Null);
    let mode = calibration
        .get("mode")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let image_points = match read_points(calibration.get("image_points")) {
        Some(points) => points,
        None => {
            return Ok(partial("corners"));
        }
    };
    let holes = read_holes(input.get("holes"))?;
    let distance_m = input.get("distance_m").and_then(|value| value.as_f64());
    let expected = input
        .get("expected_shots")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as usize;
    let alternate = input.get("ringing").and_then(|value| value.as_str()) == Some("alt");
    let measure = input
        .get("measure")
        .and_then(|value| value.as_str())
        .unwrap_or("centres");
    let diameter = input
        .get("bullet_diameter_mm")
        .and_then(|value| value.as_f64())
        .filter(|value| value.is_finite() && *value > 0.0);
    let (plane_holes, poa, precision, mut warnings, matrix, poa_image) =
        if mode == "homography" || mode == "four" {
            if image_points.len() != 4 {
                return Ok(partial("corners"));
            }
            let width_mm = match finite_positive(calibration.get("width_mm")) {
                Ok(value) => value,
                Err(_) => return Ok(partial("size")),
            };
            let height_mm = match finite_positive(calibration.get("height_mm")) {
                Ok(value) => value,
                Err(_) => return Ok(partial("size")),
            };
            let image = [
                image_points[0],
                image_points[1],
                image_points[2],
                image_points[3],
            ];
            let plane = [
                PlaneMm {
                    x: 0.0,
                    y: height_mm,
                },
                PlaneMm {
                    x: width_mm,
                    y: height_mm,
                },
                PlaneMm {
                    x: width_mm,
                    y: 0.0,
                },
                PlaneMm { x: 0.0, y: 0.0 },
            ];
            let mapped = homography(image, plane)?;
            let poa = if input
                .get("use_rectangle_centre")
                .and_then(|value| value.as_bool())
                == Some(true)
            {
                PlaneMm {
                    x: width_mm / 2.0,
                    y: height_mm / 2.0,
                }
            } else if let Some(point) = read_point(input.get("poa_image")) {
                mapped.map(point)?
            } else {
                return Ok(partial("aim-point"));
            };
            let mut millimetres = Vec::new();
            for hole in &holes {
                millimetres.push(mapped.map(hole.0)?);
            }
            let poa_image = mapped.invert_map(poa).ok();
            (
                millimetres,
                poa,
                HOMOGRAPHY_PRECISION_MM,
                Vec::new(),
                Some(mapped.matrix().to_vec()),
                poa_image,
            )
        } else if mode == "two-point" || mode == "two" {
            if image_points.len() != 2 {
                return Ok(partial("corners"));
            }
            let known_mm = match finite_positive(calibration.get("known_mm")) {
                Ok(value) => value,
                Err(_) => return Ok(partial("size")),
            };
            let parallel = read_points(calibration.get("grid")).filter(|points| points.len() == 4);
            let scale = two_point_scale(
                image_points[0],
                image_points[1],
                known_mm,
                parallel.map(|points| [points[0], points[1], points[2], points[3]]),
            )?;
            let mut warnings = vec!["orthogonal-photo".to_string()];
            if scale.skew_deg.unwrap_or(0.0) > SKEW_WARN_DEG {
                warnings.push("skew".to_string());
            }
            let poa = if let Some(point) = read_point(input.get("poa_image")) {
                map_two_point(&scale, point)
            } else {
                return Ok(partial("aim-point"));
            };
            let millimetres = holes
                .iter()
                .map(|hole| map_two_point(&scale, hole.0))
                .collect();
            let poa_image = read_point(input.get("poa_image"));
            (
                millimetres,
                poa,
                scale.precision_mm,
                warnings,
                None,
                poa_image,
            )
        } else {
            return Ok(partial("calibration"));
        };
    let shot_count: usize = holes.iter().map(|hole| hole.1).sum();
    if holes.iter().any(|hole| hole.1 > 1) {
        warnings.push("overlap-same-centre".to_string());
    }
    let mut expanded = Vec::new();
    for (index, hole) in holes.iter().enumerate() {
        for _ in 0..hole.1 {
            expanded.push(plane_holes[index]);
        }
    }
    let mode_measure = if measure == "edges" {
        MeasureMode::OutsideEdges {
            bullet_diameter_mm: diameter.unwrap_or(0.0),
        }
    } else {
        MeasureMode::Centres
    };
    if matches!(
        mode_measure,
        MeasureMode::OutsideEdges { bullet_diameter_mm } if bullet_diameter_mm <= 0.0
    ) && !expanded.is_empty()
    {
        return Ok(partial("diameter"));
    }
    let stats = if expanded.is_empty() {
        None
    } else {
        Some(group_stats(
            &expanded,
            poa,
            distance_m.unwrap_or(0.0),
            mode_measure,
            precision,
        )?)
    };
    if let Some(stats) = &stats {
        for warning in &stats.warnings {
            if !warnings.iter().any(|item| item == warning) {
                warnings.push(warning.clone());
            }
        }
    }
    let distance_ok = distance_m.filter(|value| value.is_finite() && *value > 0.0);
    let can_save = stats.is_some()
        && distance_ok.is_some()
        && expected > 0
        && shot_count == expected
        && (measure != "edges" || diameter.is_some());
    let score = if let Some(target) = target {
        if expanded.is_empty() {
            Value::Null
        } else {
            score_value(&score_holes(&expanded, target, alternate))
        }
    } else {
        Value::Null
    };
    let hole_rows: Vec<Value> = plane_holes
        .iter()
        .enumerate()
        .map(|(index, point)| {
            json!({
                "mm": [point.x, point.y],
                "relative_mm": [point.x - poa.x, point.y - poa.y],
                "count": holes[index].1,
            })
        })
        .collect();
    Ok(json!({
        "ok": true,
        "reason": "",
        "can_save": can_save,
        "shot_count": shot_count,
        "expected_shots": expected,
        "precision_mm": precision,
        "warnings": warnings,
        "poa_mm": [poa.x, poa.y],
        "poa_image": poa_image.map(|point| json!([point.x, point.y])),
        "matrix": matrix,
        "skew_deg": calibration_skew(calibration),
        "holes": hole_rows,
        "stats": stats.as_ref().map(stats_value),
        "score": score,
    }))
}

fn calibration_skew(calibration: &Value) -> Option<f64> {
    let points = read_points(calibration.get("grid"))?;
    if points.len() != 4 {
        return None;
    }
    Some(direction_angle_deg(
        points[0], points[1], points[2], points[3],
    ))
}

fn partial(reason: &str) -> Value {
    json!({
        "ok": false,
        "reason": reason,
        "can_save": false,
        "warnings": [],
        "stats": null,
        "score": null,
    })
}

fn stats_value(stats: &GroupStats) -> Value {
    json!({
        "n": stats.n,
        "extreme_spread_mm": stats.extreme_spread_mm,
        "mean_radius_mm": stats.mean_radius_mm,
        "reported_extreme_spread_mm": stats.reported_extreme_spread_mm,
        "reported_mean_radius_mm": stats.reported_mean_radius_mm,
        "reported_offset_x_mm": stats.reported_offset_x_mm,
        "reported_offset_y_mm": stats.reported_offset_y_mm,
        "offset_x_mm": stats.offset_x_mm,
        "offset_y_mm": stats.offset_y_mm,
        "centre_x_mm": stats.centre.x,
        "centre_y_mm": stats.centre.y,
        "horizontal_spread_mm": stats.horizontal_spread_mm,
        "vertical_spread_mm": stats.vertical_spread_mm,
        "sd_x_mm": stats.sd_x_mm,
        "sd_y_mm": stats.sd_y_mm,
        "radial_sd_mm": stats.radial_sd_mm,
        "cep_mm": stats.cep_mm,
        "median_radius_mm": stats.median_radius_mm,
        "es_mrad": stats.es_mrad,
        "es_moa": stats.es_moa,
        "mr_mrad": stats.mr_mrad,
        "mr_moa": stats.mr_moa,
        "offset_x_mrad": stats.offset_x_mrad,
        "offset_y_mrad": stats.offset_y_mrad,
        "offset_x_moa": stats.offset_x_moa,
        "offset_y_moa": stats.offset_y_moa,
        "correction_elevation_mrad": stats.correction_elevation_mrad,
        "correction_windage_mrad": stats.correction_windage_mrad,
        "vertical": stats.vertical,
        "horizontal": stats.horizontal,
        "precision_mm": stats.precision_mm,
        "distance_m": stats.distance_m,
        "warnings": stats.warnings,
    })
}

fn score_value(score: &ScoreReport) -> Value {
    json!({
        "available": score.available,
        "reason": score.reason,
        "total": score.total,
        "possible": score.possible,
        "notes": score.notes,
        "flags": score.flags,
        "shots": score.shots.iter().map(|shot| json!({
            "label": shot.label,
            "points": shot.points,
            "miss": shot.miss,
        })).collect::<Vec<_>>(),
    })
}

fn read_holes(value: Option<&Value>) -> Result<Vec<(ImagePoint, usize)>> {
    let Some(items) = value.and_then(|item| item.as_array()) else {
        return Ok(Vec::new());
    };
    let mut holes = Vec::new();
    for item in items {
        let Some(point) = read_point(item.get("image")) else {
            return Err(Error::Invalid("a hole is missing its position"));
        };
        let count = item
            .get("count")
            .and_then(|value| value.as_u64())
            .unwrap_or(1)
            .clamp(1, 12) as usize;
        holes.push((point, count));
    }
    Ok(holes)
}

fn read_points(value: Option<&Value>) -> Option<Vec<ImagePoint>> {
    let items = value?.as_array()?;
    let mut points = Vec::new();
    for item in items {
        points.push(read_point(Some(item))?);
    }
    Some(points)
}

fn read_point(value: Option<&Value>) -> Option<ImagePoint> {
    let value = value?;
    if let Some(pair) = value.as_array() {
        if pair.len() < 2 {
            return None;
        }
        return finite_point(pair[0].as_f64()?, pair[1].as_f64()?);
    }
    let x = value.get("x").and_then(|item| item.as_f64())?;
    let y = value.get("y").and_then(|item| item.as_f64())?;
    finite_point(x, y)
}

fn finite_point(x: f64, y: f64) -> Option<ImagePoint> {
    if x.is_finite() && y.is_finite() {
        Some(ImagePoint { x, y })
    } else {
        None
    }
}

fn finite_positive(value: Option<&Value>) -> Result<f64> {
    let number = value
        .and_then(|item| item.as_f64())
        .filter(|item| item.is_finite() && *item > 0.0 && *item < 10_000.0)
        .ok_or(Error::Invalid(
            "the reference size has to be a positive length",
        ))?;
    Ok(number)
}

fn ring_label(ring: &crate::targets::RingDoc, alternate: bool) -> String {
    if alternate && !ring.label_alt.is_empty() {
        ring.label_alt.clone()
    } else if !ring.label_primary.is_empty() {
        ring.label_primary.clone()
    } else {
        format!("{} cm", ring.diameter_cm)
    }
}

fn inside_polygon(x: f64, y: f64, points: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let mut previous = points.len() - 1;
    for index in 0..points.len() {
        let yi = points[index][1];
        let yj = points[previous][1];
        if (yi > y) != (yj > y) {
            let xi = points[index][0];
            let xj = points[previous][0];
            let cross = (xj - xi) * (y - yi) / (yj - yi) + xi;
            if x < cross {
                inside = !inside;
            }
        }
        previous = index;
    }
    inside
}

fn centroid(points: &[PlaneMm]) -> PlaneMm {
    let n = points.len() as f64;
    PlaneMm {
        x: points.iter().map(|point| point.x).sum::<f64>() / n,
        y: points.iter().map(|point| point.y).sum::<f64>() / n,
    }
}

fn bounds(points: &[PlaneMm]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    (min_x, max_x, min_y, max_y)
}

fn distance_mm(left: PlaneMm, right: PlaneMm) -> f64 {
    (left.x - right.x).hypot(left.y - right.y)
}

fn sample_sd(values: &[f64], mean: f64) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let sum: f64 = values.iter().map(|value| (value - mean).powi(2)).sum();
    (sum / (values.len() - 1) as f64).sqrt()
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        0.5 * (values[n / 2 - 1] + values[n / 2])
    }
}

fn angle_pair(offset_mm: f64, distance_m: f64) -> (Option<f64>, Option<f64>) {
    if !distance_m.is_finite() || distance_m <= 0.0 || !offset_mm.is_finite() {
        return (None, None);
    }
    let mrad = (offset_mm / 1000.0 / distance_m).atan() * 1000.0;
    let moa = mrad * (10.8 / std::f64::consts::PI);
    (Some(mrad), Some(moa))
}

fn sense(value: f64, step: f64, positive: &str, negative: &str) -> String {
    let shown = round_step(value, step);
    if shown == 0.0 {
        "ON".to_string()
    } else if shown > 0.0 {
        positive.to_string()
    } else {
        negative.to_string()
    }
}

fn round_step(value: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return value;
    }
    (value / step).round() * step
}

fn relative_error(measured: f64, expected: f64) -> f64 {
    if expected.abs() < 1e-9 {
        return measured.abs();
    }
    (measured - expected).abs() / expected.abs()
}

fn direction_angle_deg(a0: ImagePoint, a1: ImagePoint, b0: ImagePoint, b1: ImagePoint) -> f64 {
    let ax = a1.x - a0.x;
    let ay = a1.y - a0.y;
    let bx = b1.x - b0.x;
    let by = b1.y - b0.y;
    let denom = ax.hypot(ay) * bx.hypot(by);
    if denom < 1e-9 {
        return 0.0;
    }
    let cos = ((ax * bx + ay * by) / denom).clamp(-1.0, 1.0);
    let mut degrees = cos.acos().to_degrees();
    if degrees > 90.0 {
        degrees = 180.0 - degrees;
    }
    degrees
}

fn quad_area_image(points: &[ImagePoint; 4]) -> f64 {
    let mut sum = 0.0;
    for index in 0..4 {
        let next = (index + 1) % 4;
        sum += points[index].x * points[next].y - points[next].x * points[index].y;
    }
    sum.abs() * 0.5
}

fn quad_area_plane(points: &[PlaneMm; 4]) -> f64 {
    let mut sum = 0.0;
    for index in 0..4 {
        let next = (index + 1) % 4;
        sum += points[index].x * points[next].y - points[next].x * points[index].y;
    }
    sum.abs() * 0.5
}

fn normalize_image(points: &[ImagePoint; 4]) -> (Vec<ImagePoint>, [f64; 9]) {
    let cx = points.iter().map(|point| point.x).sum::<f64>() / 4.0;
    let cy = points.iter().map(|point| point.y).sum::<f64>() / 4.0;
    let dist = points
        .iter()
        .map(|point| (point.x - cx).hypot(point.y - cy))
        .sum::<f64>()
        / 4.0;
    let scale = if dist < 1e-9 {
        1.0
    } else {
        std::f64::consts::SQRT_2 / dist
    };
    let shifted = points
        .iter()
        .map(|point| ImagePoint {
            x: (point.x - cx) * scale,
            y: (point.y - cy) * scale,
        })
        .collect();
    let matrix = [
        scale,
        0.0,
        -scale * cx,
        0.0,
        scale,
        -scale * cy,
        0.0,
        0.0,
        1.0,
    ];
    (shifted, matrix)
}

fn normalize_plane(points: &[PlaneMm; 4]) -> (Vec<PlaneMm>, [f64; 9]) {
    let cx = points.iter().map(|point| point.x).sum::<f64>() / 4.0;
    let cy = points.iter().map(|point| point.y).sum::<f64>() / 4.0;
    let dist = points
        .iter()
        .map(|point| (point.x - cx).hypot(point.y - cy))
        .sum::<f64>()
        / 4.0;
    let scale = if dist < 1e-9 {
        1.0
    } else {
        std::f64::consts::SQRT_2 / dist
    };
    let shifted = points
        .iter()
        .map(|point| PlaneMm {
            x: (point.x - cx) * scale,
            y: (point.y - cy) * scale,
        })
        .collect();
    let matrix = [
        scale,
        0.0,
        -scale * cx,
        0.0,
        scale,
        -scale * cy,
        0.0,
        0.0,
        1.0,
    ];
    (shifted, matrix)
}

fn invert_similarity(matrix: &[f64; 9]) -> Result<[f64; 9]> {
    let scale = matrix[0];
    if scale.abs() < 1e-14 {
        return Err(Error::Invalid(
            "the four corners do not define a flat rectangle",
        ));
    }
    Ok([
        1.0 / scale,
        0.0,
        -matrix[2] / scale,
        0.0,
        1.0 / scale,
        -matrix[5] / scale,
        0.0,
        0.0,
        1.0,
    ])
}

fn mul3(left: &[f64; 9], right: &[f64; 9]) -> [f64; 9] {
    let mut out = [0.0; 9];
    for row in 0..3 {
        for col in 0..3 {
            out[row * 3 + col] = left[row * 3] * right[col]
                + left[row * 3 + 1] * right[3 + col]
                + left[row * 3 + 2] * right[6 + col];
        }
    }
    out
}

fn apply_matrix(matrix: &[f64; 9], x: f64, y: f64) -> Result<(f64, f64)> {
    let weight = matrix[6] * x + matrix[7] * y + matrix[8];
    if !weight.is_finite() || weight.abs() < 1e-12 {
        return Err(Error::Invalid("the photo point is off the target plane"));
    }
    let mapped_x = (matrix[0] * x + matrix[1] * y + matrix[2]) / weight;
    let mapped_y = (matrix[3] * x + matrix[4] * y + matrix[5]) / weight;
    if !mapped_x.is_finite() || !mapped_y.is_finite() {
        return Err(Error::Invalid("the photo point is off the target plane"));
    }
    Ok((mapped_x, mapped_y))
}

fn invert_matrix(matrix: &[f64; 9]) -> Result<[f64; 9]> {
    let det = matrix[0] * (matrix[4] * matrix[8] - matrix[5] * matrix[7])
        - matrix[1] * (matrix[3] * matrix[8] - matrix[5] * matrix[6])
        + matrix[2] * (matrix[3] * matrix[7] - matrix[4] * matrix[6]);
    if !det.is_finite() || det.abs() < 1e-14 {
        return Err(Error::Invalid("the rectification cannot be reversed"));
    }
    let inv = 1.0 / det;
    Ok([
        (matrix[4] * matrix[8] - matrix[5] * matrix[7]) * inv,
        (matrix[2] * matrix[7] - matrix[1] * matrix[8]) * inv,
        (matrix[1] * matrix[5] - matrix[2] * matrix[4]) * inv,
        (matrix[5] * matrix[6] - matrix[3] * matrix[8]) * inv,
        (matrix[0] * matrix[8] - matrix[2] * matrix[6]) * inv,
        (matrix[2] * matrix[3] - matrix[0] * matrix[5]) * inv,
        (matrix[3] * matrix[7] - matrix[4] * matrix[6]) * inv,
        (matrix[1] * matrix[6] - matrix[0] * matrix[7]) * inv,
        (matrix[0] * matrix[4] - matrix[1] * matrix[3]) * inv,
    ])
}

#[allow(clippy::needless_range_loop)]
fn solve_8(rows: &mut [[f64; 8]; 8], rhs: &mut [f64; 8]) -> Result<[f64; 8]> {
    for col in 0..8 {
        let mut pivot = col;
        let mut best = rows[col][col].abs();
        for row in (col + 1)..8 {
            let value = rows[row][col].abs();
            if value > best {
                best = value;
                pivot = row;
            }
        }
        if best < 1e-12 {
            return Err(Error::Invalid(
                "the four corners do not define a flat rectangle",
            ));
        }
        if pivot != col {
            rows.swap(pivot, col);
            rhs.swap(pivot, col);
        }
        let divisor = rows[col][col];
        for row in (col + 1)..8 {
            let factor = rows[row][col] / divisor;
            for index in col..8 {
                rows[row][index] -= factor * rows[col][index];
            }
            rhs[row] -= factor * rhs[col];
        }
    }
    let mut solution = [0.0; 8];
    for row in (0..8).rev() {
        let mut value = rhs[row];
        for index in (row + 1)..8 {
            value -= rows[row][index] * solution[index];
        }
        if rows[row][row].abs() < 1e-12 {
            return Err(Error::Invalid(
                "the four corners do not define a flat rectangle",
            ));
        }
        solution[row] = value / rows[row][row];
    }
    Ok(solution)
}

fn index(width: u32, x: u32, y: u32) -> usize {
    (y as usize) * (width as usize) + (x as usize)
}

fn limited_px(value: f64) -> Result<u32> {
    if !value.is_finite() || value < 8.0 || value > 4000.0 {
        return Err(Error::Invalid(
            "the rectified image is an unreasonable size",
        ));
    }
    Ok(value.round() as u32)
}

fn rectify_scale(width_mm: f64, height_mm: f64) -> f64 {
    let long_side = width_mm.max(height_mm);
    (1600.0 / long_side).clamp(4.0, 8.0)
}

fn diameter_window(bullet_diameter_mm: f64, pixels_per_mm: f64) -> (f64, f64) {
    let px = bullet_diameter_mm.max(1.0) * pixels_per_mm;
    (px * 0.45, px * 2.2)
}

fn sample(source: &Raster, x: f64, y: f64) -> u8 {
    if x < 0.0 || y < 0.0 || x > f64::from(source.width) - 1.0 || y > f64::from(source.height) - 1.0
    {
        return 255;
    }
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(source.width - 1);
    let y1 = (y0 + 1).min(source.height - 1);
    let tx = x - f64::from(x0);
    let ty = y - f64::from(y0);
    let p00 = f64::from(source.gray[index(source.width, x0, y0)]);
    let p10 = f64::from(source.gray[index(source.width, x1, y0)]);
    let p01 = f64::from(source.gray[index(source.width, x0, y1)]);
    let p11 = f64::from(source.gray[index(source.width, x1, y1)]);
    let value = p00 * (1.0 - tx) * (1.0 - ty)
        + p10 * tx * (1.0 - ty)
        + p01 * (1.0 - tx) * ty
        + p11 * tx * ty;
    value.round().clamp(0.0, 255.0) as u8
}

fn hole_radius_px(homography: &Homography, centre: PlaneMm, radius_mm: f64) -> f64 {
    let Some(origin) = homography.invert_map(centre).ok() else {
        return 8.0;
    };
    let east = homography
        .invert_map(PlaneMm {
            x: centre.x + radius_mm,
            y: centre.y,
        })
        .ok();
    let north = homography
        .invert_map(PlaneMm {
            x: centre.x,
            y: centre.y + radius_mm,
        })
        .ok();
    match (east, north) {
        (Some(east), Some(north)) => {
            let rx = (east.x - origin.x).hypot(east.y - origin.y);
            let ry = (north.x - origin.x).hypot(north.y - origin.y);
            ((rx + ry) * 0.5).clamp(3.0, 80.0)
        }
        _ => 8.0,
    }
}

fn rgba_to_gray(rgba: &[u8]) -> Vec<u8> {
    rgba.chunks_exact(4)
        .map(|pixel| {
            let value = 0.2126 * f64::from(pixel[0])
                + 0.7152 * f64::from(pixel[1])
                + 0.0722 * f64::from(pixel[2]);
            value.round().clamp(0.0, 255.0) as u8
        })
        .collect()
}

fn put_pixel(rgba: &mut [u8], width: u32, height: u32, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let offset = ((y as u32 as usize) * (width as usize) + x as usize) * 4;
    rgba[offset..offset + 4].copy_from_slice(&color);
}

fn fill_circle(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    cx: f64,
    cy: f64,
    radius: f64,
    color: [u8; 4],
) {
    let radius_sq = radius * radius;
    let x0 = (cx - radius - 1.0).floor() as i32;
    let x1 = (cx + radius + 1.0).ceil() as i32;
    let y0 = (cy - radius - 1.0).floor() as i32;
    let y1 = (cy + radius + 1.0).ceil() as i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = (f64::from(x) + 0.5) - cx;
            let dy = (f64::from(y) + 0.5) - cy;
            if dx * dx + dy * dy <= radius_sq {
                put_pixel(rgba, width, height, x, y, color);
            }
        }
    }
}

fn cross(rgba: &mut [u8], width: u32, height: u32, centre: ImagePoint, arm: f64, color: [u8; 4]) {
    let x = centre.x.round() as i32;
    let y = centre.y.round() as i32;
    let reach = arm.round() as i32;
    for step in -reach..=reach {
        put_pixel(rgba, width, height, x + step, y, color);
        put_pixel(rgba, width, height, x, y + step, color);
    }
}

fn stroke(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    start: ImagePoint,
    end: ImagePoint,
    color: [u8; 4],
) {
    let mut x0 = start.x.round() as i32;
    let mut y0 = start.y.round() as i32;
    let x1 = end.x.round() as i32;
    let y1 = end.y.round() as i32;
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    loop {
        put_pixel(rgba, width, height, x0, y0, color);
        put_pixel(rgba, width, height, x0 + 1, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let doubled = 2 * err;
        if doubled > -dy {
            err -= dy;
            x0 += sx;
        }
        if doubled < dx {
            err += dx;
            y0 += sy;
        }
    }
}
