use anyhow::Result;
use tracing::info;

mod database;
mod server;

use database::Database;
use server::AdminServer;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    info!("Starting admin_backend server...");

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://data/admin.db".to_string());
    
    let db = Database::new(&database_url).await?;
    db.migrate().await?;
    
    let server = AdminServer::new(db);
    server.serve().await?;

    Ok(())
}