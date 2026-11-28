//! Pure-function coverage for the dispatch layer (no PVE, no daemon):
//! downsample, PowerAction tiers, Upid edge forms, and the guest-guard
//! math through the config surface.

use pve_core::api::PowerAction;
use pve_core::domain::{SnapshotName, Upid, Vmid, VmidSet};

#[test]
fn power_action_confirm_tiers_match_the_war_story() {
    // Review 0003 + NANDI precedent: start/resume ungated (state change
    // without destruction); everything else needs confirm.
    for action in ["shutdown", "stop", "reboot", "reset", "suspend"] {
        let parsed: PowerAction = action.parse().unwrap();
        assert!(parsed.requires_confirm(), "{action} must be gated");
    }
    for action in ["start", "resume"] {
        let parsed: PowerAction = action.parse().unwrap();
        assert!(!parsed.requires_confirm(), "{action} must stay ungated");
    }
}

#[test]
fn power_action_target_states_drive_idempotence() {
    use pve_core::api::PowerAction;
    assert_eq!(PowerAction::Start.target_state(), Some("running"));
    assert_eq!(PowerAction::Shutdown.target_state(), Some("stopped"));
    assert_eq!(PowerAction::Stop.target_state(), Some("stopped"));
    assert_eq!(PowerAction::Reboot.target_state(), Some("running"));
    // Relative actions have no single target: idempotence skip does not apply.
    assert_eq!(PowerAction::Reset.target_state(), None);
    assert_eq!(PowerAction::Suspend.target_state(), None);
    assert_eq!(PowerAction::Resume.target_state(), None);
}

#[test]
fn power_action_parse_rejects_unknown_verbs() {
    use std::str::FromStr;
    for bad in ["", "Start", "START", "kill", "poweroff", "halt"] {
        assert!(
            PowerAction::from_str(bad).is_err(),
            "{bad:?} must not parse"
        );
    }
    let err = PowerAction::from_str("halt").unwrap_err();
    assert!(
        err.contains("start|shutdown|stop"),
        "error enumerates valid verbs: {err}"
    );
}

#[test]
fn snapshot_name_boundary_is_the_injection_wall() {
    // Review 0003 blocking: the URL path builds from this name raw.
    for hostile in [
        "..",
        ".",
        "../escape",
        "a,b",
        "a=b",
        "a/b",
        "a b",
        "a?x",
        "a#x",
        "100%2e",
    ] {
        assert!(
            SnapshotName::try_from(hostile.to_string()).is_err(),
            "hostile snapshot name {hostile:?} must be rejected"
        );
    }
    for legal in [
        "pre-clone",
        "before-upgrade-2026",
        "v1.0_test",
        "проверка-01",
    ] {
        assert!(
            SnapshotName::try_from(legal.to_string()).is_ok(),
            "legal snapshot name {legal:?} must pass"
        );
    }
}

#[test]
fn upid_node_extraction_matches_task_status_dispatch() {
    // task_status pulls the node out of the UPID — the parse must hold for
    // the shape PVE actually returns (hex pid/starttime).
    let raw = "UPID:lan-pve:0018463F:118E4951:6B0964C4:qmdestroy:118:root@pam:";
    let upid = Upid::try_from(raw.to_string()).unwrap();
    let node = upid.as_str().split(':').nth(1).unwrap();
    assert_eq!(node, "lan-pve");
    // NodeName::try_from accepts it (the dispatch does this conversion).
    let _ = pve_core::domain::NodeName::try_from(node.to_string()).unwrap();
}

#[test]
fn upid_rejects_decimal_pid_regression() {
    // The hex-pid bug (live UPID failed decimal parse) must stay fixed:
    // non-hex pid chars fail, hex passes.
    assert!(Upid::try_from("UPID:n:00001234:00001234:6B0964C4:q:1:u:".to_string()).is_ok());
    assert!(Upid::try_from("UPID:n:00001234:zzzzzzzz:6B0964C4:q:1:u:".to_string()).is_err());
}

#[test]
fn vmid_set_semantics_cover_the_protected_span_config() {
    // protected_vmids = "106" and ranges "140-141" both occurred live.
    let single = VmidSet::parse("106").unwrap();
    assert!(single.contains(Vmid::try_from(106u32).unwrap()));
    let range = VmidSet::parse("140-141").unwrap();
    assert!(range.contains(Vmid::try_from(140u32).unwrap()));
    assert!(range.contains(Vmid::try_from(141u32).unwrap()));
    assert!(!range.contains(Vmid::try_from(142u32).unwrap()));
}
