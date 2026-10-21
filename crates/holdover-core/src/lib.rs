//! Ballistic solver for Holdover.
//!
//! This crate has no user interface and does not read or write files. Outputs
//! are model predictions. Confirm them with live fire before relying on them.

mod atmosphere;
mod chart_axis;
mod constants;
mod drag;
mod error;
mod field;
mod pchip;
mod pdf;
mod reticle;
mod solve;
mod stability;
mod sync;
mod targets;
mod traj;
mod units;

pub use atmosphere::{AirState, Atmosphere, PressureKind};
pub use chart_axis::{chart_axis, chart_tick_step, format_tick, ChartAxis};
pub use constants::BC_LB_PER_IN2_TO_KG_PER_M2;
pub use drag::{BcBand, BcReference, CdSample, CdScale, DragModel, DragSpec};
pub use error::{Error, Result};
pub use field::{
    card_page, card_pages, chrono_stats, combine_offsets, default_card_page, lead, pbr, pbr_fixed,
    powder_coefficient, print_sheet_css, recoil, running_presets, scale_bar_mm, true_drag, true_mv,
    true_tracking, vertical_mv_spread_m, CardPage, Chrono, DragFit, DragMode, Lead, MvFit, Pbr,
    PbrSample, PowderFit, Recoil, RecoilInput, RunningPreset, DEFAULT_EJECTA_MPS, DRAG_GUIDANCE,
    EJECTA_CONVENTION, MV_GUIDANCE, TRACKING_GUIDANCE,
};
pub use pdf::{
    render_card_pdf, render_target_pdf, round_pt, CardSheet, PrintedTarget, TargetPath, TargetPrint,
};
pub use reticle::{
    angular_mrad, cm_per_mrad, generic_by_id, generic_reticles, marks_true_mrad, mirror_reticle,
    parse_reticle, repeat_reticle, reticle_json, sfp_warning, snap_value, subtension_factor,
    target_diameter_px, to_mrad, Eyepiece, FocalPlane, Mark, MirrorAxis, Reticle, ReticleUnit,
    DEFAULT_APPARENT_FOV_DEG,
};
pub use solve::{
    group_stats, solve, solve_zero, table, AngularHold, Bullet, Click, Direction, Effects,
    Integrator, Shot, Solution, TableSpec, TrajectoryPoint, Warning, ZeroSolution,
};
pub use stability::SPIN_DRIFT_PROVENANCE;
pub use sync::{
    bundle_doc_value, choices_for, classify, classify_pair, comparable_body, copied_record, effect,
    is_deleted, merge_plan, merge_private, parse_bundle_doc, record_label, record_value,
    referenced_blobs, strip_private, BundleDoc, CopyStamp, Effect, MergeRow, MergeStatus,
    Resolution, SeenRevision, SyncBase, SyncError, SyncRecord, BUNDLE_FORMAT,
    BUNDLE_SCHEMA_VERSION, RECORD_KINDS,
};
pub use targets::{
    apply_target_edit, bounds_cm, library_value, mirror_paths, sanitize_svg, target_by_id,
    target_json, target_library, FlagDoc, PathDoc, RingDoc, TargetDoc, MOOSE_HIT_AREA_CM,
    SJF_MOOSE_RING_DIAMETER_CM, SJF_ROE_BUCK_RING_DIAMETER_CM,
};
pub use units::{
    convert_from_si, format_number, format_quantity, parse_quantity, parse_twist, parse_wind,
    parse_wind_direction, Dimension, Twist, TwistHand, Wind,
};

pub const DISCLAIMER: &str = "All values are predictions from a mathematical model and your inputs. Confirm with live fire before relying on them. You are responsible for safe and lawful shooting.";
