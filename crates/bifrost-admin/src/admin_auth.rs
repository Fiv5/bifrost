use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bcrypt::{hash, verify, DEFAULT_COST};
use bifrost_core::{BifrostError, Result};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::state::AdminState;

pub const ADMIN_REMOTE_ACCESS_ENABLED_KEY: &str = "admin.remote_access.enabled";
pub const ADMIN_AUTH_USERNAME_KEY: &str = "admin.auth.username";
pub const ADMIN_AUTH_PASSWORD_HASH_KEY: &str = "admin.auth.password_hash";
pub const ADMIN_AUTH_JWT_SECRET_KEY: &str = "admin.auth.jwt_secret";
pub const ADMIN_AUTH_REVOKE_BEFORE_KEY: &str = "admin.auth.revoke_before";

const JWT_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminJwtClaims {
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64
}

pub fn is_remote_access_enabled(state: &AdminState) -> bool {
    let Some(storage) = state.values_storage.as_ref() else {
        return false;
    };
    let mut guard = storage.write();
    if let Err(e) = guard.refresh() {
        warn!(error = %e, "Failed to refresh values storage");
    }
    match guard.get_value(ADMIN_REMOTE_ACCESS_ENABLED_KEY) {
        Some(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        None => false,
    }
}

pub fn get_admin_username(state: &AdminState) -> Option<String> {
    let storage = state.values_storage.as_ref()?;
    {
        let mut guard = storage.write();
        if let Err(e) = guard.refresh() {
            warn!(error = %e, "Failed to refresh values storage");
        }
    }
    let guard = storage.read();
    guard
        .get_value(ADMIN_AUTH_USERNAME_KEY)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| Some("admin".to_string()))
}

pub fn ensure_admin_auth_material(state: &AdminState) -> Result<()> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };
    let mut guard = storage.write();
    guard.refresh()?;

    if guard
        .get_value(ADMIN_AUTH_USERNAME_KEY)
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        let _ = guard.set_value(ADMIN_AUTH_USERNAME_KEY, "admin");
    }

    if guard
        .get_value(ADMIN_AUTH_JWT_SECRET_KEY)
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        let secret: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        guard.set_value(ADMIN_AUTH_JWT_SECRET_KEY, &secret)?;
    }

    Ok(())
}

pub fn set_admin_password_hash(state: &AdminState, password: &str) -> Result<()> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };

    if password.is_empty() {
        return Err(BifrostError::Config("Password cannot be empty".to_string()));
    }

    let hashed = hash(password, DEFAULT_COST)
        .map_err(|e| BifrostError::Storage(format!("Failed to hash password: {e}")))?;
    let mut guard = storage.write();
    guard.refresh()?;
    guard.set_value(ADMIN_AUTH_PASSWORD_HASH_KEY, &hashed)?;
    Ok(())
}

pub fn set_remote_access_enabled(state: &AdminState, enabled: bool) -> Result<()> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };
    let mut guard = storage.write();
    guard.refresh()?;
    guard.set_value(
        ADMIN_REMOTE_ACCESS_ENABLED_KEY,
        if enabled { "true" } else { "false" },
    )?;
    Ok(())
}

pub fn set_admin_username(state: &AdminState, username: &str) -> Result<()> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };
    let trimmed = username.trim();
    if trimmed.is_empty() {
        return Err(BifrostError::Config("Username cannot be empty".to_string()));
    }
    let mut guard = storage.write();
    guard.refresh()?;
    guard.set_value(ADMIN_AUTH_USERNAME_KEY, trimmed)?;
    Ok(())
}

pub fn has_admin_password(state: &AdminState) -> bool {
    let Some(storage) = state.values_storage.as_ref() else {
        return false;
    };
    let mut guard = storage.write();
    if let Err(e) = guard.refresh() {
        warn!(error = %e, "Failed to refresh values storage");
    }
    guard
        .get_value(ADMIN_AUTH_PASSWORD_HASH_KEY)
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

pub fn verify_admin_credentials(
    state: &AdminState,
    username: &str,
    password: &str,
) -> Result<bool> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };
    {
        let mut guard = storage.write();
        guard.refresh()?;
    }
    let guard = storage.read();
    let expected_username = guard
        .get_value(ADMIN_AUTH_USERNAME_KEY)
        .unwrap_or_else(|| "admin".to_string());
    if username.trim() != expected_username.trim() {
        return Ok(false);
    }
    let Some(hash_str) = guard.get_value(ADMIN_AUTH_PASSWORD_HASH_KEY) else {
        return Ok(false);
    };
    let ok = verify(password, &hash_str)
        .map_err(|e| BifrostError::Storage(format!("Failed to verify password: {e}")))?;
    Ok(ok)
}

fn jwt_secret(state: &AdminState) -> Result<String> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };
    {
        let mut guard = storage.write();
        guard.refresh()?;
    }
    let guard = storage.read();
    let secret = guard
        .get_value(ADMIN_AUTH_JWT_SECRET_KEY)
        .ok_or_else(|| BifrostError::Config("JWT secret not initialized".to_string()))?;
    let secret = secret.trim().to_string();
    if secret.is_empty() {
        return Err(BifrostError::Config("JWT secret is empty".to_string()));
    }
    Ok(secret)
}

pub fn issue_admin_jwt(state: &AdminState, username: &str) -> Result<(String, AdminJwtClaims)> {
    ensure_admin_auth_material(state)?;
    let now = now_unix();
    let exp = now + JWT_TTL.as_secs() as i64;
    let claims = AdminJwtClaims {
        sub: username.to_string(),
        iat: now,
        exp,
        jti: uuid::Uuid::new_v4().to_string(),
    };

    let secret = jwt_secret(state)?;
    let mut header = Header::new(Algorithm::HS256);
    header.typ = Some("JWT".to_string());
    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| BifrostError::Storage(format!("Failed to encode JWT: {e}")))?;
    Ok((token, claims))
}

pub fn revoke_all_admin_sessions(state: &AdminState) -> Result<()> {
    let Some(storage) = state.values_storage.as_ref() else {
        return Err(BifrostError::Config(
            "Values storage not configured".to_string(),
        ));
    };
    let ts = now_unix();
    let mut guard = storage.write();
    guard.refresh()?;
    guard.set_value(ADMIN_AUTH_REVOKE_BEFORE_KEY, &ts.to_string())?;
    Ok(())
}

pub fn validate_admin_jwt(state: &AdminState, token: &str) -> Result<AdminJwtClaims> {
    ensure_admin_auth_material(state)?;
    let secret = jwt_secret(state)?;

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.leeway = 5;

    let data = jsonwebtoken::decode::<AdminJwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| BifrostError::Proxy(format!("Invalid token: {e}")))?;

    let claims = data.claims;

    // 严格控制生命周期：exp - iat <= 7 天
    if claims.exp <= claims.iat {
        return Err(BifrostError::Proxy("Invalid token timestamps".to_string()));
    }
    let ttl = claims.exp - claims.iat;
    if ttl > JWT_TTL.as_secs() as i64 {
        return Err(BifrostError::Proxy(
            "Token lifetime exceeds 7 days".to_string(),
        ));
    }

    // revoke-all：iat 必须 > revoke_before（秒级时间戳，使用 <= 兼容同秒 revoke）
    if let Some(storage) = state.values_storage.as_ref() {
        let mut guard = storage.write();
        guard.refresh()?;
        if let Some(revoke_before) = guard
            .get_value(ADMIN_AUTH_REVOKE_BEFORE_KEY)
            .and_then(|s| s.trim().parse::<i64>().ok())
        {
            if claims.iat <= revoke_before {
                return Err(BifrostError::Proxy("Token revoked".to_string()));
            }
        }
    }

    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bifrost_storage::ValuesStorage;

    fn new_state() -> (AdminState, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let values_dir = tmp.path().join("values");
        let storage = ValuesStorage::with_dir(values_dir).expect("values storage");

        (AdminState::new(19999).with_values_storage(storage), tmp)
    }

    fn encode_with_secret(secret: &str, claims: &AdminJwtClaims) -> String {
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("JWT".to_string());
        jsonwebtoken::encode(
            &header,
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("encode jwt")
    }

    #[test]
    fn test_issue_admin_jwt_sets_7d_ttl_and_round_trip_validate() {
        let (state, _tmp) = new_state();
        let (token, claims) = issue_admin_jwt(&state, "admin").expect("issue token");
        assert!(claims.exp > claims.iat);
        assert_eq!(claims.exp - claims.iat, JWT_TTL.as_secs() as i64);

        let parsed = validate_admin_jwt(&state, &token).expect("validate token");
        assert_eq!(parsed.sub, "admin");
        assert_eq!(parsed.iat, claims.iat);
        assert_eq!(parsed.exp, claims.exp);
        assert_eq!(parsed.jti, claims.jti);
    }

    #[test]
    fn test_validate_admin_jwt_rejects_invalid_timestamps_and_too_long_ttl() {
        let (state, _tmp) = new_state();
        ensure_admin_auth_material(&state).expect("init auth material");
        let secret = state
            .values_storage
            .as_ref()
            .unwrap()
            .read()
            .get_value(ADMIN_AUTH_JWT_SECRET_KEY)
            .expect("jwt secret");

        let now = now_unix();

        // exp 仍在未来，但 iat > exp，应被自定义校验拒绝。
        let claims = AdminJwtClaims {
            sub: "admin".to_string(),
            iat: now + 120,
            exp: now + 60,
            jti: uuid::Uuid::new_v4().to_string(),
        };
        let token = encode_with_secret(&secret, &claims);
        let err = validate_admin_jwt(&state, &token).unwrap_err().to_string();
        assert!(err.contains("Invalid token timestamps"));

        // TTL 超过 7 天，应被自定义校验拒绝。
        let claims = AdminJwtClaims {
            sub: "admin".to_string(),
            iat: now,
            exp: now + (JWT_TTL.as_secs() as i64) + 60,
            jti: uuid::Uuid::new_v4().to_string(),
        };
        let token = encode_with_secret(&secret, &claims);
        let err = validate_admin_jwt(&state, &token).unwrap_err().to_string();
        assert!(err.contains("Token lifetime exceeds 7 days"));
    }

    #[test]
    fn test_validate_admin_jwt_respects_revoke_before() {
        let (state, _tmp) = new_state();
        ensure_admin_auth_material(&state).expect("init auth material");
        let secret = state
            .values_storage
            .as_ref()
            .unwrap()
            .read()
            .get_value(ADMIN_AUTH_JWT_SECRET_KEY)
            .expect("jwt secret");

        let now = now_unix();
        let revoke_before = now;
        state
            .values_storage
            .as_ref()
            .unwrap()
            .write()
            .set_value(ADMIN_AUTH_REVOKE_BEFORE_KEY, &revoke_before.to_string())
            .expect("set revoke_before");

        let claims = AdminJwtClaims {
            sub: "admin".to_string(),
            iat: now,
            exp: now + 60,
            jti: uuid::Uuid::new_v4().to_string(),
        };
        let token = encode_with_secret(&secret, &claims);
        let err = validate_admin_jwt(&state, &token).unwrap_err().to_string();
        assert!(err.contains("Token revoked"));
    }

    #[test]
    fn test_set_admin_password_hash_and_verify_credentials() {
        let (state, _tmp) = new_state();
        assert!(!has_admin_password(&state));

        set_admin_password_hash(&state, "testpass123").expect("set password");
        assert!(has_admin_password(&state));

        let ok = verify_admin_credentials(&state, "admin", "testpass123").expect("verify");
        assert!(ok);

        let wrong = verify_admin_credentials(&state, "admin", "wrongpass").expect("verify wrong");
        assert!(!wrong);
    }

    #[test]
    fn test_set_admin_password_rejects_empty() {
        let (state, _tmp) = new_state();
        let err = set_admin_password_hash(&state, "");
        assert!(err.is_err());
    }

    #[test]
    fn test_set_and_get_admin_username() {
        let (state, _tmp) = new_state();
        set_admin_username(&state, "myuser").expect("set username");
        let username = get_admin_username(&state);
        assert_eq!(username, Some("myuser".to_string()));
    }

    #[test]
    fn test_set_admin_username_rejects_empty() {
        let (state, _tmp) = new_state();
        let err = set_admin_username(&state, "   ");
        assert!(err.is_err());
    }

    #[test]
    fn test_set_remote_access_enabled_toggle() {
        let (state, _tmp) = new_state();
        assert!(!is_remote_access_enabled(&state));

        set_remote_access_enabled(&state, true).expect("enable");
        assert!(is_remote_access_enabled(&state));

        set_remote_access_enabled(&state, false).expect("disable");
        assert!(!is_remote_access_enabled(&state));
    }

    #[test]
    fn test_revoke_all_invalidates_existing_tokens() {
        let (state, _tmp) = new_state();
        let (token, _claims) = issue_admin_jwt(&state, "admin").expect("issue token");
        validate_admin_jwt(&state, &token).expect("token should be valid before revoke");

        std::thread::sleep(std::time::Duration::from_secs(1));

        revoke_all_admin_sessions(&state).expect("revoke all");
        let err = validate_admin_jwt(&state, &token).unwrap_err().to_string();
        assert!(err.contains("Token revoked"));
    }

    #[test]
    fn test_has_admin_password_with_custom_username_and_credentials() {
        let (state, _tmp) = new_state();
        set_admin_username(&state, "operator").expect("set username");
        set_admin_password_hash(&state, "securepass").expect("set password");

        assert!(has_admin_password(&state));
        let ok = verify_admin_credentials(&state, "operator", "securepass").expect("verify");
        assert!(ok);

        let wrong_user =
            verify_admin_credentials(&state, "admin", "securepass").expect("wrong user");
        assert!(!wrong_user);
    }
}
