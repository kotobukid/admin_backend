use anyhow::Result;
use clap::{Parser, Subcommand};
use sqlx::SqlitePool;
use tracing::{error, info};

#[path = "../auth.rs"]
mod auth;

#[path = "../database.rs"]
mod database;

use auth::{ApiKey, AuthService};
use database::Database;

#[derive(Parser)]
#[command(author, version, about = "Admin Backend CLI - API Key Management Tool", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "sqlite://data/admin.db")]
    database_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new API key
    Generate {
        /// Client name for the API key
        #[arg(short, long)]
        client: String,

        /// Permissions level (read or read_write)
        #[arg(short, long, default_value = "read")]
        permissions: String,
    },

    /// List all API keys
    List {
        /// Show only active keys (used in last 30 days)
        #[arg(short, long)]
        active: bool,
    },

    /// Revoke an API key
    Revoke {
        /// Client name whose key to revoke
        #[arg(short, long)]
        client: String,
    },

    /// Show API key details
    Info {
        /// Client name to show info for
        #[arg(short, long)]
        client: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(false)
        .init();
    
    dotenvy::dotenv().ok();

    let cli = Cli::parse();

    // Override with env var if set
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| cli.database_url.clone());

    // Initialize database
    let db = Database::new(&database_url).await?;
    db.migrate().await?;

    let pool = db.pool();
    let auth_service = AuthService::new(pool.clone());

    match cli.command {
        Commands::Generate { client, permissions } => {
            generate_key(&auth_service, &client, &permissions).await?;
        }
        Commands::List { active } => {
            list_keys(&pool, active).await?;
        }
        Commands::Revoke { client } => {
            revoke_key(&pool, &client).await?;
        }
        Commands::Info { client } => {
            show_key_info(&pool, &client).await?;
        }
    }

    Ok(())
}

async fn generate_key(auth_service: &AuthService, client_name: &str, permissions: &str) -> Result<()> {
    // Validate permissions
    if permissions != "read" && permissions != "read_write" {
        error!("Invalid permissions. Must be 'read' or 'read_write'");
        return Err(anyhow::anyhow!("Invalid permissions"));
    }

    // Check if client already has a key
    let existing = sqlx::query!(
        "SELECT client_name FROM api_keys WHERE client_name = ?",
        client_name
    )
    .fetch_optional(&auth_service.pool)
    .await?;

    if existing.is_some() {
        error!("Client '{}' already has an API key", client_name);
        return Err(anyhow::anyhow!("Client already has an API key"));
    }

    // Generate the key
    let api_key = auth_service.generate_api_key(client_name, permissions).await?;

    println!("\n===== API KEY GENERATED =====");
    println!("Client: {}", client_name);
    println!("Permissions: {}", permissions);
    println!("API Key: {}", api_key);
    println!("=============================");
    println!("\nIMPORTANT: Save this API key securely. It cannot be retrieved later.");
    println!("Use this key in the 'api-key' metadata field when making gRPC requests.");

    Ok(())
}

async fn list_keys(pool: &SqlitePool, active_only: bool) -> Result<()> {
    let keys = if active_only {
        sqlx::query_as!(
            ApiKey,
            r#"
            SELECT key_hash, client_name, permissions, created_at, last_used_at
            FROM api_keys
            WHERE last_used_at IS NOT NULL 
            AND date(last_used_at) >= date('now', '-30 days')
            ORDER BY last_used_at DESC
            "#
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as!(
            ApiKey,
            r#"
            SELECT key_hash, client_name, permissions, created_at, last_used_at
            FROM api_keys
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(pool)
        .await?
    };

    if keys.is_empty() {
        println!("No API keys found.");
        return Ok(());
    }

    println!("\n{:<20} {:<12} {:<20} {:<20}", "Client", "Permissions", "Created", "Last Used");
    println!("{}", "-".repeat(80));

    for key in keys {
        let last_used = key.last_used_at.as_deref().unwrap_or("Never");
        println!(
            "{:<20} {:<12} {:<20} {:<20}",
            key.client_name,
            key.permissions,
            key.created_at,
            last_used
        );
    }

    Ok(())
}

async fn revoke_key(pool: &SqlitePool, client_name: &str) -> Result<()> {
    // Check if key exists
    let existing = sqlx::query!(
        "SELECT client_name FROM api_keys WHERE client_name = ?",
        client_name
    )
    .fetch_optional(pool)
    .await?;

    if existing.is_none() {
        error!("No API key found for client '{}'", client_name);
        return Err(anyhow::anyhow!("Client not found"));
    }

    // Confirm revocation
    println!("Are you sure you want to revoke the API key for '{}'?", client_name);
    println!("This action cannot be undone. Type 'yes' to confirm:");
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    if input.trim().to_lowercase() != "yes" {
        println!("Revocation cancelled.");
        return Ok(());
    }

    // Delete the key
    sqlx::query!(
        "DELETE FROM api_keys WHERE client_name = ?",
        client_name
    )
    .execute(pool)
    .await?;

    info!("API key for '{}' has been revoked", client_name);
    println!("API key for '{}' has been successfully revoked.", client_name);

    Ok(())
}

async fn show_key_info(pool: &SqlitePool, client_name: &str) -> Result<()> {
    let key = sqlx::query_as!(
        ApiKey,
        r#"
        SELECT key_hash, client_name, permissions, created_at, last_used_at
        FROM api_keys
        WHERE client_name = ?
        "#,
        client_name
    )
    .fetch_optional(pool)
    .await?;

    match key {
        Some(key) => {
            println!("\n===== API KEY INFO =====");
            println!("Client: {}", key.client_name);
            println!("Permissions: {}", key.permissions);
            println!("Created: {}", key.created_at);
            println!("Last Used: {}", key.last_used_at.as_deref().unwrap_or("Never"));
            println!("Key Hash: {}...", &key.key_hash[..20]);
            println!("========================");
        }
        None => {
            error!("No API key found for client '{}'", client_name);
            return Err(anyhow::anyhow!("Client not found"));
        }
    }

    Ok(())
}

