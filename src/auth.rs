use anyhow::Result;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use sqlx::SqlitePool;
use tonic::{Request, Status};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub key_hash: String,
    pub client_name: String,
    pub permissions: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

pub struct AuthService {
    pool: SqlitePool,
}

impl AuthService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn generate_api_key(&self, client_name: &str, permissions: &str) -> Result<String> {
        let raw_key = format!("ADM_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let key_hash = self.hash_key(&raw_key)?;

        sqlx::query!(
            "INSERT INTO api_keys (key_hash, client_name, permissions, created_at) 
             VALUES (?, ?, ?, datetime('now'))",
            key_hash,
            client_name,
            permissions
        )
        .execute(&self.pool)
        .await?;

        info!("Generated API key for client: {}", client_name);
        Ok(raw_key)
    }

    pub async fn verify_api_key(&self, raw_key: &str) -> Result<Option<ApiKey>> {
        let api_keys = sqlx::query_as!(
            ApiKey,
            "SELECT key_hash, client_name, permissions, created_at, last_used_at 
             FROM api_keys"
        )
        .fetch_all(&self.pool)
        .await?;

        for api_key in api_keys {
            if self.verify_key(raw_key, &api_key.key_hash)? {
                self.update_last_used(&api_key.key_hash).await?;
                return Ok(Some(api_key));
            }
        }

        Ok(None)
    }

    async fn update_last_used(&self, key_hash: &str) -> Result<()> {
        sqlx::query!(
            "UPDATE api_keys SET last_used_at = datetime('now') WHERE key_hash = ?",
            key_hash
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn hash_key(&self, raw_key: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(raw_key.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Hash error: {}", e))?;
        Ok(password_hash.to_string())
    }

    fn verify_key(&self, raw_key: &str, hash: &str) -> Result<bool> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| anyhow::anyhow!("Parse hash error: {}", e))?;
        let argon2 = Argon2::default();
        Ok(argon2.verify_password(raw_key.as_bytes(), &parsed_hash).is_ok())
    }
}

pub fn extract_api_key<T>(request: &Request<T>) -> Result<String, Status> {
    let metadata = request.metadata();
    
    if let Some(api_key) = metadata.get("api-key") {
        api_key
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| Status::unauthenticated("Invalid API key format"))
    } else {
        Err(Status::unauthenticated("API key required"))
    }
}

pub async fn authenticate_request<T>(
    request: &Request<T>,
    auth_service: &AuthService,
) -> Result<ApiKey, Status> {
    let api_key = extract_api_key(request)?;
    
    match auth_service.verify_api_key(&api_key).await {
        Ok(Some(api_key_info)) => {
            info!("Authenticated request from client: {}", api_key_info.client_name);
            Ok(api_key_info)
        }
        Ok(None) => {
            warn!("Invalid API key provided");
            Err(Status::unauthenticated("Invalid API key"))
        }
        Err(e) => {
            warn!("Authentication error: {}", e);
            Err(Status::internal("Authentication service error"))
        }
    }
}

pub fn require_write_permission(api_key: &ApiKey) -> Result<(), Status> {
    if api_key.permissions == "read_write" {
        Ok(())
    } else {
        Err(Status::permission_denied("Write permission required"))
    }
}