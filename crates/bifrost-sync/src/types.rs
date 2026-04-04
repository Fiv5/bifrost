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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub visibility: String,
    pub level: Option<i32>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub create_time: String,
    #[serde(default)]
    pub update_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGroupMember {
    pub id: String,
    pub group_id: String,
    pub user_id: String,
    pub level: i32,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub avatar: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub create_time: String,
    #[serde(default)]
    pub update_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGroupSetting {
    pub group_id: String,
    #[serde(default)]
    pub rules_enabled: bool,
    #[serde(default)]
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupReq {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGroupReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteGroupReq {
    pub user_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGroupSettingReq {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupListResponse {
    pub list: Vec<RemoteGroup>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberListResponse {
    pub list: Vec<RemoteGroupMember>,
    pub total: i64,
}
