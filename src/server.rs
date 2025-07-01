use anyhow::Result;
use tonic::{transport::Server, Request, Response, Status};
use tracing::info;

use crate::database::Database;

pub mod proto {
    tonic::include_proto!("admin");
}

use proto::admin_sync_server::{AdminSync, AdminSyncServer};
use proto::*;

pub struct AdminServer {
    db: Database,
}

impl AdminServer {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn serve(self) -> Result<()> {
        let addr = "0.0.0.0:50051".parse()?;
        
        info!("gRPC server listening on {}", addr);

        Server::builder()
            .add_service(AdminSyncServer::new(self))
            .serve(addr)
            .await?;

        Ok(())
    }
}

#[tonic::async_trait]
impl AdminSync for AdminServer {
    async fn get_sync_status(
        &self,
        request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
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
        _request: Request<tonic::Streaming<FeatureOverride>>,
    ) -> Result<Response<PushResponse>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
    }

    async fn pull_feature_overrides(
        &self,
        _request: Request<PullRequest>,
    ) -> Result<Response<Self::PullFeatureOverridesStream>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
    }

    type PullFeatureOverridesStream = 
        tokio_stream::wrappers::ReceiverStream<Result<FeatureOverride, Status>>;

    async fn confirm_features(
        &self,
        _request: Request<ConfirmRequest>,
    ) -> Result<Response<ConfirmResponse>, Status> {
        Err(Status::unimplemented("Not implemented yet"))
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