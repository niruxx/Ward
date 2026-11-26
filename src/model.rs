use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuleSpec {
    pub port: String,
    pub protocol: String,
    pub action: String,
    pub direction: String,
    pub source: String,
    pub destination: String,
}
impl RuleSpec {
    pub fn validate(&self) -> Result<(), String> {
        validate_port(&self.port)?;
        if !["tcp", "udp"].contains(&self.protocol.as_str()) {
            return Err("Choose TCP or UDP.".into());
        }
        if !["allow", "deny", "reject", "limit"].contains(&self.action.as_str()) {
            return Err("Invalid action.".into());
        }
        if !["in", "out"].contains(&self.direction.as_str()) {
            return Err("Invalid direction.".into());
        }
        if self.action == "limit" && self.protocol != "tcp" {
            return Err("Rate limiting requires TCP.".into());
        }
        for address in [&self.source, &self.destination] {
            if address != "any" {
                let mut parts = address.split('/');
                let ip: IpAddr = parts
                    .next()
                    .unwrap_or("")
                    .parse()
                    .map_err(|_| "Enter an IP address or CIDR network.")?;
                if let Some(prefix) = parts.next() {
                    let n: u8 = prefix.parse().map_err(|_| "Invalid network prefix.")?;
                    if n > if ip.is_ipv4() { 32 } else { 128 } {
                        return Err("Invalid network prefix.".into());
                    }
                }
                if parts.next().is_some() {
                    return Err("Invalid network address.".into());
                }
            }
        }
        Ok(())
    }
}
pub fn validate_port(port: &str) -> Result<(), String> {
    let values: Vec<_> = port.split('-').collect();
    if values.is_empty() || values.len() > 2 {
        return Err("Enter a port or range such as 8000-8010.".into());
    }
    if values
        .iter()
        .any(|v| v.is_empty() || !v.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err("Ports must contain decimal digits only.".into());
    }
    let numbers: Result<Vec<u16>, _> = values.iter().map(|v| v.parse::<u16>()).collect();
    let numbers = numbers.map_err(|_| "Ports must be between 1 and 65535.")?;
    if numbers.contains(&0) || (numbers.len() == 2 && numbers[0] > numbers[1]) {
        return Err("Invalid port range.".into());
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub description: String,
    pub port: Option<String>,
    pub protocol: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub backend: String,
    pub active: bool,
    pub rules: Vec<Rule>,
    pub details: String,
    pub revision: String,
    pub zone: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub enum Request {
    Status {},
    SaveRuntime {
        expected: String,
    },
    Reload {
        expected: String,
    },
    Service {
        name: String,
        add: bool,
        expected: String,
    },
    Profile {
        name: String,
        action: String,
        expected: String,
    },
    ZoneBinding {
        zone: String,
        value: String,
        source: bool,
        add: bool,
        expected: String,
    },
    SetEnabled {
        enabled: bool,
        expected: String,
    },
    Add {
        rule: RuleSpec,
        expected: String,
    },
    Delete {
        id: String,
        expected: String,
    },
    Policy {
        direction: String,
        policy: String,
        expected: String,
    },
    Logging {
        level: String,
        expected: String,
    },
}

impl Request {
    pub fn expected(&self) -> Option<&str> {
        match self {
            Self::Status {} => None,
            Self::SaveRuntime { expected }
            | Self::Reload { expected }
            | Self::Service { expected, .. }
            | Self::Profile { expected, .. }
            | Self::ZoneBinding { expected, .. }
            | Self::SetEnabled { expected, .. }
            | Self::Add { expected, .. }
            | Self::Delete { expected, .. }
            | Self::Policy { expected, .. }
            | Self::Logging { expected, .. } => Some(expected),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ports() {
        for p in ["22", "1-65535"] {
            assert!(validate_port(p).is_ok());
        }
        for p in ["0", "65536", "2-1", "22;id", "--help", "1-2-3", ""] {
            assert!(validate_port(p).is_err());
        }
    }
    #[test]
    fn addresses() {
        let mut r = RuleSpec {
            port: "22".into(),
            protocol: "tcp".into(),
            action: "allow".into(),
            direction: "in".into(),
            source: "2001:db8::/64".into(),
            destination: "any".into(),
        };
        assert!(r.validate().is_ok());
        r.source = "1.2.3.4/33".into();
        assert!(r.validate().is_err());
        r.source = "--help".into();
        assert!(r.validate().is_err());
    }
}
