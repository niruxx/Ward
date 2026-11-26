use ward::{
    backend::{execute, Kind},
    model::{Request, RuleSpec},
};
#[test]
fn mock_lifecycle_and_rejection_do_not_touch_host() {
    let initial = execute(Kind::Mock, Request::Status {}).unwrap();
    assert!(!initial.active);
    let enabled = execute(
        Kind::Mock,
        Request::SetEnabled {
            enabled: true,
            expected: initial.revision,
        },
    )
    .unwrap();
    assert!(enabled.active);
    let rule = RuleSpec {
        port: "443".into(),
        protocol: "tcp".into(),
        action: "allow".into(),
        direction: "in".into(),
        source: "any".into(),
        destination: "any".into(),
    };
    let added = execute(
        Kind::Mock,
        Request::Add {
            rule: rule.clone(),
            expected: enabled.revision,
        },
    )
    .unwrap();
    assert_eq!(added.rules.len(), 1);
    let mut invalid = rule;
    invalid.port = "22; touch /tmp/ward-injection".into();
    assert!(execute(
        Kind::Mock,
        Request::Add {
            rule: invalid,
            expected: added.revision.clone()
        }
    )
    .is_err());
    let removed = execute(
        Kind::Mock,
        Request::Delete {
            id: added.rules[0].id.clone(),
            expected: added.revision,
        },
    )
    .unwrap();
    assert!(removed.rules.is_empty());
}
#[test]
fn reject_unknown_privileged_operations_and_fields() {
    for text in [
        r#"{"operation":"Shell","command":"id"}"#,
        r#"{"operation":"Status","command":"id"}"#,
    ] {
        assert!(serde_json::from_str::<Request>(text).is_err());
    }
}
#[test]
fn reject_unsupported_rule_semantics() {
    let mut r = RuleSpec {
        port: "22".into(),
        protocol: "udp".into(),
        action: "limit".into(),
        direction: "in".into(),
        source: "any".into(),
        destination: "any".into(),
    };
    assert!(r.validate().is_err());
    r.protocol = "tcp".into();
    r.destination = "::/129".into();
    assert!(r.validate().is_err());
}
