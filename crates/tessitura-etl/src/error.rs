//! Enrichment error types for the ETL pipeline.

use thiserror::Error;

/// Errors that can occur during enrichment stages.
#[derive(Debug, Error)]
pub enum EnrichError {
    /// An HTTP request to an external source failed.
    #[error("HTTP error from {source_name}: {message}")]
    Http {
        source_name: String,
        message: String,
    },

    /// The external source returned a rate-limit response.
    #[error("rate limited by {source_name}")]
    RateLimited { source_name: String },

    /// The requested entity was not found at the external source.
    #[error("not found: {entity} at {source_name}")]
    NotFound { entity: String, source_name: String },

    /// A response from an external source could not be parsed.
    #[error("parse error from {source_name}: {message}")]
    Parse {
        source_name: String,
        message: String,
    },

    /// An error propagated from `reqwest`.
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    /// An error propagated from the core domain layer.
    #[error("database error: {0}")]
    Database(#[from] tessitura_core::Error),

    /// The circuit breaker for a source is open.
    #[error("circuit open for {source_name}")]
    CircuitOpen { source_name: String },
}

impl EnrichError {
    /// Returns `true` when the error is transient and the operation may
    /// succeed if retried.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Http { .. } | Self::RateLimited { .. })
    }

    /// Returns `true` when the error indicates the entity was not found.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound { .. })
    }
}

/// Convenience alias for enrichment results.
pub type EnrichResult<T> = std::result::Result<T, EnrichError>;
