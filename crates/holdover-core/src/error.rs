use std::fmt;

/// Recoverable solver or parsing failure.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Parse {
        what: &'static str,
        input: String,
        reason: &'static str,
    },
    Invalid(&'static str),
    ZeroNotBracketed,
    ZeroNotConverged,
    RangeNotReached {
        requested_m: f64,
        reached_m: f64,
    },
    NotInMilestone {
        feature: &'static str,
        milestone: &'static str,
    },
    Message(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse {
                what,
                input,
                reason,
            } => {
                write!(f, "could not parse {what} from '{input}': {reason}")
            }
            Error::Invalid(msg) => write!(f, "{msg}"),
            Error::ZeroNotBracketed => {
                write!(f, "zero solve could not bracket the line-of-sight crossing")
            }
            Error::ZeroNotConverged => write!(f, "zero solve did not converge"),
            Error::RangeNotReached {
                requested_m,
                reached_m,
            } => write!(
                f,
                "trajectory stopped at {reached_m:.1} m before the requested {requested_m:.1} m"
            ),
            Error::NotInMilestone { feature, milestone } => {
                write!(f, "{feature} is not implemented until {milestone}")
            }
            Error::Message(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
