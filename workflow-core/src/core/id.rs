use std::fmt::Display;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unique identifier used for all kind of entities.
#[derive(Debug, Serialize, Clone, Copy, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id {
    value: Uuid,
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl Id {
    pub fn new() -> Self {
        let value = Uuid::new_v4();
        Self { value }
    }

    pub fn into_inner(self) -> Uuid {
        self.value
    }
}

impl From<Uuid> for Id {
    fn from(value: Uuid) -> Self {
        Self { value }
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn test_creation() {
        const NUM_SAMPLES: usize = 10000;
        let mut ids = HashSet::with_capacity(NUM_SAMPLES);
        for _ in 0..NUM_SAMPLES {
            ids.insert(Id::new());
        }

        assert_eq!(ids.len(), NUM_SAMPLES);
    }
}
