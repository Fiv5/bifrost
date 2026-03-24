use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use bifrost_core::{BifrostError, Result};
use bifrost_storage::{
    ConfigChangeEvent, ConfigManager, RuleFile, RulesStorage, SyncConfig, SyncConfigUpdate,
};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify, RwLock};

use crate::client::SyncHttpClient;
use crate::normalize::normalize_remote_rule;
use crate::types::{RemoteEnv, RemoteUser, SyncReason};

pub type SharedSyncManager = Arc<SyncManager>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SyncRuleBinding {
    remote_id: String,
    remote_user_id: String,
    remote_updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DeletedRuleTombstone {
    remote_id: String,
    remote_user_id: String,
    deleted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SyncStateFile {
    token: Option<String>,
    user: Option<RemoteUser>,
    last_sync_at: Option<String>,
    last_sync_action: Option<SyncAction>,
    rule_bindings: HashMap<String, SyncRuleBinding>,
    deleted_rules: HashMap<String, DeletedRuleTombstone>,
}

#[derive(Debug, Clone, Default)]
struct LoginPromptState {
    last_opened_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncStatus {
    pub enabled: bool,
    pub auto_sync: bool,
    pub remote_base_url: String,
    pub has_session: bool,
    pub reachable: bool,
    pub authorized: bool,
    pub syncing: bool,
    pub reason: SyncReason,
    pub last_sync_at: Option<String>,
    pub last_sync_action: Option<SyncAction>,
    pub last_error: Option<String>,
    pub user: Option<RemoteUser>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    LocalPushed,
    RemotePulled,
    Bidirectional,
    NoChange,
}

#[derive(Debug, Clone, Default)]
pub struct SyncRuntimeState {
    pub reachable: bool,
    pub authorized: bool,
    pub syncing: bool,
    pub reason: SyncReason,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct SyncManagerHandle {
    inner: SharedSyncManager,
}

impl SyncManagerHandle {
    pub fn new(inner: SharedSyncManager) -> Self {
        Self { inner }
    }

    pub async fn status(&self) -> SyncStatus {
        self.inner.status().await
    }

    pub async fn save_token(&self, token: String) -> Result<SyncStatus> {
        self.inner.save_token(token).await?;
        Ok(self.inner.status().await)
    }

    pub async fn logout(&self) -> Result<SyncStatus> {
        self.inner.logout().await?;
        Ok(self.inner.status().await)
    }

    pub async fn trigger_sync(&self) {
        self.inner.trigger_sync();
    }

    pub async fn login_url(&self, callback_url: &str) -> Result<String> {
        self.inner.login_url(callback_url).await
    }

    pub async fn remote_sample(&self, limit: usize) -> Result<Vec<RemoteEnv>> {
        self.inner.remote_sample(limit).await
    }
}

pub struct SyncManager {
    config_manager: Arc<ConfigManager>,
    local_callback_url: String,
    state_file: PathBuf,
    state: Mutex<SyncStateFile>,
    login_prompt: Mutex<LoginPromptState>,
    runtime: RwLock<SyncRuntimeState>,
    wake: Notify,
}

impl SyncManager {
    pub fn new(config_manager: Arc<ConfigManager>, admin_port: u16) -> Result<Self> {
        let state_file = config_manager.data_dir().join("sync-state.json");
        let state = if state_file.exists() {
            let content = fs::read_to_string(&state_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            SyncStateFile::default()
        };
        Ok(Self {
            config_manager,
            local_callback_url: format!("http://127.0.0.1:{admin_port}/login.html"),
            state_file,
            state: Mutex::new(state),
            login_prompt: Mutex::new(LoginPromptState::default()),
            runtime: RwLock::new(SyncRuntimeState::default()),
            wake: Notify::new(),
        })
    }

    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    pub fn trigger_sync(&self) {
        self.wake.notify_one();
    }

    pub async fn status(&self) -> SyncStatus {
        let config = self.config_manager.config().await;
        let runtime = self.runtime.read().await.clone();
        let state = self.state.lock().clone();
        let has_session = state
            .token
            .as_deref()
            .is_some_and(|token| !token.trim().is_empty());
        SyncStatus {
            enabled: config.sync.enabled,
            auto_sync: config.sync.auto_sync,
            remote_base_url: config.sync.remote_base_url,
            has_session,
            reachable: runtime.reachable,
            authorized: runtime.authorized,
            syncing: runtime.syncing,
            reason: runtime.reason,
            last_sync_at: state.last_sync_at,
            last_sync_action: state.last_sync_action,
            last_error: runtime.last_error,
            user: state.user,
        }
    }

    pub async fn login_url(&self, callback_url: &str) -> Result<String> {
        let config = self.config_manager.config().await;
        let client = SyncHttpClient::new(&config.sync)?;
        Ok(client.login_url(&config.sync, callback_url))
    }

    pub async fn request_login(&self) -> Result<()> {
        let config = self.config_manager.config().await;
        self.open_login_browser(&config.sync, true).await
    }

    pub async fn save_token(&self, token: String) -> Result<()> {
        self.config_manager
            .update_sync_config(SyncConfigUpdate {
                auto_sync: Some(true),
                ..Default::default()
            })
            .await?;
        {
            let mut state = self.state.lock();
            state.token = Some(token);
            self.persist_state(&state)?;
        }
        self.wake.notify_one();
        Ok(())
    }

    pub async fn remote_sample(&self, limit: usize) -> Result<Vec<RemoteEnv>> {
        let config = self.config_manager.config().await;
        let token = self
            .state
            .lock()
            .token
            .clone()
            .ok_or_else(|| BifrostError::Config("sync session token missing".to_string()))?;
        let user = self
            .state
            .lock()
            .user
            .clone()
            .ok_or_else(|| BifrostError::Config("sync user missing".to_string()))?;
        let client = SyncHttpClient::new(&config.sync)?;
        let mut envs = client
            .search_envs(&config.sync, &token, &user.user_id)
            .await?;
        envs.sort_by(|a, b| b.update_time.cmp(&a.update_time));
        envs.truncate(limit.max(1));
        Ok(envs)
    }

    pub async fn logout(&self) -> Result<()> {
        let config = self.config_manager.config().await;
        let token = { self.state.lock().token.clone() };
        if let Some(token) = token {
            let client = SyncHttpClient::new(&config.sync)?;
            let _ = client.logout(&config.sync, &token).await;
        }
        {
            let mut state = self.state.lock();
            state.token = None;
            state.user = None;
            self.persist_state(&state)?;
        }
        self.login_prompt.lock().last_opened_at = None;
        {
            let mut runtime = self.runtime.write().await;
            runtime.authorized = false;
            runtime.reason = SyncReason::Unauthorized;
            runtime.last_error = None;
        }
        Ok(())
    }

    async fn run(self: &Arc<Self>) {
        let mut receiver = self.config_manager.subscribe();
        loop {
            let config = self.config_manager.config().await;
            let interval = Duration::from_secs(config.sync.probe_interval_secs.max(2));
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = self.wake.notified() => {}
                event = receiver.recv() => {
                    match event {
                        Ok(ConfigChangeEvent::RulesChanged | ConfigChangeEvent::SyncConfigChanged) => {}
                        Ok(_) => continue,
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
            if let Err(error) = self.tick().await {
                let mut runtime = self.runtime.write().await;
                runtime.syncing = false;
                runtime.reason = SyncReason::Error;
                runtime.last_error = Some(error.to_string());
            }
        }
    }

    async fn tick(&self) -> Result<()> {
        let config = self.config_manager.config().await;
        if !config.sync.enabled {
            let mut runtime = self.runtime.write().await;
            runtime.reachable = false;
            runtime.authorized = false;
            runtime.syncing = false;
            runtime.reason = SyncReason::Disabled;
            runtime.last_error = None;
            return Ok(());
        }

        let client = SyncHttpClient::new(&config.sync)?;
        let reachable = client.probe_reachable(&config.sync).await;
        tracing::info!(
            target: "bifrost_sync::manager",
            enabled = config.sync.enabled,
            auto_sync = config.sync.auto_sync,
            remote_base_url = %config.sync.remote_base_url,
            reachable,
            "sync tick evaluated connectivity"
        );
        if !reachable {
            let mut runtime = self.runtime.write().await;
            runtime.reachable = false;
            runtime.authorized = false;
            runtime.syncing = false;
            runtime.reason = SyncReason::Unreachable;
            runtime.last_error = None;
            return Ok(());
        }

        let token = { self.state.lock().token.clone() };
        if token.as_deref().unwrap_or("").is_empty() {
            let _ = self.open_login_browser(&config.sync, false).await;
            let mut runtime = self.runtime.write().await;
            runtime.reachable = true;
            runtime.authorized = false;
            runtime.syncing = false;
            runtime.reason = SyncReason::Unauthorized;
            runtime.last_error = None;
            return Ok(());
        }

        let token = token.unwrap_or_default();
        let user = client.get_user_info(&config.sync, &token).await?;
        let Some(user) = user else {
            {
                let mut state = self.state.lock();
                state.user = None;
                state.token = None;
                self.persist_state(&state)?;
            }
            let mut runtime = self.runtime.write().await;
            runtime.reachable = true;
            runtime.authorized = false;
            runtime.syncing = false;
            runtime.reason = SyncReason::Unauthorized;
            runtime.last_error = None;
            return Ok(());
        };

        {
            let mut state = self.state.lock();
            state.user = Some(user.clone());
            self.persist_state(&state)?;
        }

        if !config.sync.auto_sync {
            let mut runtime = self.runtime.write().await;
            runtime.reachable = true;
            runtime.authorized = true;
            runtime.syncing = false;
            runtime.reason = SyncReason::Ready;
            runtime.last_error = None;
            return Ok(());
        }

        {
            let mut runtime = self.runtime.write().await;
            runtime.reachable = true;
            runtime.authorized = true;
            runtime.syncing = true;
            runtime.reason = SyncReason::Syncing;
            runtime.last_error = None;
        }

        let result = self.sync_rules(&client, &config.sync, &token, &user).await;
        let mut runtime = self.runtime.write().await;
        runtime.reachable = true;
        runtime.authorized = true;
        runtime.syncing = false;
        match result {
            Ok(()) => {
                runtime.reason = SyncReason::Ready;
                runtime.last_error = None;
                Ok(())
            }
            Err(error) => {
                tracing::error!(
                    target: "bifrost_sync::manager",
                    error = %error,
                    user_id = %user.user_id,
                    "sync tick failed"
                );
                runtime.reason = SyncReason::Error;
                runtime.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    async fn open_login_browser(&self, sync_config: &SyncConfig, force: bool) -> Result<()> {
        let should_open = {
            let prompt = self.login_prompt.lock();
            force || prompt.last_opened_at.is_none()
        };
        if !should_open {
            return Ok(());
        }

        let client = SyncHttpClient::new(sync_config)?;
        let login_url = client.login_url_with_reauth(sync_config, &self.local_callback_url);
        open_url_in_browser(&login_url)?;
        self.login_prompt.lock().last_opened_at = Some(Utc::now());
        Ok(())
    }

    async fn sync_rules(
        &self,
        client: &SyncHttpClient,
        config: &SyncConfig,
        token: &str,
        user: &RemoteUser,
    ) -> Result<()> {
        let rules_storage = self.config_manager.rules_storage().await;
        let local_rules = rules_storage.load_all()?;
        let remote_rules = client.search_envs(config, token, &user.user_id).await?;
        tracing::info!(
            target: "bifrost_sync::manager",
            local_rules = local_rules.len(),
            remote_rules = remote_rules.len(),
            user_id = %user.user_id,
            "starting rules sync"
        );

        let mut state = self.state.lock().clone();
        self.refresh_deleted_rules(&local_rules, &mut state);

        let local_map: HashMap<String, RuleFile> = local_rules
            .iter()
            .cloned()
            .map(|rule| (rule.name.clone(), rule))
            .collect();
        let remote_map: HashMap<String, RemoteEnv> = remote_rules
            .iter()
            .cloned()
            .map(|env| (env.name.clone(), env))
            .collect();

        let mut just_deleted_names: HashSet<String> = HashSet::new();
        let deleted_rule_names: Vec<String> = state.deleted_rules.keys().cloned().collect();
        for deleted_name in deleted_rule_names {
            let Some(tombstone) = state.deleted_rules.get(&deleted_name).cloned() else {
                continue;
            };
            if let Some(remote_env) = remote_rules
                .iter()
                .find(|env| env.id == tombstone.remote_id)
            {
                match client.delete_env(config, token, remote_env).await {
                    Ok(_) => {
                        tracing::info!(
                            target: "bifrost_sync::manager",
                            name = %deleted_name,
                            remote_id = %tombstone.remote_id,
                            "deleted remote rule via tombstone"
                        );
                        state.deleted_rules.remove(&deleted_name);
                        state.rule_bindings.remove(&deleted_name);
                        just_deleted_names.insert(deleted_name);
                    }
                    Err(error) => {
                        tracing::warn!(
                            target: "bifrost_sync::manager",
                            name = %deleted_name,
                            remote_id = %tombstone.remote_id,
                            %error,
                            "failed to delete remote rule, keeping tombstone for retry"
                        );
                        just_deleted_names.insert(deleted_name);
                    }
                }
            } else {
                tracing::debug!(
                    target: "bifrost_sync::manager",
                    name = %deleted_name,
                    remote_id = %tombstone.remote_id,
                    "remote rule not found, clearing tombstone"
                );
                state.deleted_rules.remove(&deleted_name);
                state.rule_bindings.remove(&deleted_name);
                just_deleted_names.insert(deleted_name);
            }
        }

        let mut all_names: HashSet<String> = local_map.keys().cloned().collect();
        all_names.extend(remote_map.keys().cloned());

        let mut pulled_remote = false;
        let mut pushed_local = false;
        for name in all_names {
            match (local_map.get(&name), remote_map.get(&name)) {
                (Some(local_rule), Some(remote_env)) => {
                    let normalized_remote_content =
                        normalize_remote_rule(remote_env, &remote_rules);
                    match resolve_rule_conflict(local_rule, remote_env, &normalized_remote_content)
                    {
                        ConflictResolution::PushLocal => {
                            let updated_remote = client
                                .update_env(config, token, remote_env, &local_rule.content)
                                .await?;
                            state.rule_bindings.insert(
                                name.clone(),
                                SyncRuleBinding {
                                    remote_id: updated_remote.id,
                                    remote_user_id: updated_remote.user_id,
                                    remote_updated_at: updated_remote.update_time,
                                },
                            );
                            pushed_local = true;
                        }
                        ConflictResolution::PullRemote => {
                            self.save_remote_as_local(
                                &rules_storage,
                                local_rule,
                                remote_env,
                                &remote_rules,
                            )?;
                            state.rule_bindings.insert(
                                name.clone(),
                                SyncRuleBinding {
                                    remote_id: remote_env.id.clone(),
                                    remote_user_id: remote_env.user_id.clone(),
                                    remote_updated_at: remote_env.update_time.clone(),
                                },
                            );
                            pulled_remote = true;
                        }
                        ConflictResolution::KeepLocal => {
                            state.rule_bindings.insert(
                                name.clone(),
                                SyncRuleBinding {
                                    remote_id: remote_env.id.clone(),
                                    remote_user_id: remote_env.user_id.clone(),
                                    remote_updated_at: remote_env.update_time.clone(),
                                },
                            );
                        }
                    }
                }
                (Some(local_rule), None) => {
                    tracing::info!(
                        target: "bifrost_sync::manager",
                        name = %local_rule.name,
                        "creating remote rule from local"
                    );
                    let created = client
                        .create_env(
                            config,
                            token,
                            &user.user_id,
                            &local_rule.name,
                            &local_rule.content,
                        )
                        .await?;
                    state.rule_bindings.insert(
                        name.clone(),
                        SyncRuleBinding {
                            remote_id: created.id,
                            remote_user_id: created.user_id,
                            remote_updated_at: created.update_time,
                        },
                    );
                    pushed_local = true;
                }
                (None, Some(remote_env)) => {
                    if just_deleted_names.contains(&name) {
                        tracing::debug!(
                            target: "bifrost_sync::manager",
                            name = %remote_env.name,
                            "skipping recently deleted rule, not re-pulling from remote"
                        );
                        continue;
                    }
                    tracing::info!(
                        target: "bifrost_sync::manager",
                        name = %remote_env.name,
                        "pulling remote rule into local storage"
                    );
                    let remote_placeholder = RuleFile {
                        name: remote_env.name.clone(),
                        content: normalize_remote_rule(remote_env, &remote_rules),
                        enabled: false,
                        sort_order: 0,
                        description: Some("Synced from remote".to_string()),
                        version: "1.0.0".to_string(),
                        created_at: remote_env.create_time.clone(),
                        updated_at: remote_env.update_time.clone(),
                    };
                    rules_storage.save(&remote_placeholder)?;
                    state.rule_bindings.insert(
                        name.clone(),
                        SyncRuleBinding {
                            remote_id: remote_env.id.clone(),
                            remote_user_id: remote_env.user_id.clone(),
                            remote_updated_at: remote_env.update_time.clone(),
                        },
                    );
                    pulled_remote = true;
                }
                (None, None) => {}
            }
        }

        state.last_sync_at = Some(Utc::now().to_rfc3339());
        state.last_sync_action = Some(match (pushed_local, pulled_remote) {
            (true, true) => SyncAction::Bidirectional,
            (true, false) => SyncAction::LocalPushed,
            (false, true) => SyncAction::RemotePulled,
            (false, false) => SyncAction::NoChange,
        });
        *self.state.lock() = state.clone();
        self.persist_state(&state)?;

        if pulled_remote {
            let _ = self.config_manager.notify(ConfigChangeEvent::RulesChanged);
        }

        Ok(())
    }

    fn save_remote_as_local(
        &self,
        rules_storage: &RulesStorage,
        existing_rule: &RuleFile,
        remote_env: &RemoteEnv,
        remote_envs: &[RemoteEnv],
    ) -> Result<()> {
        let rule = merge_remote_into_local_rule(existing_rule, remote_env, remote_envs);
        rules_storage.save(&rule)
    }

    fn refresh_deleted_rules(&self, local_rules: &[RuleFile], state: &mut SyncStateFile) {
        let live_names: HashSet<&str> = local_rules.iter().map(|rule| rule.name.as_str()).collect();
        let missing_names: Vec<String> = state
            .rule_bindings
            .keys()
            .filter(|name| !live_names.contains(name.as_str()))
            .cloned()
            .collect();

        for missing_name in missing_names {
            if state.deleted_rules.contains_key(&missing_name) {
                continue;
            }
            if let Some(binding) = state.rule_bindings.get(&missing_name) {
                state.deleted_rules.insert(
                    missing_name.clone(),
                    DeletedRuleTombstone {
                        remote_id: binding.remote_id.clone(),
                        remote_user_id: binding.remote_user_id.clone(),
                        deleted_at: Utc::now().to_rfc3339(),
                    },
                );
            }
        }
    }

    fn persist_state(&self, state: &SyncStateFile) -> Result<()> {
        let content = serde_json::to_string_pretty(state)
            .map_err(|e| BifrostError::Config(format!("failed to serialize sync state: {e}")))?;
        fs::write(&self.state_file, content)?;
        Ok(())
    }
}

fn should_refresh_synced_local_copy(local_rule: &RuleFile, remote_env: &RemoteEnv) -> bool {
    local_rule.description.as_deref() == Some("Synced from remote")
        || local_rule.content == remote_env.rule
}

fn merge_remote_into_local_rule(
    existing_rule: &RuleFile,
    remote_env: &RemoteEnv,
    remote_envs: &[RemoteEnv],
) -> RuleFile {
    RuleFile {
        name: existing_rule.name.clone(),
        content: normalize_remote_rule(remote_env, remote_envs),
        enabled: existing_rule.enabled,
        sort_order: existing_rule.sort_order,
        description: existing_rule.description.clone(),
        version: existing_rule.version.clone(),
        created_at: existing_rule.created_at.clone(),
        updated_at: remote_env.update_time.clone(),
    }
}

fn open_url_in_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    command
        .spawn()
        .map_err(|error| BifrostError::Network(format!("failed to open login browser: {error}")))?;
    Ok(())
}

fn compare_timestamps(left: &str, right: &str) -> std::cmp::Ordering {
    let left_time = parse_timestamp(left);
    let right_time = parse_timestamp(right);
    left_time.cmp(&right_time)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConflictResolution {
    PushLocal,
    PullRemote,
    KeepLocal,
}

fn resolve_rule_conflict(
    local_rule: &RuleFile,
    remote_env: &RemoteEnv,
    normalized_remote_content: &str,
) -> ConflictResolution {
    match compare_timestamps(&local_rule.updated_at, &remote_env.update_time) {
        std::cmp::Ordering::Greater => ConflictResolution::PushLocal,
        std::cmp::Ordering::Less => ConflictResolution::PullRemote,
        std::cmp::Ordering::Equal => {
            if should_refresh_synced_local_copy(local_rule, remote_env)
                && local_rule.content != normalized_remote_content
            {
                ConflictResolution::PullRemote
            } else {
                ConflictResolution::KeepLocal
            }
        }
    }
}

fn parse_timestamp(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn compare_timestamp_prefers_latest_value() {
        assert_eq!(
            compare_timestamps("2026-03-20T10:00:00Z", "2026-03-20T09:00:00Z"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_timestamps("2026-03-20T08:00:00Z", "2026-03-20T09:00:00Z"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn merge_remote_into_local_preserves_local_metadata() {
        let existing_rule = RuleFile::new("demo", "old.example.com host://127.0.0.1:3000")
            .with_enabled(false)
            .with_sort_order(7)
            .with_description(Some("Pinned locally".to_string()));
        let remote_env = RemoteEnv {
            id: "remote-id".to_string(),
            user_id: "user-1".to_string(),
            name: "demo".to_string(),
            rule: "new.example.com host://127.0.0.1:3200".to_string(),
            create_time: "2026-03-20T09:00:00Z".to_string(),
            update_time: "2026-03-20T12:00:00Z".to_string(),
        };

        let merged = merge_remote_into_local_rule(
            &existing_rule,
            &remote_env,
            std::slice::from_ref(&remote_env),
        );

        assert_eq!(merged.name, "demo");
        assert_eq!(merged.content, "new.example.com host://127.0.0.1:3200");
        assert!(!merged.enabled);
        assert_eq!(merged.sort_order, 7);
        assert_eq!(merged.description.as_deref(), Some("Pinned locally"));
        assert_eq!(merged.version, existing_rule.version);
        assert_eq!(merged.created_at, existing_rule.created_at);
        assert_eq!(merged.updated_at, "2026-03-20T12:00:00Z");
    }

    #[test]
    fn resolve_rule_conflict_prefers_local_when_local_is_newer() {
        let mut local_rule = RuleFile::new("demo", "local.example.com host://127.0.0.1:3000");
        local_rule.updated_at = "2026-03-20T12:00:00Z".to_string();
        let remote_env = RemoteEnv {
            id: "remote-id".to_string(),
            user_id: "user-1".to_string(),
            name: "demo".to_string(),
            rule: "remote.example.com host://127.0.0.1:3200".to_string(),
            create_time: "2026-03-20T09:00:00Z".to_string(),
            update_time: "2026-03-20T11:00:00Z".to_string(),
        };

        assert_eq!(
            resolve_rule_conflict(&local_rule, &remote_env, &remote_env.rule),
            ConflictResolution::PushLocal
        );
    }

    #[test]
    fn resolve_rule_conflict_prefers_remote_when_remote_is_newer() {
        let mut local_rule = RuleFile::new("demo", "local.example.com host://127.0.0.1:3000");
        local_rule.updated_at = "2026-03-20T10:00:00Z".to_string();
        let remote_env = RemoteEnv {
            id: "remote-id".to_string(),
            user_id: "user-1".to_string(),
            name: "demo".to_string(),
            rule: "remote.example.com host://127.0.0.1:3200".to_string(),
            create_time: "2026-03-20T09:00:00Z".to_string(),
            update_time: "2026-03-20T11:00:00Z".to_string(),
        };

        assert_eq!(
            resolve_rule_conflict(&local_rule, &remote_env, &remote_env.rule),
            ConflictResolution::PullRemote
        );
    }

    #[test]
    fn resolve_rule_conflict_keeps_local_when_timestamps_match() {
        let mut local_rule = RuleFile::new("demo", "local.example.com host://127.0.0.1:3000")
            .with_description(Some("Pinned locally".to_string()));
        local_rule.updated_at = "2026-03-20T11:00:00Z".to_string();
        let remote_env = RemoteEnv {
            id: "remote-id".to_string(),
            user_id: "user-1".to_string(),
            name: "demo".to_string(),
            rule: "remote.example.com host://127.0.0.1:3200".to_string(),
            create_time: "2026-03-20T09:00:00Z".to_string(),
            update_time: "2026-03-20T11:00:00Z".to_string(),
        };

        assert_eq!(
            resolve_rule_conflict(&local_rule, &remote_env, &remote_env.rule),
            ConflictResolution::KeepLocal
        );
    }

    #[test]
    fn resolve_rule_conflict_refreshes_synced_copy_when_timestamps_match() {
        let mut local_rule = RuleFile::new("demo", "stale.example.com host://127.0.0.1:3000")
            .with_description(Some("Synced from remote".to_string()));
        local_rule.updated_at = "2026-03-20T11:00:00Z".to_string();
        let remote_env = RemoteEnv {
            id: "remote-id".to_string(),
            user_id: "user-1".to_string(),
            name: "demo".to_string(),
            rule: "fresh.example.com host://127.0.0.1:3200".to_string(),
            create_time: "2026-03-20T09:00:00Z".to_string(),
            update_time: "2026-03-20T11:00:00Z".to_string(),
        };

        assert_eq!(
            resolve_rule_conflict(&local_rule, &remote_env, &remote_env.rule),
            ConflictResolution::PullRemote
        );
    }

    #[test]
    fn sync_action_summarizes_push_pull_and_idle_results() {
        let action = |pushed_local: bool, pulled_remote: bool| match (pushed_local, pulled_remote) {
            (true, true) => SyncAction::Bidirectional,
            (true, false) => SyncAction::LocalPushed,
            (false, true) => SyncAction::RemotePulled,
            (false, false) => SyncAction::NoChange,
        };

        assert_eq!(action(true, false), SyncAction::LocalPushed);
        assert_eq!(action(false, true), SyncAction::RemotePulled);
        assert_eq!(action(true, true), SyncAction::Bidirectional);
        assert_eq!(action(false, false), SyncAction::NoChange);
    }

    #[tokio::test]
    async fn save_token_reenables_auto_sync_after_login() {
        let temp_dir = TempDir::new().unwrap();
        let config_manager = Arc::new(ConfigManager::new(temp_dir.path().to_path_buf()).unwrap());
        config_manager
            .update_sync_config(SyncConfigUpdate {
                enabled: Some(true),
                auto_sync: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();
        let manager = SyncManager::new(config_manager.clone(), 9900).unwrap();

        manager.save_token("login-token".to_string()).await.unwrap();

        let config = config_manager.config().await;
        let status = manager.status().await;
        assert!(config.sync.auto_sync);
        assert!(status.has_session);
        assert_eq!(manager.state.lock().token.as_deref(), Some("login-token"));
    }
}
