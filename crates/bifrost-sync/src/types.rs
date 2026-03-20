use serde::{Deserialize, Serialize};

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
