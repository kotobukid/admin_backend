use anyhow::Result;
use sqlx::Row;
use std::env;
use tonic::{transport::{Server, Identity, ServerTlsConfig}, Request, Response, Status};
use tracing::{info, warn};

use crate::auth::{AuthService, authenticate_request, require_write_permission};
use crate::database::Database;

pub mod proto {
    tonic::include_proto!("admin");
}

use proto::admin_sync_server::{AdminSync, AdminSyncServer};
use proto::*;

pub struct AdminServer {
    db: Database,
    auth: AuthService,
}

impl AdminServer {
    pub fn new(db: Database) -> Self {
        let auth = AuthService::new(db.pool().clone());
        Self { db, auth }
    }

    pub async fn serve(self) -> Result<()> {
        let addr = "0.0.0.0:50051".parse()?;
        
        let mut server_builder = Server::builder();

        // TLS設定の確認
        if let (Ok(cert_path), Ok(key_path)) = (
            env::var("TLS_CERT_PATH"),
            env::var("TLS_KEY_PATH")
        ) {
            match self.setup_tls(&cert_path, &key_path).await {
                Ok(tls_config) => {
                    info!("TLS enabled - gRPC server listening on {} with TLS", addr);
                    server_builder = server_builder.tls_config(tls_config)?;
                },
                Err(e) => {
                    warn!("Failed to setup TLS: {}. Falling back to plaintext", e);
                    info!("gRPC server listening on {} (plaintext)", addr);
                }
            }
        } else {
            info!("TLS not configured - gRPC server listening on {} (plaintext)", addr);
        }

        server_builder
            .add_service(AdminSyncServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }

    async fn setup_tls(&self, cert_path: &str, key_path: &str) -> Result<ServerTlsConfig> {
        let cert = tokio::fs::read(cert_path).await
            .map_err(|e| anyhow::anyhow!("Failed to read certificate file {}: {}", cert_path, e))?;
        let key = tokio::fs::read(key_path).await
            .map_err(|e| anyhow::anyhow!("Failed to read private key file {}: {}", key_path, e))?;

        let identity = Identity::from_pem(cert, key);
        Ok(ServerTlsConfig::new().identity(identity))
    }
}

#[tonic::async_trait]
impl AdminSync for AdminServer {
    async fn get_sync_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let _api_key = authenticate_request(&request, &self.auth).await?;
        let req = request.into_inner();
        info!("GetSyncStatus request from client: {}", req.client_id);

        let response = StatusResponse {
            server_time: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
            sync_status: std::collections::HashMap::new(),
            total_feature_overrides: 0,
            total_confirmed_features: 0,
            total_rule_patterns: 0,
        };

        Ok(Response::new(response))
    }

    async fn push_feature_overrides(
        &self,
        request: Request<tonic::Streaming<FeatureOverride>>,
    ) -> Result<Response<PushResponse>, Status> {
        let api_key_result = {
            let metadata = request.metadata();
            if let Some(api_key_value) = metadata.get("api-key") {
                api_key_value.to_str()
                    .map_err(|_| Status::unauthenticated("Invalid API key format"))
                    .and_then(|key| Ok(key.to_string()))
            } else {
                Err(Status::unauthenticated("API key required"))
            }
        }?;

        let api_key = self.auth.verify_api_key(&api_key_result).await
            .map_err(|_| Status::internal("Authentication service error"))?
            .ok_or_else(|| Status::unauthenticated("Invalid API key"))?;

        require_write_permission(&api_key)?;

        let mut stream = request.into_inner();
        let mut items_received = 0;
        let mut items_updated = 0;
        let mut items_created = 0;
        let mut errors = Vec::new();

        while let Some(feature_override) = stream.message().await? {
            items_received += 1;
            
            match self.upsert_feature_override(&feature_override).await {
                Ok(was_updated) => {
                    if was_updated {
                        items_updated += 1;
                    } else {
                        items_created += 1;
                    }
                }
                Err(e) => {
                    errors.push(format!("Error processing {}: {}", feature_override.pronunciation, e));
                }
            }
        }

        info!(
            "PushFeatureOverrides completed: {} received, {} created, {} updated, {} errors",
            items_received, items_created, items_updated, errors.len()
        );

        let response = PushResponse {
            items_received,
            items_updated,
            items_created,
            errors,
        };

        Ok(Response::new(response))
    }

    async fn pull_feature_overrides(
        &self,
        request: Request<PullRequest>,
    ) -> Result<Response<Self::PullFeatureOverridesStream>, Status> {
        let _api_key = authenticate_request(&request, &self.auth).await?;
        let req = request.into_inner();
        
        let since_clause = if let Some(since) = req.since {
            let since_time = chrono::DateTime::from_timestamp(since.seconds, since.nanos as u32)
                .unwrap()
                .to_rfc3339();
            format!("WHERE updated_at > '{}'", since_time)
        } else {
            String::new()
        };

        let limit_clause = req.limit
            .map(|l| format!("LIMIT {}", l))
            .unwrap_or_default();

        let query = format!(
            "SELECT pronunciation, fixed_bits1, fixed_bits2, fixed_burst_bits, created_at, updated_at, note 
             FROM card_feature_override {} ORDER BY updated_at ASC {}",
            since_clause, limit_clause
        );

        let rows = sqlx::query(&query)
            .fetch_all(self.db.pool())
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            for row in rows {
                let pronunciation: String = row.get("pronunciation");
                let fixed_bits1: i64 = row.get("fixed_bits1");
                let fixed_bits2: i64 = row.get("fixed_bits2");
                let fixed_burst_bits: i64 = row.get("fixed_burst_bits");
                let created_at: String = row.get("created_at");
                let updated_at: String = row.get("updated_at");
                let note: Option<String> = row.get("note");

                let created_at_ts = chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| prost_types::Timestamp {
                        seconds: dt.timestamp(),
                        nanos: dt.timestamp_subsec_nanos() as i32,
                    })
                    .ok();

                let updated_at_ts = chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map(|dt| prost_types::Timestamp {
                        seconds: dt.timestamp(),
                        nanos: dt.timestamp_subsec_nanos() as i32,
                    })
                    .ok();

                let feature_override = FeatureOverride {
                    pronunciation,
                    fixed_bits1,
                    fixed_bits2,
                    fixed_burst_bits,
                    created_at: created_at_ts,
                    updated_at: updated_at_ts,
                    note,
                };

                if tx.send(Ok(feature_override)).await.is_err() {
                    break;
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(stream))
    }

    type PullFeatureOverridesStream = 
        tokio_stream::wrappers::ReceiverStream<Result<FeatureOverride, Status>>;

    async fn confirm_features(
        &self,
        request: Request<ConfirmRequest>,
    ) -> Result<Response<ConfirmResponse>, Status> {
        let api_key = authenticate_request(&request, &self.auth).await?;
        require_write_permission(&api_key)?;
        
        let req = request.into_inner();

        match sqlx::query!(
            "INSERT OR REPLACE INTO feature_confirmation 
             (pronunciation, confirmed_at, confirmed_by, rule_version, feature_bits1, feature_bits2, burst_bits)
             VALUES (?, datetime('now'), ?, ?, ?, ?, ?)",
            req.pronunciation,
            api_key.client_name,
            req.rule_version,
            req.feature_bits1,
            req.feature_bits2,
            req.burst_bits
        )
        .execute(self.db.pool())
        .await
        {
            Ok(_) => {
                info!("Features confirmed for pronunciation: {}", req.pronunciation);
                Ok(Response::new(ConfirmResponse {
                    success: true,
                    error: None,
                }))
            }
            Err(e) => {
                let error_msg = format!("Failed to confirm features: {}", e);
                Ok(Response::new(ConfirmResponse {
                    success: false,
                    error: Some(error_msg),
                }))
            }
        }
    }

    async fn get_confirmed_features(
        &self,
        _request: Request<()>,
    ) -> Result<Response<Self::GetConfirmedFeaturesStream>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
    }

    type GetConfirmedFeaturesStream = 
        tokio_stream::wrappers::ReceiverStream<Result<ConfirmedFeature, Status>>;

    async fn unconfirm_feature(
        &self,
        _request: Request<UnconfirmRequest>,
    ) -> Result<Response<UnconfirmResponse>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
    }

    async fn push_rule_patterns(
        &self,
        _request: Request<tonic::Streaming<RulePattern>>,
    ) -> Result<Response<PushResponse>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
    }

    async fn pull_rule_patterns(
        &self,
        _request: Request<PullRequest>,
    ) -> Result<Response<Self::PullRulePatternsStream>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
    }

    type PullRulePatternsStream = 
        tokio_stream::wrappers::ReceiverStream<Result<RulePattern, Status>>;

    async fn record_sync(
        &self,
        _request: Request<SyncRecord>,
    ) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}

impl AdminServer {
    async fn upsert_feature_override(&self, feature_override: &FeatureOverride) -> Result<bool, anyhow::Error> {
        let created_at = feature_override.created_at
            .as_ref()
            .map(|ts| chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32).unwrap())
            .unwrap_or_else(chrono::Utc::now);
        
        let updated_at = feature_override.updated_at
            .as_ref()
            .map(|ts| chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32).unwrap())
            .unwrap_or_else(chrono::Utc::now);

        let existing = sqlx::query!(
            "SELECT pronunciation FROM card_feature_override WHERE pronunciation = ?",
            feature_override.pronunciation
        )
        .fetch_optional(self.db.pool())
        .await?;

        let was_updated = existing.is_some();

        let created_at_str = created_at.to_rfc3339();
        let updated_at_str = updated_at.to_rfc3339();

        sqlx::query!(
            "INSERT OR REPLACE INTO card_feature_override 
             (pronunciation, fixed_bits1, fixed_bits2, fixed_burst_bits, created_at, updated_at, note)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            feature_override.pronunciation,
            feature_override.fixed_bits1,
            feature_override.fixed_bits2,
            feature_override.fixed_burst_bits,
            created_at_str,
            updated_at_str,
            feature_override.note
        )
        .execute(self.db.pool())
        .await?;

        Ok(was_updated)
    }
}