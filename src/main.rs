mod api;
mod bridge;
mod config;
mod db;
mod graphql;
mod mega_batch_indexer;
mod models;
mod rpc;
mod schema;
mod websocket;

mod analytics {
    pub mod analytics;
}

use anyhow::Result;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use crate::api::server::create_router;
use crate::config::Config;
use crate::db::connection::establish_connection;
use crate::db::operations::BlockRepository;
use crate::db::address_analytics::AddressAnalyticsRepository;
use crate::mega_batch_indexer::MegaBatchIndexer;
use crate::websocket::WebSocketBroadcaster;
use std::sync::Arc;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    
    init_logging(&config.log_level)?;
    
    info!("Starting Taiko Explorer Backend");
    info!("Chain ID: {}", config.taiko_chain_id);
    
    let pool = establish_connection();
    
    run_migrations(&pool)?;
    
    info!("🔧 Initializing database repositories and WebSocket broadcaster");
    let block_repo = BlockRepository::new(pool.clone());
    let analytics_repo = AddressAnalyticsRepository::new(pool);
    let websocket_broadcaster = Arc::new(WebSocketBroadcaster::new());
    
    info!("🚀 Initializing MEGA BATCH INDEXER");
    info!("🎯 AGGRESSIVE settings: 1000-block batches, 1000-receipt mega batches, ultra-massive bulk storage");
    let indexer = MegaBatchIndexer::new(
        block_repo.clone(), 
        analytics_repo.clone(), 
        websocket_broadcaster.clone(),
        config.taiko_rpc_nodes
    ).await?;
    
    let app = create_router(block_repo, analytics_repo, websocket_broadcaster.clone());
    
    let indexer_handle = tokio::spawn(async move {
        if let Err(e) = indexer.start_mega_batching().await {
            tracing::error!("MEGA BATCH indexer failed: {}", e);
        }
    });
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("🚀 GraphQL server starting on http://localhost:3000");
    info!("📊 GraphQL Playground available at http://localhost:3000");
    info!("🔍 Health check at http://localhost:3000/health");
    info!("🚀 MEGA BATCH indexer: 1000-block batches, ultra-aggressive performance, massive bulk operations");
    
    let server_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app.into_make_service()).await {
            tracing::error!("Server failed: {}", e);
        }
    });
    
    tokio::select! {
        _ = indexer_handle => info!("Indexer completed"),
        _ = server_handle => info!("Server stopped"),
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        }
    }
    
    Ok(())
}

fn init_logging(log_level: &str) -> Result<()> {
    let level = match log_level.to_lowercase().as_str() {
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;
    
    Ok(())
}

fn run_migrations(pool: &crate::db::connection::DbPool) -> Result<()> {
    info!("Running database migrations...");
    
    let mut conn = pool.get()?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!("Migration error: {}", e))?;
    
    info!("Migrations completed successfully");
    Ok(())
}
