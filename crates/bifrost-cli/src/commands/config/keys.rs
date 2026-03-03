use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigKey {
    TlsEnabled,
    TlsUnsafeSsl,
    TlsDisconnectOnChange,
    TlsExclude,
    TlsInclude,
    TlsAppExclude,
    TlsAppInclude,

    TrafficMaxRecords,
    TrafficMaxBodySize,
    TrafficMaxBufferSize,
    TrafficRetentionDays,

    AccessMode,
    AccessAllowLan,
}

impl ConfigKey {
    pub fn is_list(&self) -> bool {
        matches!(
            self,
            Self::TlsExclude | Self::TlsInclude | Self::TlsAppExclude | Self::TlsAppInclude
        )
    }

    pub fn is_size(&self) -> bool {
        matches!(self, Self::TrafficMaxBodySize | Self::TrafficMaxBufferSize)
    }

    pub fn all_keys() -> Vec<&'static str> {
        vec![
            "tls.enabled",
            "tls.unsafe-ssl",
            "tls.disconnect-on-change",
            "tls.exclude",
            "tls.include",
            "tls.app-exclude",
            "tls.app-include",
            "traffic.max-records",
            "traffic.max-body-size",
            "traffic.max-buffer-size",
            "traffic.retention-days",
            "access.mode",
            "access.allow-lan",
        ]
    }
}

impl FromStr for ConfigKey {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "tls.enabled" => Ok(Self::TlsEnabled),
            "tls.unsafe-ssl" => Ok(Self::TlsUnsafeSsl),
            "tls.disconnect-on-change" => Ok(Self::TlsDisconnectOnChange),
            "tls.exclude" => Ok(Self::TlsExclude),
            "tls.include" => Ok(Self::TlsInclude),
            "tls.app-exclude" => Ok(Self::TlsAppExclude),
            "tls.app-include" => Ok(Self::TlsAppInclude),
            "traffic.max-records" => Ok(Self::TrafficMaxRecords),
            "traffic.max-body-size" => Ok(Self::TrafficMaxBodySize),
            "traffic.max-buffer-size" => Ok(Self::TrafficMaxBufferSize),
            "traffic.retention-days" => Ok(Self::TrafficRetentionDays),
            "access.mode" => Ok(Self::AccessMode),
            "access.allow-lan" => Ok(Self::AccessAllowLan),
            _ => Err(format!(
                "Unknown config key: '{}'\n\nAvailable keys:\n{}",
                s,
                ConfigKey::all_keys()
                    .iter()
                    .map(|k| format!("  - {}", k))
                    .collect::<Vec<_>>()
                    .join("\n")
            )),
        }
    }
}

impl std::fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::TlsEnabled => "tls.enabled",
            Self::TlsUnsafeSsl => "tls.unsafe-ssl",
            Self::TlsDisconnectOnChange => "tls.disconnect-on-change",
            Self::TlsExclude => "tls.exclude",
            Self::TlsInclude => "tls.include",
            Self::TlsAppExclude => "tls.app-exclude",
            Self::TlsAppInclude => "tls.app-include",
            Self::TrafficMaxRecords => "traffic.max-records",
            Self::TrafficMaxBodySize => "traffic.max-body-size",
            Self::TrafficMaxBufferSize => "traffic.max-buffer-size",
            Self::TrafficRetentionDays => "traffic.retention-days",
            Self::AccessMode => "access.mode",
            Self::AccessAllowLan => "access.allow-lan",
        };
        write!(f, "{}", s)
    }
}

pub fn parse_bool(s: &str) -> Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        _ => Err(format!(
            "Invalid boolean value: '{}'. Use true/false/yes/no/on/off/1/0",
            s
        )),
    }
}

pub fn parse_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

pub fn parse_size(s: &str) -> Result<usize, String> {
    let s = s.to_uppercase();
    if let Some(num) = s.strip_suffix("KB") {
        return num
            .trim()
            .parse::<usize>()
            .map(|n| n * 1024)
            .map_err(|e| format!("Invalid size: {}", e));
    }
    if let Some(num) = s.strip_suffix("MB") {
        return num
            .trim()
            .parse::<usize>()
            .map(|n| n * 1024 * 1024)
            .map_err(|e| format!("Invalid size: {}", e));
    }
    if let Some(num) = s.strip_suffix("GB") {
        return num
            .trim()
            .parse::<usize>()
            .map(|n| n * 1024 * 1024 * 1024)
            .map_err(|e| format!("Invalid size: {}", e));
    }
    if let Some(num) = s.strip_suffix('B') {
        return num
            .trim()
            .parse::<usize>()
            .map_err(|e| format!("Invalid size: {}", e));
    }
    s.parse::<usize>()
        .map_err(|e| format!("Invalid size: {}", e))
}

pub fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{} GB", bytes / (1024 * 1024 * 1024))
    } else if bytes >= 1024 * 1024 {
        format!("{} MB", bytes / (1024 * 1024))
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{} B", bytes)
    }
}
