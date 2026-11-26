use crate::model::*;
use glib::variant::ToVariant;
const NAME: &str = "org.fedoraproject.FirewallD1";
const PATH: &str = "/org/fedoraproject/FirewallD1";
const ZONE: &str = "org.fedoraproject.FirewallD1.zone";
fn bus() -> Result<gio::DBusConnection, String> {
    gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE)
        .map_err(|e| format!("Unable to connect to the system bus: {e}"))
}
fn call(
    c: &gio::DBusConnection,
    interface: &str,
    method: &str,
    args: glib::Variant,
) -> Result<glib::Variant, String> {
    c.call_sync(
        Some(NAME),
        PATH,
        interface,
        method,
        Some(&args),
        None,
        gio::DBusCallFlags::ALLOW_INTERACTIVE_AUTHORIZATION | gio::DBusCallFlags::NO_AUTO_START,
        30000,
        gio::Cancellable::NONE,
    )
    .map_err(|e| format!("firewalld {method}: {e}"))
}
fn string(v: glib::Variant) -> Result<String, String> {
    v.get::<(String,)>()
        .map(|x| x.0)
        .ok_or_else(|| "Unexpected firewalld reply.".into())
}
fn strings(v: glib::Variant) -> Result<Vec<String>, String> {
    v.get::<(Vec<String>,)>()
        .map(|x| x.0)
        .ok_or_else(|| "Unexpected firewalld list.".into())
}
pub fn ensure_not_running() -> Result<(), String> {
    let c = bus()?;
    let reply = c
        .call_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameHasOwner",
            Some(&(NAME,).to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            2000,
            gio::Cancellable::NONE,
        )
        .map_err(|e| format!("Cannot check for conflicting firewalld: {e}"))?;
    let (active,) = reply
        .get::<(bool,)>()
        .ok_or("Invalid conflict-check response")?;
    if active {
        return Err(
            "firewalld is running. Resolve the backend conflict before changing UFW.".into(),
        );
    }
    Ok(())
}
pub fn available() -> bool {
    let Ok(c) = bus() else { return false };
    c.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ListActivatableNames",
        None,
        None,
        gio::DBusCallFlags::NONE,
        2000,
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|v| v.get::<(Vec<String>,)>())
    .is_some_and(|v| v.0.iter().any(|n| n == NAME))
        || running(&c)
}
fn running(c: &gio::DBusConnection) -> bool {
    c.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "NameHasOwner",
        Some(&(NAME,).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        2000,
        gio::Cancellable::NONE,
    )
    .ok()
    .and_then(|v| v.get::<(bool,)>())
    .is_some_and(|v| v.0)
}
fn status(c: &gio::DBusConnection) -> Result<Snapshot, String> {
    if !running(c) {
        return Err("firewalld is installed but not running. Start it using your system service manager after checking for conflicting firewalls.".into());
    }
    let zone = string(call(c, NAME, "getDefaultZone", ().to_variant())?)?;
    let zones = strings(call(c, ZONE, "getZones", ().to_variant())?)?;
    let mut rules = Vec::new();
    let mut details=String::from("Runtime configuration · zone policies control incoming traffic. Outgoing traffic may also be affected by firewalld policies.\n");
    for z in zones {
        let ports = call(c, ZONE, "getPorts", (&z,).to_variant())?
            .get::<(Vec<(String, String)>,)>()
            .ok_or("Invalid ports response")?
            .0;
        let services = strings(call(c, ZONE, "getServices", (&z,).to_variant())?)?;
        let rich = strings(call(c, ZONE, "getRichRules", (&z,).to_variant())?)?;
        let interfaces = strings(call(c, ZONE, "getInterfaces", (&z,).to_variant())?)?;
        let sources = strings(call(c, ZONE, "getSources", (&z,).to_variant())?)?;
        details.push_str(&format!(
            "\nZone {z}\nInterfaces: {}\nSources: {}\n",
            interfaces.join(", "),
            sources.join(", ")
        ));
        for (p, proto) in ports {
            rules.push(Rule {
                id: format!("port|{z}|{p}|{proto}"),
                description: format!("Allow {p}/{proto} · zone {z} · runtime"),
                port: Some(p),
                protocol: Some(proto),
            });
        }
        for svc in services {
            rules.push(Rule {
                id: format!("service|{z}|{svc}"),
                description: format!("Service {svc} · zone {z} · runtime"),
                port: None,
                protocol: None,
            });
        }
        for r in rich {
            rules.push(Rule {
                id: format!("rich|{z}|{r}"),
                description: format!("Rich rule · {z}: {r}"),
                port: None,
                protocol: None,
            });
        }
    }
    let revision = serde_json::to_string(&(&zone, &rules, &details)).map_err(|e| e.to_string())?;
    Ok(Snapshot {
        backend: "firewalld".into(),
        active: true,
        rules,
        details,
        revision,
        zone,
    })
}
pub fn execute(request: Request) -> Result<Snapshot, String> {
    let c = bus()?;
    let current = status(&c)?;
    if matches!(request, Request::Status {}) {
        return Ok(current);
    }
    if super::ufw::ufw_path().is_ok() {
        let config = std::fs::read_to_string("/etc/ufw/ufw.conf").map_err(|_| {
            "Cannot verify UFW is disabled. Resolve backend availability before changing firewalld."
        })?;
        if config.lines().any(|l| l.trim() == "ENABLED=yes") {
            return Err(
                "UFW is enabled. Resolve the backend conflict before changing firewalld.".into(),
            );
        }
    }
    let expected = request.expected().ok_or("Missing configuration revision")?;
    if expected != current.revision {
        return Err("Configuration changed externally. Refresh and try again.".into());
    }
    match request {
        Request::Add{rule,..}=>{
            rule.validate()?;
            if rule.direction!="in" {return Err("Zone rules in Ward apply to incoming traffic. Outgoing firewalld policies need a policy-specific editor.".into());}
            if rule.action=="allow"&&rule.source=="any"&&rule.destination=="any" {
                call(&c,ZONE,"addPort",(&current.zone,&rule.port,&rule.protocol,0i32).to_variant())?;
            }else{
                let rich=rich_rule(&rule)?;
                call(&c,ZONE,"addRichRule",(&current.zone,&rich,0i32).to_variant())?;
            }
        },
        Request::Delete{id,..}=>{
            if !current.rules.iter().any(|r|r.id==id){return Err("Rule no longer exists.".into());}
            let parts:Vec<_>=id.splitn(4,'|').collect();
            match parts.as_slice(){
                ["port",z,p,proto]=>{call(&c,ZONE,"removePort",(z,p,proto).to_variant())?;},
                ["service",z,s]=>{call(&c,ZONE,"removeService",(z,s).to_variant())?;},
                // Rich-rule text can contain '|'; recover the complete text.
                ["rich",z,..]=>{let r=id.splitn(3,'|').nth(2).ok_or("Invalid rich rule")?;call(&c,ZONE,"removeRichRule",(z,r).to_variant())?;},
                _=>return Err("Unsupported rule.".into())
            }
        },
        Request::Service{name,add,..}=>{
            let available=strings(call(&c,NAME,"getServices",().to_variant())?)?;
            if !available.contains(&name){return Err("Unknown firewalld service.".into());}
            if add{call(&c,ZONE,"addService",(&current.zone,&name,0i32).to_variant())?;}else{call(&c,ZONE,"removeService",(&current.zone,&name).to_variant())?;}
        },
        Request::ZoneBinding {zone,value,source,add,..} => {
            let zones=strings(call(&c,ZONE,"getZones",().to_variant())?)?;
            if !zones.contains(&zone) {return Err("Unknown zone.".into());}
            if source {
                let spec=RuleSpec{port:"1".into(),protocol:"tcp".into(),action:"allow".into(),direction:"in".into(),source:value.clone(),destination:"any".into()};
                spec.validate()?;if value=="any" {return Err("A zone source must be an IP address or network.".into());}
            }else if value.is_empty() || value.len()>15 || !value.chars().all(|c|c.is_ascii_alphanumeric() || "_.-:".contains(c)) {return Err("Invalid interface name.".into());}
            let method=match(source,add){(true,true)=>"addSource",(true,false)=>"removeSource",(false,true)=>"addInterface",(false,false)=>"removeInterface"};
            call(&c,ZONE,method,(&zone,&value).to_variant())?;
        },
        Request::SaveRuntime{..}=>{call(&c,NAME,"runtimeToPermanent",().to_variant())?;},
        Request::Reload{..}=>{call(&c,NAME,"reload",().to_variant())?;},
        _=>return Err("This operation is unavailable for firewalld. Ward does not stop services or map zone policies to UFW defaults.".into()),
    }
    status(&c)
}

/// Build a bounded firewalld rule from validated fields, never raw rich-language
/// input. Addresses are parsed and canonicalized before entering the grammar.
pub(crate) fn rich_rule(rule: &RuleSpec) -> Result<String, String> {
    use std::net::IpAddr;
    rule.validate()?;
    if rule.direction != "in" {
        return Err("Only incoming firewalld zone rules are supported here.".into());
    }
    let action = match rule.action.as_str() {
        "allow" => "accept",
        "deny" => "drop",
        "reject" => "reject",
        _ => {
            return Err("UFW-style rate limiting is not supported by this firewalld editor.".into())
        }
    };
    let mut family = None;
    let mut addresses = String::new();
    for (element, value) in [("source", &rule.source), ("destination", &rule.destination)] {
        if value == "any" {
            continue;
        }
        let mut parts = value.split('/');
        let ip: IpAddr = parts
            .next()
            .unwrap_or("")
            .parse()
            .map_err(|_| "Enter an IP address or network.")?;
        let ipv4 = ip.is_ipv4();
        if family.is_some_and(|current| current != ipv4) {
            return Err("Source and destination must use the same IP version.".into());
        }
        family = Some(ipv4);
        let address = if let Some(prefix) = parts.next() {
            let prefix: u8 = prefix.parse().map_err(|_| "Invalid network prefix.")?;
            format!("{ip}/{prefix}")
        } else {
            ip.to_string()
        };
        addresses.push_str(&format!(" {element} address=\"{address}\""));
    }
    let family = family
        .map(|ipv4| format!(" family=\"{}\"", if ipv4 { "ipv4" } else { "ipv6" }))
        .unwrap_or_default();
    Ok(format!(
        "rule{family}{addresses} port port=\"{}\" protocol=\"{}\" {action}",
        rule.port, rule.protocol
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> RuleSpec {
        RuleSpec {
            port: "443".into(),
            protocol: "tcp".into(),
            action: "allow".into(),
            direction: "in".into(),
            source: "any".into(),
            destination: "any".into(),
        }
    }
    #[test]
    fn restricted_ipv4_rule() {
        let mut s = spec();
        s.source = "192.0.2.0/24".into();
        s.destination = "198.51.100.3".into();
        assert_eq!(
            rich_rule(&s).unwrap(),
            r#"rule family="ipv4" source address="192.0.2.0/24" destination address="198.51.100.3" port port="443" protocol="tcp" accept"#
        );
    }
    #[test]
    fn ipv6_and_dual_stack() {
        let mut s = spec();
        s.source = "2001:0db8::/64".into();
        assert!(rich_rule(&s)
            .unwrap()
            .contains(r#"family="ipv6" source address="2001:db8::/64""#));
        s.source = "any".into();
        s.action = "deny".into();
        s.port = "8000-8010".into();
        s.protocol = "udp".into();
        assert_eq!(
            rich_rule(&s).unwrap(),
            r#"rule port port="8000-8010" protocol="udp" drop"#
        );
        s.action = "reject".into();
        assert!(rich_rule(&s).unwrap().ends_with(" reject"));
    }
    #[test]
    fn reject_mixed_family_and_unsupported_semantics() {
        let mut s = spec();
        s.source = "192.0.2.1".into();
        s.destination = "2001:db8::1".into();
        assert!(rich_rule(&s).is_err());
        s.destination = "any".into();
        s.action = "limit".into();
        assert!(rich_rule(&s).is_err());
        s.action = "allow".into();
        s.direction = "out".into();
        assert!(rich_rule(&s).is_err());
    }
    #[test]
    fn reject_grammar_injection() {
        let mut s = spec();
        s.port = r#"443" accept"#.into();
        assert!(rich_rule(&s).is_err());
        s.port = "443".into();
        s.source = r#"192.0.2.1" accept"#.into();
        assert!(rich_rule(&s).is_err());
    }
}
