use serde::{Deserialize, Serialize};

/// A musical period or era.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Period {
    pub name: String,
    /// Approximate start year.
    pub start_year: Option<i32>,
    /// Approximate end year.
    pub end_year: Option<i32>,
}

impl Period {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start_year: None,
            end_year: None,
        }
    }

    #[must_use]
    pub fn with_range(mut self, start: i32, end: i32) -> Self {
        self.start_year = Some(start);
        self.end_year = Some(end);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_period_new() {
        let period = Period::new("Baroque");
        assert_eq!(period.name, "Baroque");
        assert!(period.start_year.is_none());
    }

    #[test]
    fn test_period_with_range() {
        let period = Period::new("Baroque").with_range(1600, 1750);
        assert_eq!(period.start_year, Some(1600));
        assert_eq!(period.end_year, Some(1750));
    }
}
