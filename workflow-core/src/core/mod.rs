mod id;
mod messaging;
mod options;
mod states;

use std::fmt::Display;

pub use id::*;
pub use messaging::*;
pub use options::*;
pub use states::*;

use serde::{Deserialize, Serialize};

use crate::Error;

/// A timestamp being used for tracking the time of events.
pub type TimeStamp = chrono::DateTime<chrono::Local>;

/// A set of input parameters for a job or task consisting of key-value pairs.
pub type ParameterSet = std::collections::HashMap<String, String>;

/// A convenient trait for creating a parameter set from a list of key-value pairs.
pub trait ToParameterSet {
    fn to_params(self) -> ParameterSet;
}

impl<I, K, V> ToParameterSet for I
where
    I: IntoIterator<Item = (K, V)>,
    K: ToString,
    V: ToString,
{
    fn to_params(self) -> ParameterSet {
        self.into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }
}

/// The status of a job or task.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub enum Status {
    NotStarted = 0,
    Queued = 1,
    Running = 2,
    Finished = 3,
    Failed = 4,
}

impl Status {
    /// Returns true if the status is either `Finished` or `Failed`.
    #[inline]
    pub fn is_done(self) -> bool {
        matches!(self, Status::Finished | Status::Failed)
    }
}

impl TryFrom<i32> for Status {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Status::NotStarted),
            1 => Ok(Status::Queued),
            2 => Ok(Status::Running),
            3 => Ok(Status::Finished),
            4 => Ok(Status::Failed),
            _ => Err(Error::InvalidStatusCode(value)),
        }
    }
}

impl From<Status> for i32 {
    fn from(state: Status) -> Self {
        match state {
            Status::NotStarted => 0,
            Status::Queued => 1,
            Status::Running => 2,
            Status::Finished => 3,
            Status::Failed => 4,
        }
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = match self {
            Status::NotStarted => "Not Started",
            Status::Queued => "Queued",
            Status::Running => "Running",
            Status::Finished => "Finished",
            Status::Failed => "Failed",
        };

        write!(f, "{}", status)
    }
}

/// The state of a job that consists of its status and the current stage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq)]
pub struct JobState {
    pub status: Status,
    pub stage: usize,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_status_conversion() {
        let all_statuses = vec![
            Status::NotStarted,
            Status::Queued,
            Status::Running,
            Status::Finished,
            Status::Failed,
        ];

        for status in all_statuses {
            let i = i32::from(status);
            let converted_status = Status::try_from(i).unwrap();
            assert_eq!(status, converted_status);
        }
    }
}
