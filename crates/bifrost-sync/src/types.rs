use serde::{Deserialize, Deserializer, Serialize};

fn string_or_number<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNumber {
        String(String),
        U64(u64),
        I64(i64),
        F64(f64),
    }
    match StringOrNumber::deserialize(deserializer)? {
        StringOrNumber::String(s) => Ok(s),
        StringOrNumber::U64(n) => Ok(n.to_string()),
        StringOrNumber::I64(n) => Ok(n.to_string()),
        StringOrNumber::F64(n) => Ok(n.to_string()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteUser {
    pub user_id: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteEnv {
    #[serde(default, deserialize_with = "string_or_number")]
    pub id: String,
    pub user_id: String,
    pub name: String,
    #[serde(default)]
    pub rule: String,
    pub create_time: String,
    pub update_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncReason {
    #[default]
    Disabled,
    Reachable,
    Unreachable,
    Unauthorized,
    Ready,
    Syncing,
    Error,
}
