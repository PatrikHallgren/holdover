//! Command line for the Holdover solver.
//!
//! `holdover-cli solve --profile p.json --range 437m --wind 3mps@9oc --json`

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use holdover_core::{
    parse_quantity, parse_twist, parse_wind, Atmosphere, BcBand, BcReference, Bullet, Click,
    Dimension, DragModel, DragSpec, Effects, PressureKind, Shot, TableSpec, TwistHand, DISCLAIMER,
};
use serde::Deserialize;

#[derive(Parser)]
#[command(
    name = "holdover-cli",
    version,
    about = "Holdover ballistic calculator"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Solve a single slant range.
    Solve {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        range: String,
        #[arg(long)]
        wind: Option<String>,
        #[arg(long)]
        inclination: Option<String>,
        /// Print a JSON object instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Solve a range table.
    Table {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        step: String,
        #[arg(long)]
        wind: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Density, speed of sound and density altitude for one atmosphere.
    Atmosphere {
        #[arg(long)]
        temperature: String,
        #[arg(long)]
        pressure: String,
        #[arg(long, default_value = "station")]
        pressure_kind: String,
        #[arg(long, default_value = "0")]
        humidity: String,
        #[arg(long, default_value = "0 m")]
        altitude: String,
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("holdover-cli: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    match cli.command {
        Command::Solve {
            profile,
            range,
            wind,
            inclination,
            json,
        } => {
            let mut shot = load_shot(&profile)?;
            apply_overrides(&mut shot, wind, inclination)?;
            let range_m =
                parse_quantity(&range, Dimension::Length, None).map_err(|e| e.to_string())?;
            let solution = holdover_core::solve(&shot, range_m).map_err(|e| e.to_string())?;
            emit_solution(&solution, json);
        }
        Command::Table {
            profile,
            from,
            to,
            step,
            wind,
            json,
        } => {
            let mut shot = load_shot(&profile)?;
            apply_overrides(&mut shot, wind, None)?;
            let spec = TableSpec {
                start_m: length(&from)?,
                end_m: length(&to)?,
                step_m: length(&step)?,
            };
            let solution = holdover_core::table(&shot, spec).map_err(|e| e.to_string())?;
            emit_solution(&solution, json);
        }
        Command::Atmosphere {
            temperature,
            pressure,
            pressure_kind,
            humidity,
            altitude,
            json,
        } => {
            let atmo = atmosphere_from_parts(
                &temperature,
                &pressure,
                &pressure_kind,
                &humidity,
                &altitude,
            )?;
            let state = atmo.state().map_err(|e| e.to_string())?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "density_kg_m3": state.density_kg_m3,
                        "speed_of_sound_mps": state.speed_of_sound_mps,
                        "station_pressure_pa": state.station_pressure_pa,
                        "density_altitude_m": state.density_altitude_m,
                        "pressure_kind_suspicious": state.pressure_kind_suspicious,
                        "disclaimer": DISCLAIMER,
                    })
                );
            } else {
                println!(
                    "density {:.4} kg/m³ · sound {:.1} m/s · station {:.1} hPa · density altitude {:.0} m",
                    state.density_kg_m3,
                    state.speed_of_sound_mps,
                    state.station_pressure_pa / 100.0,
                    state.density_altitude_m
                );
                println!("{DISCLAIMER}");
            }
        }
    }
    Ok(())
}

fn emit_solution(solution: &holdover_core::Solution, json: bool) {
    if json {
        let points: Vec<_> = solution
            .points
            .iter()
            .map(|point| {
                serde_json::json!({
                    "range_m": point.range_m,
                    "path_m": point.path_m,
                    "drop_m": point.drop_m,
                    "windage_m": point.windage_m,
                    "velocity_mps": point.velocity_mps,
                    "energy_j": point.energy_j,
                    "energy_ft_lbf": point.energy_ft_lbf,
                    "tof_s": point.tof_s,
                    "mach": point.mach,
                    "elevation_mrad": point.elevation.mrad,
                    "elevation_moa": point.elevation.moa,
                    "elevation_iphy": point.elevation.iphy,
                    "elevation_cm_per_100m": point.elevation.cm_per_100m,
                    "elevation_direction": direction_name(point.elevation.direction),
                    "elevation_clicks": point.elevation.clicks,
                    "windage_mrad": point.windage.mrad,
                    "windage_iphy": point.windage.iphy,
                    "windage_cm_per_100m": point.windage.cm_per_100m,
                    "windage_direction": direction_name(point.windage.direction),
                    "windage_clicks": point.windage.clicks,
                    "spin_drift_m": point.spin_drift_m,
                    "sg": point.sg,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "bore_angle_mrad": solution.bore_angle_rad * 1000.0,
                "muzzle_velocity_mps": solution.muzzle_velocity_mps,
                "density_kg_m3": solution.density_kg_m3,
                "speed_of_sound_mps": solution.speed_of_sound_mps,
                "density_altitude_m": solution.density_altitude_m,
                "transonic_range_m": solution.transonic_range_m,
                "warnings": solution.warnings.iter().map(|w| format!("{w:?}")).collect::<Vec<_>>(),
                "points": points,
                "disclaimer": DISCLAIMER,
            })
        );
    } else {
        for point in &solution.points {
            println!(
                "{:.0} m   {} {:.2} mrad   {} {:.2} mrad",
                point.range_m,
                direction_name(point.elevation.direction),
                point.elevation.mrad.abs(),
                direction_name(point.windage.direction),
                point.windage.mrad.abs()
            );
            if let (Some(up), Some(right)) = (point.elevation.clicks, point.windage.clicks) {
                println!("clicks {up} / {right}");
            }
            println!(
                "{:.0} m/s · {:.0} J · {:.3} s · Mach {:.2}",
                point.velocity_mps, point.energy_j, point.tof_s, point.mach
            );
        }
        println!("{DISCLAIMER}");
    }
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

fn apply_overrides(
    shot: &mut Shot,
    wind: Option<String>,
    inclination: Option<String>,
) -> Result<(), String> {
    if let Some(wind) = wind {
        shot.wind = parse_wind(&wind).map_err(|e| e.to_string())?;
    }
    if let Some(inclination) = inclination {
        shot.inclination_rad = parse_quantity(&inclination, Dimension::Angle, Some("deg"))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn length(text: &str) -> Result<f64, String> {
    parse_quantity(text, Dimension::Length, None).map_err(|e| e.to_string())
}

fn load_shot(path: &PathBuf) -> Result<Shot, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let profile: Profile = serde_json::from_str(&text).map_err(|e| format!("profile JSON: {e}"))?;
    profile.into_shot()
}

#[derive(Deserialize)]
struct Profile {
    mv: String,
    #[serde(default)]
    mv_temperature: Option<String>,
    #[serde(default)]
    powder_temperature: Option<String>,
    #[serde(default)]
    temperature_sensitivity: Option<String>,
    sight_height: String,
    #[serde(default)]
    bore_angle: Option<String>,
    #[serde(default)]
    zero: Option<ZeroProfile>,
    atmosphere: AtmoProfile,
    #[serde(default)]
    inclination: Option<String>,
    #[serde(default)]
    cant: Option<String>,
    #[serde(default)]
    twist: Option<String>,
    #[serde(default)]
    twist_hand: Option<String>,
    bullet: BulletProfile,
    #[serde(default)]
    scope: Option<ScopeProfile>,
    #[serde(default)]
    effects: Option<EffectsProfile>,
    #[serde(default)]
    latitude: Option<String>,
    #[serde(default)]
    azimuth: Option<String>,
    #[serde(default)]
    wind: Option<String>,
}

#[derive(Deserialize)]
struct ZeroProfile {
    range: String,
    #[serde(default)]
    offset: Option<String>,
    #[serde(default)]
    atmosphere: Option<AtmoProfile>,
}

#[derive(Deserialize)]
struct AtmoProfile {
    temperature: String,
    pressure: String,
    #[serde(default = "default_station")]
    pressure_kind: String,
    #[serde(default)]
    humidity: f64,
    #[serde(default = "default_zero")]
    altitude: String,
}

#[derive(Deserialize)]
struct BulletProfile {
    weight: String,
    diameter: String,
    #[serde(default)]
    length: Option<String>,
    drag: DragProfile,
}

#[derive(Deserialize)]
struct DragProfile {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    bc: Option<f64>,
    #[serde(default)]
    reference: Option<String>,
    #[serde(default)]
    bands: Option<Vec<BandProfile>>,
    #[serde(default)]
    custom: Option<Vec<(f64, f64)>>,
}

#[derive(Deserialize)]
struct BandProfile {
    bc: f64,
    #[serde(default)]
    min: Option<String>,
    #[serde(default)]
    reference: Option<String>,
}

#[derive(Deserialize)]
struct ScopeProfile {
    click: String,
    #[serde(default = "default_one")]
    tracking_factor: f64,
    #[serde(default)]
    travel_up: Option<String>,
    #[serde(default)]
    travel_down: Option<String>,
    #[serde(default)]
    travel_right: Option<String>,
    #[serde(default)]
    travel_left: Option<String>,
}

#[derive(Deserialize)]
struct EffectsProfile {
    #[serde(default = "default_true")]
    incline: bool,
    #[serde(default = "default_true")]
    spin_drift: bool,
    #[serde(default)]
    coriolis: bool,
    #[serde(default)]
    cant: bool,
    #[serde(default)]
    aerodynamic_jump: bool,
}

fn default_station() -> String {
    "station".to_string()
}
fn default_zero() -> String {
    "0 m".to_string()
}
fn default_one() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}

impl Profile {
    fn into_shot(self) -> Result<Shot, String> {
        let atmosphere = self.atmosphere.parse()?;
        let mut shot = Shot::basic(
            parse_quantity(&self.mv, Dimension::Velocity, None).map_err(|e| e.to_string())?,
            self.bullet.parse()?,
            atmosphere,
        );
        shot.sight_height_m = length(&self.sight_height)?;
        shot.mv_temperature_c = optional_temp(self.mv_temperature)?;
        shot.powder_temperature_c = optional_temp(self.powder_temperature)?;
        if let Some(sens) = self.temperature_sensitivity {
            shot.temperature_sensitivity_mps_per_c = Some(
                parse_quantity(&sens, Dimension::Velocity, Some("m/s"))
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(angle) = self.bore_angle {
            shot.bore_angle_rad = Some(
                parse_quantity(&angle, Dimension::Angle, Some("mrad"))
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(zero) = self.zero {
            shot.zero_range_m = Some(length(&zero.range)?);
            if let Some(offset) = zero.offset {
                shot.zero_offset_m = length(&offset)?;
            }
            if let Some(atmo) = zero.atmosphere {
                shot.zero_atmosphere = Some(atmo.parse()?);
            }
        }
        if let Some(inclination) = self.inclination {
            shot.inclination_rad = parse_quantity(&inclination, Dimension::Angle, Some("deg"))
                .map_err(|e| e.to_string())?;
        }
        if let Some(cant) = self.cant {
            shot.cant_rad =
                parse_quantity(&cant, Dimension::Angle, Some("deg")).map_err(|e| e.to_string())?;
        }
        if let Some(twist) = self.twist {
            let mut parsed = parse_twist(&twist).map_err(|e| e.to_string())?;
            if let Some(hand) = self.twist_hand {
                parsed.hand = match hand.to_lowercase().as_str() {
                    "left" | "lh" => TwistHand::Left,
                    "right" | "rh" => TwistHand::Right,
                    _ => return Err("twist_hand must be right or left".into()),
                };
            }
            shot.twist = Some(parsed);
        }
        if let Some(scope) = self.scope {
            shot.click = Some(scope.parse()?);
        }
        if let Some(effects) = self.effects {
            shot.effects = Effects {
                incline: effects.incline,
                spin_drift: effects.spin_drift,
                coriolis: effects.coriolis,
                cant: effects.cant,
                aerodynamic_jump: effects.aerodynamic_jump,
            };
        }
        if let Some(latitude) = self.latitude {
            shot.latitude_rad = parse_quantity(&latitude, Dimension::Angle, Some("deg"))
                .map_err(|e| e.to_string())?;
        }
        if let Some(azimuth) = self.azimuth {
            shot.azimuth_rad = parse_quantity(&azimuth, Dimension::Angle, Some("deg"))
                .map_err(|e| e.to_string())?;
        }
        if let Some(wind) = self.wind {
            shot.wind = parse_wind(&wind).map_err(|e| e.to_string())?;
        }
        Ok(shot)
    }
}

impl BulletProfile {
    fn parse(self) -> Result<Bullet, String> {
        Ok(Bullet {
            weight_kg: parse_quantity(&self.weight, Dimension::Mass, None)
                .map_err(|e| e.to_string())?,
            diameter_m: length(&self.diameter)?,
            length_m: self.length.as_deref().map(length).transpose()?,
            drag: self.drag.parse()?,
        })
    }
}

impl DragProfile {
    fn parse(self) -> Result<DragSpec, String> {
        if let Some(points) = self.custom {
            return Ok(DragSpec::Custom { points });
        }
        let model =
            DragModel::parse(self.model.as_deref().unwrap_or("G1")).map_err(|e| e.to_string())?;
        let bands = if let Some(bands) = self.bands {
            bands
                .into_iter()
                .map(|band| band.parse())
                .collect::<Result<Vec<_>, _>>()?
        } else if let Some(bc) = self.bc {
            vec![BcBand {
                bc_lb_per_in2: bc,
                min_mps: 0.0,
                reference: parse_reference(self.reference.as_deref())?,
            }]
        } else {
            return Err("drag needs a bc or a custom Cd table".into());
        };
        Ok(DragSpec::Standard { model, bands })
    }
}

impl BandProfile {
    fn parse(self) -> Result<BcBand, String> {
        Ok(BcBand {
            bc_lb_per_in2: self.bc,
            min_mps: match self.min {
                Some(min) => parse_quantity(&min, Dimension::Velocity, Some("m/s"))
                    .map_err(|e| e.to_string())?,
                None => 0.0,
            },
            reference: parse_reference(self.reference.as_deref())?,
        })
    }
}

fn parse_reference(text: Option<&str>) -> Result<BcReference, String> {
    match text.unwrap_or("ICAO").to_ascii_lowercase().as_str() {
        "icao" => Ok(BcReference::Icao),
        "army" | "metro" | "army-standard-metro" => Ok(BcReference::ArmyStandardMetro),
        other => Err(format!("unknown BC reference '{other}'")),
    }
}

impl ScopeProfile {
    fn parse(self) -> Result<Click, String> {
        Ok(Click {
            value_rad: parse_quantity(&self.click, Dimension::Angle, None)
                .map_err(|e| e.to_string())?,
            tracking_factor: self.tracking_factor,
            travel_up_rad: optional_angle(self.travel_up)?,
            travel_down_rad: optional_angle(self.travel_down)?,
            travel_right_rad: optional_angle(self.travel_right)?,
            travel_left_rad: optional_angle(self.travel_left)?,
        })
    }
}

impl AtmoProfile {
    fn parse(self) -> Result<Atmosphere, String> {
        atmosphere_from_parts(
            &self.temperature,
            &self.pressure,
            &self.pressure_kind,
            &self.humidity.to_string(),
            &self.altitude,
        )
    }
}

fn atmosphere_from_parts(
    temperature: &str,
    pressure: &str,
    pressure_kind: &str,
    humidity: &str,
    altitude: &str,
) -> Result<Atmosphere, String> {
    let mut humidity: f64 = humidity
        .trim()
        .trim_end_matches('%')
        .parse()
        .map_err(|_| "humidity must be a number")?;
    if humidity > 1.0 {
        humidity /= 100.0;
    }
    Ok(Atmosphere {
        temperature_c: parse_quantity(temperature, Dimension::Temperature, Some("C"))
            .map_err(|e| e.to_string())?,
        pressure_pa: parse_quantity(pressure, Dimension::Pressure, Some("hPa"))
            .map_err(|e| e.to_string())?,
        pressure_kind: match pressure_kind.to_ascii_lowercase().as_str() {
            "station" => PressureKind::Station,
            "sea-level" | "sealevel" | "qnh" => PressureKind::SeaLevel,
            other => {
                return Err(format!(
                    "pressure_kind must be station or sea-level, not {other}"
                ))
            }
        },
        humidity,
        altitude_m: length(altitude)?,
    })
}

fn optional_temp(value: Option<String>) -> Result<Option<f64>, String> {
    value
        .map(|value| {
            parse_quantity(&value, Dimension::Temperature, Some("C")).map_err(|e| e.to_string())
        })
        .transpose()
}

fn optional_angle(value: Option<String>) -> Result<Option<f64>, String> {
    value
        .map(|value| {
            parse_quantity(&value, Dimension::Angle, Some("mrad")).map_err(|e| e.to_string())
        })
        .transpose()
}
