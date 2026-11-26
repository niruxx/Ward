use crate::model::*;
use std::process::Command;
pub fn ufw_path() -> Result<&'static str, String> {
    ["/usr/sbin/ufw", "/sbin/ufw"]
        .into_iter()
        .find(|p| std::path::Path::new(p).is_file())
        .ok_or_else(|| "UFW is not installed.".into())
}
fn run(args: &[&str]) -> Result<String, String> {
    let o = Command::new(ufw_path()?)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}
pub fn parse_rules(text: &str) -> Vec<Rule> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let tail = line.strip_prefix('[')?;
            let (id, description) = tail.split_once(']')?;
            let id: u32 = id.trim().parse().ok()?;
            let pair = description
                .split_whitespace()
                .next()
                .and_then(|s| s.split_once('/'));
            Some(Rule {
                id: id.to_string(),
                description: description.trim().into(),
                port: pair
                    .filter(|(_, p)| ["tcp", "udp"].contains(p))
                    .map(|(p, _)| p.replace(':', "-")),
                protocol: pair
                    .filter(|(_, p)| ["tcp", "udp"].contains(p))
                    .map(|(_, p)| p.to_string()),
            })
        })
        .collect()
}
pub fn status() -> Result<Snapshot, String> {
    let numbered = run(&["status", "numbered"])?;
    let verbose = run(&["status", "verbose"])?;
    let profiles = run(&["app", "list"]).unwrap_or_else(|e| format!("Profiles unavailable: {e}"));
    let ipv6 = std::fs::read_to_string("/etc/default/ufw")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("IPV6="))
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "IPv6 configuration unavailable".into());
    Ok(Snapshot {
        backend: "UFW".into(),
        active: numbered.lines().any(|l| l == "Status: active"),
        rules: parse_rules(&numbered),
        details: format!("{verbose}\n{ipv6}\n{profiles}"),
        revision: format!("{numbered}\n{verbose}"),
        zone: String::new(),
    })
}
pub fn execute(request: Request) -> Result<Snapshot, String> {
    if matches!(request, Request::Status {}) {
        return status();
    }
    super::firewalld::ensure_not_running()?;
    let current = status()?;
    let expected = request.expected().ok_or("Missing configuration revision")?;
    if expected != current.revision {
        return Err("Firewall changed externally. Refresh before making changes.".into());
    }
    match request {
        Request::SetEnabled { enabled, .. } => {
            run(&["--force", if enabled { "enable" } else { "disable" }])?;
        }
        Request::Add { rule, .. } => {
            rule.validate()?;
            let port = rule.port.replace('-', ":");
            run(&[
                &rule.action,
                &rule.direction,
                "proto",
                &rule.protocol,
                "from",
                &rule.source,
                "to",
                &rule.destination,
                "port",
                &port,
            ])?;
        }
        Request::Profile { name, action, .. } => {
            if !["allow", "deny", "reject"].contains(&action.as_str()) {
                return Err("Invalid profile action.".into());
            }
            let list = run(&["app", "list"])?;
            if name.is_empty()
                || name.starts_with('-')
                || !list
                    .lines()
                    .filter(|l| l.starts_with("  "))
                    .any(|l| l.trim() == name)
            {
                return Err("Choose an installed UFW application profile.".into());
            }
            run(&[&action, "in", &name])?;
        }
        Request::Delete { id, .. } => {
            let n: u32 = id.parse().map_err(|_| "Invalid rule number.")?;
            if !current.rules.iter().any(|r| r.id == n.to_string()) {
                return Err("Rule no longer exists.".into());
            }
            run(&["--force", "delete", &n.to_string()])?;
        }
        Request::Policy {
            direction, policy, ..
        } => {
            if !["incoming", "outgoing"].contains(&direction.as_str())
                || !["allow", "deny", "reject"].contains(&policy.as_str())
            {
                return Err("Invalid policy.".into());
            }
            run(&["default", &policy, &direction])?;
        }
        Request::Logging { level, .. } => {
            if !["off", "low", "medium", "high", "full"].contains(&level.as_str()) {
                return Err("Invalid logging level.".into());
            }
            run(&["logging", &level])?;
        }
        Request::Status {} => {}
        _ => return Err("This operation is specific to firewalld.".into()),
    }
    status()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbered() {
        let r=parse_rules("Status: active\n[ 1] 22/tcp ALLOW IN Anywhere\n[12] 443/tcp (v6) ALLOW IN Anywhere (v6)\n");
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].id, "12");
    }
    #[test]
    fn inactive() {
        assert!(parse_rules("Status: inactive").is_empty());
    }
}
