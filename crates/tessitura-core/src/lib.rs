//! Core domain model for tessitura.
//!
//! This crate defines the FRBR-rooted data model (Work, Expression,
//! Manifestation, Item), the SQLite schema, taxonomy types, and
//! provenance tracking.

#![deny(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod error;
pub mod model;
pub mod provenance;
pub mod schema;
pub mod taxonomy;

pub use error::{Error, Result};
