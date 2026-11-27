//! pve-core — domain types and PVE API client core for pve-mcp.
//!
//! Domain values are branded newtypes validated once at the boundaries
//! (MCP tool args, TOML config, API responses) and passed unvalidated
//! thereafter. See `.claude/rules/rust-dev.md`.

pub mod client;
pub mod config;
pub mod domain;
pub mod error;

pub use domain::{GuestRef, GuestType, NodeName, SnapshotName, StorageName, Upid, Vmid, VmidRange};
pub use error::{DomainError, Result};
