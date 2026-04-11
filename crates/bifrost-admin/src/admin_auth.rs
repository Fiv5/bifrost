use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bcrypt::{hash, verify, DEFAULT_COST};
use bifrost_core::{BifrostError, Result};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

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
    let guard = storage.read();
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

#[allow(dead_code)]
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
    storage
        .write()
        .set_value(ADMIN_AUTH_PASSWORD_HASH_KEY, &hashed)?;
    Ok(())
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
    storage
        .write()
        .set_value(ADMIN_AUTH_REVOKE_BEFORE_KEY, &ts.to_string())?;
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

    // revoke-all：iat 必须 >= revoke_before
    if let Some(storage) = state.values_storage.as_ref() {
        if let Some(revoke_before) = storage
            .read()
            .get_value(ADMIN_AUTH_REVOKE_BEFORE_KEY)
            .and_then(|s| s.trim().parse::<i64>().ok())
        {
            if claims.iat < revoke_before {
                return Err(BifrostError::Proxy("Token revoked".to_string()));
            }
        }
    }

    Ok(claims)
}
