mod id;
mod states;

pub use id::*;
pub use states::*;

use serde::{Deserialize, Serialize};

/// A timestamp being used for tracking the time of events.
pub type TimeStamp = chrono::DateTime<chrono::Local>;

/// A set of input parameters for a job or task consisting of key-value pairs.
pub type ParameterSet = std::collections::HashMap<String, String>;

/// The status of a job or task.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub enum Status {
    NotStarted,
    Queued,
    Finished,
    Failed,
}

/// The state of a job that consists of its status and the current stage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq)]
pub struct JobState {
    pub status: Status,
    pub stage: usize,
}
