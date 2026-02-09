use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::RwLock;

use ipnet::IpNet;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessMode {
    AllowAll,
    #[default]
    LocalOnly,
    Whitelist,
    Interactive,
}

impl std::str::FromStr for AccessMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "allow_all" | "allowall" | "all" => Ok(AccessMode::AllowAll),
            "local_only" | "localonly" | "local" => Ok(AccessMode::LocalOnly),
            "whitelist" | "wl" => Ok(AccessMode::Whitelist),
            "interactive" | "prompt" => Ok(AccessMode::Interactive),
            _ => Err(format!("Invalid access mode: {}", s)),
        }
    }
}

impl std::fmt::Display for AccessMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessMode::AllowAll => write!(f, "allow_all"),
            AccessMode::LocalOnly => write!(f, "local_only"),
            AccessMode::Whitelist => write!(f, "whitelist"),
            AccessMode::Interactive => write!(f, "interactive"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    Allow,
    Deny,
    Prompt(IpAddr),
}

#[derive(Debug, Clone)]
pub struct AccessControlConfig {
    pub mode: AccessMode,
    pub whitelist: Vec<String>,
    pub allow_lan: bool,
}

impl Default for AccessControlConfig {
    fn default() -> Self {
        Self {
            mode: AccessMode::LocalOnly,
            whitelist: Vec::new(),
            allow_lan: false,
        }
    }
}

pub struct ClientAccessControl {
    mode: AccessMode,
    whitelist: HashSet<IpNet>,
    allow_lan: bool,
    temporary_whitelist: RwLock<HashSet<IpAddr>>,
    session_denied: RwLock<HashSet<IpAddr>>,
}

impl ClientAccessControl {
    pub fn new(config: AccessControlConfig) -> Self {
        let mut whitelist = HashSet::new();

        for entry in &config.whitelist {
            match Self::parse_ip_or_cidr(entry) {
                Ok(ip_net) => {
                    whitelist.insert(ip_net);
                }
                Err(e) => {
                    warn!("Invalid whitelist entry '{}': {}", entry, e);
                }
            }
        }

        Self {
            mode: config.mode,
            whitelist,
            allow_lan: config.allow_lan,
            temporary_whitelist: RwLock::new(HashSet::new()),
            session_denied: RwLock::new(HashSet::new()),
        }
    }

    pub fn with_mode(mode: AccessMode) -> Self {
        Self {
            mode,
            whitelist: HashSet::new(),
            allow_lan: false,
            temporary_whitelist: RwLock::new(HashSet::new()),
            session_denied: RwLock::new(HashSet::new()),
        }
    }

    fn parse_ip_or_cidr(s: &str) -> Result<IpNet, String> {
        if s.contains('/') {
            s.parse::<IpNet>()
                .map_err(|e| format!("Invalid CIDR notation: {}", e))
        } else {
            let ip: IpAddr = s
                .parse()
                .map_err(|e| format!("Invalid IP address: {}", e))?;
            let ip_net = match ip {
                IpAddr::V4(v4) => format!("{}/32", v4)
                    .parse()
                    .map_err(|e| format!("Failed to create IpNet: {}", e))?,
                IpAddr::V6(v6) => format!("{}/128", v6)
                    .parse()
                    .map_err(|e| format!("Failed to create IpNet: {}", e))?,
            };
            Ok(ip_net)
        }
    }

    pub fn check_access(&self, peer_addr: &IpAddr) -> AccessDecision {
        if self.is_loopback(peer_addr) {
            debug!("Allowing loopback address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        match self.mode {
            AccessMode::AllowAll => {
                debug!("AllowAll mode: accepting {}", peer_addr);
                AccessDecision::Allow
            }
            AccessMode::LocalOnly => {
                info!("LocalOnly mode: rejecting non-local address {}", peer_addr);
                AccessDecision::Deny
            }
            AccessMode::Whitelist => self.check_whitelist(peer_addr),
            AccessMode::Interactive => self.check_interactive(peer_addr),
        }
    }

    fn check_whitelist(&self, peer_addr: &IpAddr) -> AccessDecision {
        if self.is_in_temporary_whitelist(peer_addr) {
            debug!("Allowing temporarily whitelisted address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        if self.is_in_whitelist(peer_addr) {
            debug!("Allowing whitelisted address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        if self.allow_lan && self.is_private_network(peer_addr) {
            debug!("Allowing LAN address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        info!(
            "Whitelist mode: rejecting non-whitelisted address {}",
            peer_addr
        );
        AccessDecision::Deny
    }

    fn check_interactive(&self, peer_addr: &IpAddr) -> AccessDecision {
        if self.is_session_denied(peer_addr) {
            debug!("Denying previously rejected address: {}", peer_addr);
            return AccessDecision::Deny;
        }

        if self.is_in_temporary_whitelist(peer_addr) {
            debug!("Allowing temporarily whitelisted address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        if self.is_in_whitelist(peer_addr) {
            debug!("Allowing whitelisted address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        if self.allow_lan && self.is_private_network(peer_addr) {
            debug!("Allowing LAN address: {}", peer_addr);
            return AccessDecision::Allow;
        }

        AccessDecision::Prompt(*peer_addr)
    }

    pub fn is_loopback(&self, ip: &IpAddr) -> bool {
        ip.is_loopback()
    }

    pub fn is_private_network(&self, ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                octets[0] == 10
                    || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                    || (octets[0] == 192 && octets[1] == 168)
                    || (octets[0] == 169 && octets[1] == 254)
            }
            IpAddr::V6(v6) => {
                let segments = v6.segments();
                (segments[0] & 0xfe00) == 0xfc00 || (segments[0] == 0xfe80)
            }
        }
    }

    pub fn is_in_whitelist(&self, ip: &IpAddr) -> bool {
        for net in &self.whitelist {
            if net.contains(ip) {
                return true;
            }
        }
        false
    }

    pub fn is_in_temporary_whitelist(&self, ip: &IpAddr) -> bool {
        let temp = self.temporary_whitelist.read().unwrap();
        temp.contains(ip)
    }

    pub fn is_session_denied(&self, ip: &IpAddr) -> bool {
        let denied = self.session_denied.read().unwrap();
        denied.contains(ip)
    }

    pub fn add_to_whitelist(&mut self, ip_or_cidr: &str) -> Result<(), String> {
        let ip_net = Self::parse_ip_or_cidr(ip_or_cidr)?;
        self.whitelist.insert(ip_net);
        info!("Added {} to whitelist", ip_or_cidr);
        Ok(())
    }

    pub fn remove_from_whitelist(&mut self, ip_or_cidr: &str) -> Result<bool, String> {
        let ip_net = Self::parse_ip_or_cidr(ip_or_cidr)?;
        let removed = self.whitelist.remove(&ip_net);
        if removed {
            info!("Removed {} from whitelist", ip_or_cidr);
        }
        Ok(removed)
    }

    pub fn add_temporary(&self, ip: IpAddr) {
        let mut temp = self.temporary_whitelist.write().unwrap();
        temp.insert(ip);
        info!("Added {} to temporary whitelist", ip);
    }

    pub fn remove_temporary(&self, ip: &IpAddr) -> bool {
        let mut temp = self.temporary_whitelist.write().unwrap();
        temp.remove(ip)
    }

    pub fn deny_session(&self, ip: IpAddr) {
        let mut denied = self.session_denied.write().unwrap();
        denied.insert(ip);
        info!("Denied {} for this session", ip);
    }

    pub fn clear_session_denied(&self) {
        let mut denied = self.session_denied.write().unwrap();
        denied.clear();
    }

    pub fn whitelist_entries(&self) -> Vec<String> {
        self.whitelist.iter().map(|n| n.to_string()).collect()
    }

    pub fn temporary_whitelist_entries(&self) -> Vec<IpAddr> {
        let temp = self.temporary_whitelist.read().unwrap();
        temp.iter().cloned().collect()
    }

    pub fn mode(&self) -> AccessMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: AccessMode) {
        self.mode = mode;
        info!("Access mode changed to: {}", mode);
    }

    pub fn allow_lan(&self) -> bool {
        self.allow_lan
    }

    pub fn set_allow_lan(&mut self, allow: bool) {
        self.allow_lan = allow;
        info!("Allow LAN set to: {}", allow);
    }
}

impl Default for ClientAccessControl {
    fn default() -> Self {
        Self::new(AccessControlConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_access_mode_from_str() {
        assert_eq!(
            "allow_all".parse::<AccessMode>().unwrap(),
            AccessMode::AllowAll
        );
        assert_eq!(
            "local_only".parse::<AccessMode>().unwrap(),
            AccessMode::LocalOnly
        );
        assert_eq!(
            "whitelist".parse::<AccessMode>().unwrap(),
            AccessMode::Whitelist
        );
        assert_eq!(
            "interactive".parse::<AccessMode>().unwrap(),
            AccessMode::Interactive
        );
        assert!("invalid".parse::<AccessMode>().is_err());
    }

    #[test]
    fn test_loopback_detection() {
        let ac = ClientAccessControl::default();

        assert!(ac.is_loopback(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(ac.is_loopback(&IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(!ac.is_loopback(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
    }

    #[test]
    fn test_private_network_detection() {
        let ac = ClientAccessControl::default();

        assert!(ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
        assert!(ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));

        assert!(!ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!ac.is_private_network(&IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1))));
    }

    #[test]
    fn test_local_only_mode() {
        let ac = ClientAccessControl::with_mode(AccessMode::LocalOnly);

        let localhost = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        assert_eq!(ac.check_access(&localhost), AccessDecision::Allow);

        let external = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(ac.check_access(&external), AccessDecision::Deny);
    }

    #[test]
    fn test_allow_all_mode() {
        let ac = ClientAccessControl::with_mode(AccessMode::AllowAll);

        let external = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
        assert_eq!(ac.check_access(&external), AccessDecision::Allow);
    }

    #[test]
    fn test_whitelist_mode() {
        let config = AccessControlConfig {
            mode: AccessMode::Whitelist,
            whitelist: vec!["192.168.1.0/24".to_string()],
            allow_lan: false,
        };
        let ac = ClientAccessControl::new(config);

        let allowed = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(ac.check_access(&allowed), AccessDecision::Allow);

        let denied = IpAddr::V4(Ipv4Addr::new(192, 168, 2, 100));
        assert_eq!(ac.check_access(&denied), AccessDecision::Deny);
    }

    #[test]
    fn test_whitelist_with_allow_lan() {
        let config = AccessControlConfig {
            mode: AccessMode::Whitelist,
            whitelist: vec![],
            allow_lan: true,
        };
        let ac = ClientAccessControl::new(config);

        let lan_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(ac.check_access(&lan_ip), AccessDecision::Allow);

        let external = IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8));
        assert_eq!(ac.check_access(&external), AccessDecision::Deny);
    }

    #[test]
    fn test_interactive_mode() {
        let ac = ClientAccessControl::with_mode(AccessMode::Interactive);

        let external = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(ac.check_access(&external), AccessDecision::Prompt(external));
    }

    #[test]
    fn test_temporary_whitelist() {
        let ac = ClientAccessControl::with_mode(AccessMode::Whitelist);

        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(ac.check_access(&ip), AccessDecision::Deny);

        ac.add_temporary(ip);
        assert_eq!(ac.check_access(&ip), AccessDecision::Allow);

        ac.remove_temporary(&ip);
        assert_eq!(ac.check_access(&ip), AccessDecision::Deny);
    }

    #[test]
    fn test_session_denied() {
        let ac = ClientAccessControl::with_mode(AccessMode::Interactive);

        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));

        assert_eq!(ac.check_access(&ip), AccessDecision::Prompt(ip));

        ac.deny_session(ip);
        assert_eq!(ac.check_access(&ip), AccessDecision::Deny);

        ac.clear_session_denied();
        assert_eq!(ac.check_access(&ip), AccessDecision::Prompt(ip));
    }

    #[test]
    fn test_add_remove_whitelist() {
        let mut ac = ClientAccessControl::with_mode(AccessMode::Whitelist);

        ac.add_to_whitelist("10.0.0.1").unwrap();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(ac.is_in_whitelist(&ip));

        ac.remove_from_whitelist("10.0.0.1").unwrap();
        assert!(!ac.is_in_whitelist(&ip));
    }

    #[test]
    fn test_cidr_whitelist() {
        let mut ac = ClientAccessControl::with_mode(AccessMode::Whitelist);

        ac.add_to_whitelist("10.0.0.0/8").unwrap();

        assert!(ac.is_in_whitelist(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(ac.is_in_whitelist(&IpAddr::V4(Ipv4Addr::new(10, 255, 255, 255))));
        assert!(!ac.is_in_whitelist(&IpAddr::V4(Ipv4Addr::new(11, 0, 0, 1))));
    }

    #[test]
    fn test_whitelist_entries() {
        let config = AccessControlConfig {
            mode: AccessMode::Whitelist,
            whitelist: vec!["192.168.1.0/24".to_string(), "10.0.0.1".to_string()],
            allow_lan: false,
        };
        let ac = ClientAccessControl::new(config);

        let entries = ac.whitelist_entries();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(format!("{}", AccessMode::AllowAll), "allow_all");
        assert_eq!(format!("{}", AccessMode::LocalOnly), "local_only");
        assert_eq!(format!("{}", AccessMode::Whitelist), "whitelist");
        assert_eq!(format!("{}", AccessMode::Interactive), "interactive");
    }
}
