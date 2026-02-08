//! ETL pipeline stages for tessitura.
//!
//! Implements the scan, identify, enrich, harmonize, index, and export
//! stages as treadle `Stage` implementations.

#![deny(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod acoustid;
pub mod config;
pub mod enrich;
pub mod error;
pub mod harmonize;
pub mod identify;
pub mod musicbrainz;
pub mod pipeline;
pub mod scan;
pub mod work_item;

pub use config::Config;
pub use enrich::stage::EnrichStage;
pub use error::{EnrichError, EnrichResult};
pub use harmonize::HarmonizeStage;
pub use identify::IdentifyStage;
pub use pipeline::{build_full_pipeline, build_pipeline};
pub use scan::ScanStage;
pub use work_item::MusicFile;
