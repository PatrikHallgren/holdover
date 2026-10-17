//! Ballistic solver for Holdover.
//!
//! This crate has no user interface and does not read or write files. Outputs
//! are model predictions. Confirm them with live fire before relying on them.

mod atmosphere;
mod constants;
mod drag;
mod error;
mod pchip;
mod solve;
mod stability;
mod targets;
mod traj;
mod units;

pub use atmosphere::{AirState, Atmosphere, PressureKind};
pub use constants::BC_LB_PER_IN2_TO_KG_PER_M2;
pub use drag::{BcBand, BcReference, CdSample, DragModel, DragSpec};
pub use error::{Error, Result};
pub use solve::{
    group_stats, lead, pbr, solve, solve_zero, table, true_drag, true_mv, AngularHold, Bullet,
    Click, Direction, Effects, Integrator, Shot, Solution, TableSpec, TrajectoryPoint, Warning,
    ZeroSolution,
};
pub use targets::{SJF_MOOSE_RING_DIAMETER_CM, SJF_ROE_BUCK_RING_DIAMETER_CM};
pub use units::{parse_quantity, parse_twist, parse_wind, Dimension, Twist, TwistHand, Wind};

pub const DISCLAIMER: &str = "All values are predictions from a mathematical model and your inputs. Confirm with live fire before relying on them. You are responsible for safe and lawful shooting.";
