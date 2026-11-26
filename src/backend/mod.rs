pub mod firewalld;
pub mod ufw;
use crate::model::*;
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ufw,
    Firewalld,
    Mock,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ufw => "UFW",
            Self::Firewalld => "firewalld",
            Self::Mock => "Mock · no host changes",
        }
    }
}
pub fn detect() -> Vec<Kind> {
    let mut found = Vec::new();
    if ufw::ufw_path().is_ok() {
        found.push(Kind::Ufw);
    }
    if firewalld::available() {
        found.push(Kind::Firewalld);
    }
    found
}
pub fn execute(kind: Kind, request: Request) -> Result<Snapshot, String> {
    match kind {
        Kind::Ufw => {
            let output = Command::new("/usr/bin/pkexec")
                .arg("/usr/libexec/ward-helper")
                .arg(serde_json::to_string(&request).map_err(|e| e.to_string())?)
                .env_clear()
                .env("PATH", "/usr/sbin:/usr/bin")
                .output()
                .map_err(|e| format!("Unable to request administrator authorization: {e}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Authorization or UFW operation failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            serde_json::from_slice(&output.stdout)
                .map_err(|e| format!("Invalid helper response: {e}"))
        }
        Kind::Firewalld => firewalld::execute(request),
        Kind::Mock => mock(request),
    }
}
fn mock(request: Request) -> Result<Snapshot, String> {
    use std::sync::Mutex;
    static STATE: Mutex<Option<Snapshot>> = Mutex::new(None);
    let mut state = STATE.lock().map_err(|_| "Mock state unavailable")?;
    let s = state.get_or_insert_with(|| Snapshot {
        backend: "Mock".into(),
        active: false,
        rules: vec![],
        details: "Simulation only. Incoming: deny; outgoing: allow.".into(),
        revision: "0".into(),
        zone: String::new(),
    });
    match request {
        Request::Status {} => {}
        Request::SetEnabled { enabled, .. } => s.active = enabled,
        Request::Add { rule, .. } => {
            rule.validate()?;
            s.rules.push(Rule {
                id: s.revision.clone(),
                description: format!(
                    "{} {} {}/{} from {}",
                    rule.action, rule.direction, rule.port, rule.protocol, rule.source
                ),
                port: Some(rule.port),
                protocol: Some(rule.protocol),
            });
        }
        Request::Delete { id, .. } => s.rules.retain(|r| r.id != id),
        _ => return Err("This operation is unavailable in mock mode.".into()),
    }
    s.revision = (s.revision.parse::<u64>().unwrap_or(0) + 1).to_string();
    Ok(s.clone())
}
