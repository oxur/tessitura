use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::ids::{ArtistId, ExpressionId, WorkId};

/// A specific performance or recording of a Work.
///
/// Corresponds to the FRBR "Expression" level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    pub id: ExpressionId,
    pub work_id: WorkId,
    pub title: Option<String>,

    /// `MusicBrainz` recording ID.
    pub musicbrainz_id: Option<String>,

    /// Primary performer IDs (soloists, ensembles).
    pub performer_ids: Vec<ArtistId>,

    /// Conductor, if applicable.
    pub conductor_id: Option<ArtistId>,

    /// Recording year.
    pub recorded_year: Option<i32>,

    /// Duration in seconds.
    pub duration_secs: Option<f64>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Expression {
    #[must_use]
    pub fn new(work_id: WorkId) -> Self {
        let now = Utc::now();
        Self {
            id: ExpressionId::new(),
            work_id,
            title: None,
            musicbrainz_id: None,
            performer_ids: Vec::new(),
            conductor_id: None,
            recorded_year: None,
            duration_secs: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn with_musicbrainz_id(mut self, mbid: impl Into<String>) -> Self {
        self.musicbrainz_id = Some(mbid.into());
        self
    }

    #[must_use]
    pub fn with_performer(mut self, performer_id: ArtistId) -> Self {
        self.performer_ids.push(performer_id);
        self
    }

    #[must_use]
    pub fn with_conductor(mut self, conductor_id: ArtistId) -> Self {
        self.conductor_id = Some(conductor_id);
        self
    }

    #[must_use]
    pub fn with_duration(mut self, secs: f64) -> Self {
        self.duration_secs = Some(secs);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expression_new() {
        let work_id = WorkId::new();
        let expr = Expression::new(work_id);
        assert_eq!(expr.work_id, work_id);
        assert!(expr.title.is_none());
    }

    #[test]
    fn test_expression_builder() {
        let work_id = WorkId::new();
        let performer = ArtistId::new();
        let conductor = ArtistId::new();

        let expr = Expression::new(work_id)
            .with_title("Karajan/BPO 1962")
            .with_performer(performer)
            .with_conductor(conductor)
            .with_duration(2400.5);

        assert_eq!(expr.title, Some("Karajan/BPO 1962".to_string()));
        assert_eq!(expr.performer_ids.len(), 1);
        assert_eq!(expr.conductor_id, Some(conductor));
        assert_eq!(expr.duration_secs, Some(2400.5));
    }
}
