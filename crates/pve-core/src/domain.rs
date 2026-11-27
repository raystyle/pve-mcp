//! Branded domain types. Constructed only via validated TryFrom at the
//! boundaries; invariants hold for every value in circulation.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::{DomainError, Result};

/// PVE guest ID. PVE accepts 100..=999999 for new guests; values outside
/// are rejected at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Vmid(u32);

impl Vmid {
    pub const MIN: u32 = 100;
    pub const MAX: u32 = 999_999;

    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for Vmid {
    type Error = DomainError;

    fn try_from(v: u32) -> Result<Self> {
        if (Self::MIN..=Self::MAX).contains(&v) {
            Ok(Self(v))
        } else {
            Err(DomainError::Vmid(v, "must be within 100..=999999"))
        }
    }
}

impl TryFrom<i64> for Vmid {
    type Error = DomainError;

    fn try_from(v: i64) -> Result<Self> {
        u32::try_from(v)
            .ok()
            .and_then(|v| Vmid::try_from(v).ok())
            .ok_or(DomainError::Vmid(v as u32, "must be a valid guest id"))
    }
}

impl fmt::Display for Vmid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Inclusive VMID range for protected_vmids config: `100,101,9000-9010`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmidRange {
    lo: Vmid,
    hi: Vmid,
}

impl VmidRange {
    pub fn new(lo: Vmid, hi: Vmid) -> Result<Self> {
        if lo <= hi {
            Ok(Self { lo, hi })
        } else {
            Err(DomainError::VmidRange(format!("{lo}-{hi}")))
        }
    }

    pub fn contains(&self, v: Vmid) -> bool {
        self.lo <= v && v <= self.hi
    }
}

/// Inclusive VMID allow/deny set parsed from config `protected_vmids`:
/// comma-separated ids and ranges, e.g. `"100,106,140-141"`.
#[derive(Debug, Clone, Default)]
pub struct VmidSet {
    entries: Vec<VmidRange>,
}

impl VmidSet {
    pub fn parse(spec: &str) -> Result<Self> {
        let mut entries = Vec::new();
        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let entry = match part.split_once('-') {
                Some((lo, hi)) => {
                    let lo: Vmid = lo
                        .trim()
                        .parse::<u32>()
                        .ok()
                        .and_then(|v| Vmid::try_from(v).ok())
                        .ok_or(DomainError::VmidRange(part.to_string()))?;
                    let hi: Vmid = hi
                        .trim()
                        .parse::<u32>()
                        .ok()
                        .and_then(|v| Vmid::try_from(v).ok())
                        .ok_or(DomainError::VmidRange(part.to_string()))?;
                    VmidRange::new(lo, hi)?
                }
                None => {
                    let v: Vmid = part
                        .parse::<u32>()
                        .ok()
                        .and_then(|v| Vmid::try_from(v).ok())
                        .ok_or(DomainError::VmidRange(part.to_string()))?;
                    VmidRange::new(v, v)?
                }
            };
            entries.push(entry);
        }
        Ok(Self { entries })
    }

    pub fn contains(&self, v: Vmid) -> bool {
        self.entries.iter().any(|e| e.contains(v))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// PVE node name: non-empty, no whitespace, no URL-hostile characters
/// (node names flow into API path segments).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeName(String);

impl NodeName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NodeName {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() {
            return Err(DomainError::NodeName(s, "must not be empty"));
        }
        // Same dot-segment hazard as SnapshotName: the name lands in URL
        // path segments; a ".." node would redirect API calls elsewhere.
        if s == "." || s == ".." {
            return Err(DomainError::NodeName(s, "must not be a dot-segment"));
        }
        if s.chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '%'))
        {
            return Err(DomainError::NodeName(
                s,
                "must not contain whitespace or path-hostile characters",
            ));
        }
        Ok(Self(s))
    }
}

impl TryFrom<&str> for NodeName {
    type Error = DomainError;

    fn try_from(s: &str) -> Result<Self> {
        Self::try_from(s.to_string())
    }
}

impl fmt::Display for NodeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// PVE storage name: same path-segment constraints as node names.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageName(String);

impl StorageName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for StorageName {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() {
            return Err(DomainError::StorageName(s, "must not be empty"));
        }
        if s == "." || s == ".." {
            return Err(DomainError::StorageName(s, "must not be a dot-segment"));
        }
        if s.chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '%'))
        {
            return Err(DomainError::StorageName(
                s,
                "must not contain whitespace or path-hostile characters",
            ));
        }
        Ok(Self(s))
    }
}

impl fmt::Display for StorageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Snapshot name: PVE disallows some characters (spaces are tolerated by the
/// API but break scripting; we reject them at the boundary).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotName(String);

impl SnapshotName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SnapshotName {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        if s.is_empty() || s == "current" {
            return Err(DomainError::SnapshotName(
                s,
                "must be non-empty and not 'current'",
            ));
        }
        // Dot-segments: the name flows raw into a URL path segment
        // ("/snapshot/<name>"), and WHATWG normalization turns ".." into
        // a parent hop — a crafted name would DELETE the guest itself.
        // '?'/'#' inject query/fragment; '%' breaks percent-decoding.
        if s == "." || s == ".." || s.starts_with("./") || s.starts_with("../") {
            return Err(DomainError::SnapshotName(s, "must not be a dot-segment"));
        }
        if s.chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | ',' | '=' | '?' | '#' | '%'))
        {
            return Err(DomainError::SnapshotName(
                s,
                "must not contain whitespace, '/', ',', '=', '?', '#' or '%'",
            ));
        }
        Ok(Self(s))
    }
}

impl fmt::Display for SnapshotName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// PVE task UPID, e.g. `UPID:lan-pve:0000ABCD:00001234:00000000:qemustart:102:root@pam:`.
/// Parses out the parts tools need (node, target guest, start time).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Upid(String);

impl Upid {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Upid {
    type Error = DomainError;

    fn try_from(s: String) -> Result<Self> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 9 || parts[0] != "UPID" || parts[1].is_empty() {
            return Err(DomainError::Upid(
                s,
                "expected UPID:<node>:<pstart>:<pid>:<starttime>:<dtype>:<id>:<user>:",
            ));
        }
        // pid/starttime are hex per pve-common's UPID format
        // (e.g. UPID:node:0016C602:116DC4AD:...:qmstart:104:root@pam:).
        if u32::from_str_radix(parts[3], 16).is_err() {
            return Err(DomainError::Upid(s, "pid field is not hex"));
        }
        Ok(Self(s))
    }
}

impl fmt::Display for Upid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Guest type: qemu VM or LXC container. Exhaustive by definition —
/// matches must not fall through to a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestType {
    Qemu,
    Lxc,
}

impl GuestType {
    pub fn as_api_segment(self) -> &'static str {
        match self {
            Self::Qemu => "qemu",
            Self::Lxc => "lxc",
        }
    }
}

impl fmt::Display for GuestType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qemu => f.write_str("qemu"),
            Self::Lxc => f.write_str("lxc"),
        }
    }
}

/// Locates one guest: node + type + vmid. The only way tools address a guest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuestRef {
    pub node: NodeName,
    pub ty: GuestType,
    pub vmid: Vmid,
}

impl GuestRef {
    pub fn new(node: NodeName, ty: GuestType, vmid: Vmid) -> Self {
        Self { node, ty, vmid }
    }

    /// API path segment for this guest, e.g. `nodes/lan-pve/qemu/102`.
    pub fn api_path(&self) -> String {
        format!(
            "nodes/{}/{}/{}",
            self.node,
            self.ty.as_api_segment(),
            self.vmid
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmid_accepts_valid_range() {
        assert_eq!(Vmid::try_from(100u32).unwrap().get(), 100);
        assert_eq!(Vmid::try_from(999_999u32).unwrap().get(), 999_999);
    }

    #[test]
    fn vmid_rejects_out_of_range() {
        assert!(Vmid::try_from(0u32).is_err());
        assert!(Vmid::try_from(99u32).is_err());
        assert!(Vmid::try_from(1_000_000u32).is_err());
    }

    #[test]
    fn vmid_from_i64_rejects_negative() {
        assert!(Vmid::try_from(-1i64).is_err());
        assert_eq!(Vmid::try_from(102i64).unwrap().get(), 102);
    }

    #[test]
    fn vmid_range_contains_inclusive() {
        let r = VmidRange::new(
            Vmid::try_from(140u32).unwrap(),
            Vmid::try_from(141u32).unwrap(),
        )
        .unwrap();
        assert!(r.contains(Vmid::try_from(140u32).unwrap()));
        assert!(r.contains(Vmid::try_from(141u32).unwrap()));
        assert!(!r.contains(Vmid::try_from(139u32).unwrap()));
        assert!(!r.contains(Vmid::try_from(142u32).unwrap()));
    }

    #[test]
    fn vmid_range_rejects_reversed() {
        assert!(VmidRange::new(
            Vmid::try_from(141u32).unwrap(),
            Vmid::try_from(140u32).unwrap()
        )
        .is_err());
    }

    #[test]
    fn vmid_set_parses_mixed_spec() {
        let set = VmidSet::parse("100, 106, 140-141").unwrap();
        assert!(set.contains(Vmid::try_from(100u32).unwrap()));
        assert!(set.contains(Vmid::try_from(106u32).unwrap()));
        assert!(set.contains(Vmid::try_from(140u32).unwrap()));
        assert!(set.contains(Vmid::try_from(141u32).unwrap()));
        assert!(!set.contains(Vmid::try_from(102u32).unwrap()));
        assert!(!set.is_empty());
    }

    #[test]
    fn vmid_set_rejects_garbage() {
        assert!(VmidSet::parse("abc").is_err());
        assert!(VmidSet::parse("99").is_err());
        assert!(VmidSet::parse("141-140").is_err());
        assert!(VmidSet::parse("").unwrap().is_empty());
    }

    #[test]
    fn node_name_accepts_plain() {
        assert_eq!(NodeName::try_from("lan-pve").unwrap().as_str(), "lan-pve");
    }

    #[test]
    fn node_name_rejects_hostile() {
        assert!(NodeName::try_from("").is_err());
        assert!(NodeName::try_from("lan pve").is_err());
        assert!(NodeName::try_from("a/b").is_err());
        assert!(NodeName::try_from("a?b").is_err());
    }

    #[test]
    fn storage_name_rejects_hostile() {
        assert!(StorageName::try_from("local-lvm".to_string()).is_ok());
        assert!(StorageName::try_from(String::new()).is_err());
    }

    #[test]
    fn snapshot_name_rejects_dot_segment_and_url_injection() {
        // Review 0003 blocking: ".." flows into "/snapshot/<name>" and
        // WHATWG normalization turns it into a parent hop — DELETE would
        // destroy the guest itself. The exact attack vector, held green.
        for name in [
            "..",
            ".",
            "../x",
            "x/../y",
            "snap?query",
            "snap#frag",
            "100%2e%2e",
        ] {
            assert!(
                SnapshotName::try_from(name.to_string()).is_err(),
                "injection name {name:?} must be rejected"
            );
        }
    }

    #[test]
    fn node_and_storage_names_reject_dot_segments() {
        assert!(NodeName::try_from("..").is_err());
        assert!(StorageName::try_from("..".to_string()).is_err());
    }

    #[test]
    fn snapshot_name_rejects_current_and_spaces() {
        assert!(SnapshotName::try_from("pre-clone".to_string()).is_ok());
        assert!(SnapshotName::try_from("current".to_string()).is_err());
        assert!(SnapshotName::try_from("my snap".to_string()).is_err());
        assert!(SnapshotName::try_from(String::new()).is_err());
    }

    #[test]
    fn upid_parses_valid() {
        // Real captured UPID from PVE 9.2 (hex pid + hex starttime).
        let raw = "UPID:lan-pve:0016C602:116DC4AD:6B091185:qmstart:104:root@pam:";
        let upid = Upid::try_from(raw.to_string()).unwrap();
        assert_eq!(upid.as_str(), raw);
    }

    #[test]
    fn upid_rejects_malformed() {
        assert!(Upid::try_from("not-a-upid".to_string()).is_err());
        assert!(Upid::try_from("UPID:lan-pve:x:y:z:qemustart:102:root@pam:".to_string()).is_err());
    }

    #[test]
    fn guest_type_api_segments_exhaustive() {
        // Compile-time exhaustiveness guard: adding a variant breaks this match.
        let segments = [
            GuestType::Qemu.as_api_segment(),
            GuestType::Lxc.as_api_segment(),
        ];
        assert_eq!(segments, ["qemu", "lxc"]);
        assert_eq!(GuestType::Qemu.to_string(), "qemu");
        assert_eq!(GuestType::Lxc.to_string(), "lxc");
    }

    #[test]
    fn guest_ref_holds_valid_parts() {
        let r = GuestRef::new(
            NodeName::try_from("lan-pve").unwrap(),
            GuestType::Qemu,
            Vmid::try_from(102u32).unwrap(),
        );
        assert_eq!(r.vmid.get(), 102);
        assert_eq!(r.node.as_str(), "lan-pve");
    }
}
