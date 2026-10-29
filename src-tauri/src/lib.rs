pub mod community_identity;
pub mod community_host;
pub mod community_backup;
mod app_ownership;
pub mod community_connector;
pub mod application;
pub mod domain;
pub mod kernel;
pub mod persistence;
pub mod providers;
pub mod vault;
pub mod imports;
pub mod backup;
pub mod diagnostics;
pub mod experimental_runtime;
pub mod experimental_continuity;
pub mod experimental_internal_state;
pub mod experimental_reflection;
mod export;
mod deletion;
pub mod memory;
pub mod retrieval;
mod connections;
#[cfg(feature="desktop")]
mod desktop;
#[cfg(feature="desktop")]
pub use desktop::run;
