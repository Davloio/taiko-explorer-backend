use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::models::block::Block;
use crate::models::transaction::Transaction;
use crate::models::address_analytics::AddressStats;

#[derive(Clone, Debug)]
pub enum WebSocketMessage {
    NewBlock(Block),
    NewTransaction(Transaction),
    Stats { 
        total_blocks: i64, 
        latest_block: Option<i64>, 
        total_transactions: i64,
        total_addresses: i64,
    },
    NewAddress { address: String },
    AddressActivity { address: String, transaction_hash: String, transaction_type: String },
    AddressStatsUpdate(AddressStats),
    NewBlockHeight { 
        block_number: u64, 
        miner: String,
        transaction_count: i32,
        timestamp: i64,
    },
    BlockComplete { block_number: u64 },
}

#[derive(Clone)]
pub struct WebSocketBroadcaster {
    sender: broadcast::Sender<WebSocketMessage>,
    connections: Arc<RwLock<HashMap<String, broadcast::Receiver<WebSocketMessage>>>>,
}

impl WebSocketBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000);
        
        Self {
            sender,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Broadcast a new block to all connected clients
    pub async fn broadcast_new_block(&self, block: Block) {
        let msg = WebSocketMessage::NewBlock(block);
        if let Err(e) = self.sender.send(msg) {
            warn!("Failed to broadcast new block: {}", e);
        } else {
            info!("🌐 Broadcasted new block to {} connections", self.connection_count().await);
        }
    }

    /// Broadcast a new transaction to all connected clients
    pub async fn broadcast_new_transaction(&self, transaction: Transaction) {
        let msg = WebSocketMessage::NewTransaction(transaction);
        if let Err(e) = self.sender.send(msg) {
            warn!("Failed to broadcast new transaction: {}", e);
        }
    }

    /// Broadcast current stats (total blocks, latest block, total transactions, total addresses)
    pub async fn broadcast_stats(&self, total_blocks: i64, latest_block: Option<i64>, total_transactions: i64, total_addresses: i64) {
        let msg = WebSocketMessage::Stats { 
            total_blocks, 
            latest_block, 
            total_transactions,
            total_addresses,
        };
        if let Err(e) = self.sender.send(msg) {
            warn!("Failed to broadcast stats: {}", e);
        } else {
            info!("🌐 Broadcasted stats update to {} connections", self.connection_count().await);
        }
    }

    /// Broadcast when a new address appears on the network
    pub async fn broadcast_new_address(&self, address: String) {
        let msg = WebSocketMessage::NewAddress { address: address.clone() };
        if let Err(e) = self.sender.send(msg) {
            warn!("Failed to broadcast new address: {}", e);
        } else {
            info!("🌐 Broadcasted new address {} to {} connections", address, self.connection_count().await);
        }
    }

    /// Broadcast address activity (new transaction involving an address)
    pub async fn broadcast_address_activity(&self, address: String, transaction_hash: String, transaction_type: String) {
        let msg = WebSocketMessage::AddressActivity { 
            address: address.clone(), 
            transaction_hash: transaction_hash.clone(), 
            transaction_type 
        };
        if let Err(e) = self.sender.send(msg) {
            warn!("Failed to broadcast address activity: {}", e);
        } else {
            info!("🌐 Broadcasted activity for address {} (tx: {}) to {} connections", 
                  address, transaction_hash, self.connection_count().await);
        }
    }

    /// Broadcast updated address statistics
    pub async fn broadcast_address_stats_update(&self, address_stats: AddressStats) {
        let msg = WebSocketMessage::AddressStatsUpdate(address_stats.clone());
        if let Err(e) = self.sender.send(msg) {
            warn!("Failed to broadcast address stats update: {}", e);
        } else {
            info!("🌐 Broadcasted stats update for address {} to {} connections", 
                  address_stats.address, self.connection_count().await);
        }
    }

    /// Broadcast when a block is completely processed (with all transactions)
    pub async fn broadcast_live_block_complete(&self, block: Block) {
        // Send both block completion AND new_block message for frontend
        let completion_msg = WebSocketMessage::BlockComplete { block_number: block.number as u64 };
        let new_block_msg = WebSocketMessage::NewBlock(block.clone());
        
        if let Err(e) = self.sender.send(completion_msg) {
            warn!("Failed to broadcast block completion: {}", e);
        }
        
        if let Err(e) = self.sender.send(new_block_msg) {
            warn!("Failed to broadcast new block: {}", e);
        } else {
            info!("📡 COMPLETE: Broadcasting complete block #{} to {} connections", 
                  block.number, self.connection_count().await);
        }
    }

    /// Broadcast new block height immediately (before transactions are processed)
    pub async fn broadcast_new_block_height(&self, block_number: u64, miner: String, timestamp: i64) {
        let height_msg = WebSocketMessage::NewBlockHeight { 
            block_number,
            miner,
            transaction_count: 0, // Will be updated later when transactions are processed
            timestamp,
        };
        
        if let Err(e) = self.sender.send(height_msg) {
            warn!("Failed to broadcast new block height: {}", e);
        } else {
            info!("📡 INSTANT: Broadcasting block height #{} to {} connections", 
                  block_number, self.connection_count().await);
        }
    }

    /// Get number of active connections
    async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Add a new connection
    async fn add_connection(&self) -> (String, broadcast::Receiver<WebSocketMessage>) {
        let connection_id = Uuid::new_v4().to_string();
        let receiver = self.sender.subscribe();
        
        self.connections.write().await.insert(connection_id.clone(), self.sender.subscribe());
        info!("🔌 New WebSocket connection: {} (total: {})", connection_id, self.connection_count().await + 1);
        
        (connection_id, receiver)
    }

    /// Remove a connection
    async fn remove_connection(&self, connection_id: &str) {
        self.connections.write().await.remove(connection_id);
        info!("🔌 WebSocket disconnected: {} (total: {})", connection_id, self.connection_count().await);
    }
}

impl Default for WebSocketBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    Extension(broadcaster): Extension<Arc<WebSocketBroadcaster>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, broadcaster))
}

async fn handle_websocket(socket: WebSocket, broadcaster: Arc<WebSocketBroadcaster>) {
    let (connection_id, mut receiver) = broadcaster.add_connection().await;
    let (mut sender, mut receiver_ws) = socket.split();

    // Send welcome message
    let welcome_msg = json!({
        "type": "connected",
        "message": "Connected to Taiko Explorer WebSocket",
        "connection_id": connection_id
    });
    
    if let Err(e) = sender.send(Message::Text(welcome_msg.to_string().into())).await {
        error!("Failed to send welcome message: {}", e);
        broadcaster.remove_connection(&connection_id).await;
        return;
    }

    // Spawn task to handle incoming messages from client
    let broadcaster_clone = broadcaster.clone();
    let connection_id_clone = connection_id.clone();
    let incoming_task = tokio::spawn(async move {
        while let Some(msg) = receiver_ws.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    info!("Received message from {}: {}", connection_id_clone, text);
                    // Handle ping/pong or subscription requests here if needed
                }
                Ok(Message::Close(_)) => {
                    info!("Connection {} requested close", connection_id_clone);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error for {}: {}", connection_id_clone, e);
                    break;
                }
                _ => {}
            }
        }
        broadcaster_clone.remove_connection(&connection_id_clone).await;
    });

    // Handle outgoing messages (broadcasts)
    while let Ok(broadcast_msg) = receiver.recv().await {
        let json_msg = match broadcast_msg {
            WebSocketMessage::NewBlock(block) => {
                json!({
                    "type": "new_block",
                    "data": {
                        "number": block.number,
                        "hash": block.hash,
                        "timestamp": block.timestamp,
                        "transaction_count": block.transaction_count,
                        "gas_used": block.gas_used,
                        "gas_limit": block.gas_limit,
                        "miner": block.miner
                    }
                })
            }
            WebSocketMessage::NewTransaction(transaction) => {
                json!({
                    "type": "new_transaction", 
                    "data": {
                        "hash": transaction.hash,
                        "block_number": transaction.block_number,
                        "from_address": transaction.from_address,
                        "to_address": transaction.to_address,
                        "value": transaction.value.to_string(),
                        "gas_used": transaction.gas_used,
                        "status": transaction.status
                    }
                })
            }
            WebSocketMessage::Stats { total_blocks, latest_block, total_transactions, total_addresses } => {
                json!({
                    "type": "stats",
                    "data": {
                        "total_blocks": total_blocks,
                        "latest_block": latest_block.unwrap_or(0),
                        "total_transactions": total_transactions,
                        "total_addresses": total_addresses
                    }
                })
            }
            WebSocketMessage::NewAddress { address } => {
                json!({
                    "type": "new_address",
                    "data": {
                        "address": address
                    }
                })
            }
            WebSocketMessage::AddressActivity { address, transaction_hash, transaction_type } => {
                json!({
                    "type": "address_activity",
                    "data": {
                        "address": address,
                        "transaction_hash": transaction_hash,
                        "transaction_type": transaction_type
                    }
                })
            }
            WebSocketMessage::AddressStatsUpdate(address_stats) => {
                json!({
                    "type": "address_stats_update",
                    "data": {
                        "address": address_stats.address,
                        "total_transactions": address_stats.total_transactions,
                        "total_sent_transactions": address_stats.total_sent_transactions,
                        "total_received_transactions": address_stats.total_received_transactions,
                        "gas_used": address_stats.gas_used,
                        "unique_counterparties": address_stats.unique_counterparties,
                        "contract_deployments": address_stats.contract_deployments,
                        "first_seen_block": address_stats.first_seen_block,
                        "last_seen_block": address_stats.last_seen_block
                    }
                })
            }
            WebSocketMessage::NewBlockHeight { block_number, miner, transaction_count, timestamp } => {
                json!({
                    "type": "new_block_height",
                    "data": {
                        "block_number": block_number,
                        "timestamp": timestamp,
                        "status": "height_stored",
                        "miner": miner,
                        "transaction_count": transaction_count
                    }
                })
            }
            WebSocketMessage::BlockComplete { block_number } => {
                json!({
                    "type": "block_complete",
                    "data": {
                        "block_number": block_number,
                        "timestamp": chrono::Utc::now().timestamp(),
                        "status": "transactions_processed"
                    }
                })
            }
        };

        if let Err(e) = sender.send(Message::Text(json_msg.to_string().into())).await {
            error!("Failed to send message to {}: {}", connection_id, e);
            break;
        }
    }

    // Clean up
    incoming_task.abort();
    broadcaster.remove_connection(&connection_id).await;
    info!("WebSocket handler completed for {}", connection_id);
}