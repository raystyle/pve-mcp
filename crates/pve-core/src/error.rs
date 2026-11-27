//! Domain error types.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, DomainError>;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid vmid {0}: {1}")]
    Vmid(u32, &'static str),
    #[error("invalid node name {0:?}: {1}")]
    NodeName(String, &'static str),
    #[error("invalid storage name {0:?}: {1}")]
    StorageName(String, &'static str),
    #[error("invalid snapshot name {0:?}: {1}")]
    SnapshotName(String, &'static str),
    #[error("invalid UPID {0:?}: {1}")]
    Upid(String, &'static str),
    #[error("invalid vmid range {0:?}")]
    VmidRange(String),
    #[error("config error: {0}")]
    ConfigIo(String),
}
