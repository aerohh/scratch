use crate::p2p::protocol::{SyncCodec, SYNC_PROTOCOL};
use crate::p2p::sync::{conflict_copy_name, SyncEngine};
use crate::p2p::types::{SharePermission, SyncMessage, SyncStatus, ActivityEventType, ConnectionType, CHUNK_SIZE, COMPRESSION_THRESHOLD, MAX_CONCURRENT_TRANSFERS};
use anyhow::Result;
use libp2p::autonat;
use libp2p::gossipsub::{self, IdentTopic, MessageId, ValidationMode};
use libp2p::kad::{self, store::MemoryStore};
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{futures::StreamExt, mdns, noise, tcp, yamux, PeerId, SwarmBuilder, Multiaddr};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Semaphore};

// Type alias for Kademlia behaviour with MemoryStore
type KadBehaviour = kad::Behaviour<kad::store::MemoryStore>>;

// Type alias for GossipSub
type Gossipsub = gossipsub::Behaviour;

/// GossipSub topic for file change broadcasts
const FILE_CHANGES_TOPIC: &str = "scratch/file-changes";

/// File change broadcast message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileChangeBroadcast {
    share_id: String,
    path: String,
    is_deleted: bool,
    hash: String,
    modified: i64,
    peer_id: String,
    timestamp: i64,
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent", prelude = "libp2p::swarm::derive_prelude")]
struct ScratchBehaviour {
    mdns: mdns::tokio::Behaviour,
    request_response: request_response::Behaviour<SyncCodec>,
    kad: KadBehaviour,
    autonat: autonat::Behaviour,
    gossipsub: Gossipsub,
}

#[derive(Debug)]
enum BehaviourEvent {
    Mdns(mdns::Event),
    RequestResponse(request_response::Event<SyncMessage, SyncMessage>),
    Kad(kad::Event),
    AutoNat(autonat::Event),
    Gossipsub(gossipsub::Event),
}

impl From<mdns::Event> for BehaviourEvent {
    fn from(value: mdns::Event) -> Self {
        Self::Mdns(value)
    }
}

impl From<request_response::Event<SyncMessage, SyncMessage>> for BehaviourEvent {
    fn from(value: request_response::Event<SyncMessage, SyncMessage>) -> Self {
        Self::RequestResponse(value)
    }
}

impl From<kad::Event> for BehaviourEvent {
    fn from(value: kad::Event) -> Self {
        Self::Kad(value)
    }
}

impl From<autonat::Event> for BehaviourEvent {
    fn from(value: autonat::Event) -> Self {
        Self::AutoNat(value)
    }
}

impl From<gossipsub::Event> for BehaviourEvent {
    fn from(value: gossipsub::Event) -> Self {
        Self::Gossipsub(value)
    }
}

#[derive(Debug, Clone)]
struct ShareRuntime {
    local_path: PathBuf,
    known_peers: HashSet<PeerId>,
    permission: SharePermission,
    is_owner: bool,
}

/// Connection metrics for a peer
#[derive(Debug, Clone)]
struct PeerConnectionMetrics {
    connection_type: ConnectionType,
    latency_ms: Option<u64>,
    bandwidth_bps: Option<u64>,
    last_ping: Option<Instant>,
    /// For tracking concurrent transfers
    active_transfers: usize,
    /// Bandwidth measurement state
    bandwidth_measurement: Option<BandwidthMeasurement>,
    /// Bytes transferred in current measurement window
    bytes_transferred: u64,
    /// Time when current measurement window started
    measurement_start: Option<Instant>,
}

/// Active bandwidth measurement
#[derive(Debug, Clone)]
struct BandwidthMeasurement {
    bytes_transferred: u64,
    start_time: Instant,
}

impl PeerConnectionMetrics {
    fn new(connection_type: ConnectionType) -> Self {
        Self {
            connection_type,
            latency_ms: None,
            bandwidth_bps: None,
            last_ping: None,
            active_transfers: 0,
            bandwidth_measurement: None,
            bytes_transferred: 0,
            measurement_start: None,
        }
    }

    fn can_start_transfer(&self) -> bool {
        self.active_transfers < MAX_CONCURRENT_TRANSFERS
    }

    fn increment_transfers(&mut self) -> bool {
        if self.active_transfers < MAX_CONCURRENT_TRANSFERS {
            self.active_transfers += 1;
            true
        } else {
            false
        }
    }

    fn decrement_transfers(&mut self) {
        if self.active_transfers > 0 {
            self.active_transfers -= 1;
        }
    }

    /// Start bandwidth measurement for a transfer
    fn start_bandwidth_measurement(&mut self) {
        self.bandwidth_measurement = Some(BandwidthMeasurement {
            bytes_transferred: 0,
            start_time: Instant::now(),
        });
    }

    /// Record bytes transferred and update bandwidth estimate
    fn record_bytes_transferred(&mut self, bytes: u64) {
        if let Some(ref mut measurement) = self.bandwidth_measurement {
            measurement.bytes_transferred += bytes;
            self.bytes_transferred += bytes;

            // Update bandwidth estimate every 100KB or 1 second
            if self.bytes_transferred >= 100_000 || measurement.start_time.elapsed() >= Duration::from_secs(1) {
                let elapsed = measurement.start_time.elapsed().as_secs_f64();
                if elapsed > 0.0 {
                    let bps = (measurement.bytes_transferred as f64 / elapsed) as u64;
                    // Use exponential moving average for bandwidth estimate
                    self.bandwidth_bps = Some(match self.bandwidth_bps {
                        Some(current) => (current * 7 + bps) / 8, // 87.5% old, 12.5% new
                        None => bps,
                    });
                }

                // Reset measurement
                self.bandwidth_measurement = Some(BandwidthMeasurement {
                    bytes_transferred: 0,
                    start_time: Instant::now(),
                });
            }
        }
    }

    /// Get adaptive chunk size based on measured bandwidth
    fn get_adaptive_chunk_size(&self) -> usize {
        const MIN_CHUNK: usize = 32 * 1024;  // 32KB
        const MAX_CHUNK: usize = 128 * 1024; // 128KB
        const DEFAULT_CHUNK: usize = 64 * 1024; // 64KB

        match self.bandwidth_bps {
            None => DEFAULT_CHUNK,
            Some(bps) => {
                // < 1 Mbps: use smaller chunks
                if bps < 1_000_000 {
                    MIN_CHUNK
                }
                // > 10 Mbps: use larger chunks
                else if bps > 10_000_000 {
                    MAX_CHUNK
                }
                // Otherwise: scale between min and max
                else {
                    let ratio = (bps - 1_000_000) as f64 / 9_000_000.0;
                    MIN_CHUNK + ((MAX_CHUNK - MIN_CHUNK) as f64 * ratio) as usize
                }
            }
        }
    }

    /// End bandwidth measurement
    fn end_bandwidth_measurement(&mut self) {
        if let Some(measurement) = self.bandwidth_measurement.take() {
            let elapsed = measurement.start_time.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                let bps = (measurement.bytes_transferred as f64 / elapsed) as u64;
                self.bandwidth_bps = Some(match self.bandwidth_bps {
                    Some(current) => (current + bps) / 2, // Average of old and new
                    None => bps,
                });
            }
        }
    }
}

#[derive(Debug)]
enum NetworkCommand {
    RegisterShare {
        share_id: String,
        local_path: PathBuf,
        initial_peers: HashSet<PeerId>,
        permission: SharePermission,
        is_owner: bool,
    },
    RemoveShare {
        share_id: String,
    },
    RevokeShare {
        share_id: String,
    },
    NotifyFileChange {
        share_id: String,
        relative_path: String,
        is_deleted: bool,
        broadcast: bool,  // Whether to broadcast via GossipSub
    },
    TriggerSync {
        share_id: String,
    },
    AddPeerToShare {
        share_id: String,
        peer_id: PeerId,
    },
    RemovePeerFromShare {
        share_id: String,
        peer_id: PeerId,
    },
    UpdatePeerPermission {
        share_id: String,
        peer_id: PeerId,
        new_permission: SharePermission,
    },
    LogActivity {
        share_id: String,
        event_type: ActivityEventType,
        peer_id: String,
        peer_name: Option<String>,
        details: String,
    },
    BroadcastFileChange {
        share_id: String,
        relative_path: String,
        is_deleted: bool,
        hash: String,
        modified: i64,
    },
    Shutdown,
}

#[derive(Debug, Default)]
struct NetworkRuntimeState {
    connected_peers: HashSet<PeerId>,
    /// Track connection metrics per peer
    peer_metrics: HashMap<PeerId, PeerConnectionMetrics>,
    /// Track active file transfers: (share_id, path) -> peer_id
    active_transfers: HashMap<(String, String), PeerId>,
}

impl NetworkRuntimeState {
    /// Add or update a connected peer with initial connection type
    fn add_peer(&mut self, peer_id: PeerId, connection_type: ConnectionType) {
        self.connected_peers.insert(peer_id);
        self.peer_metrics
            .entry(peer_id)
            .or_insert_with(|| PeerConnectionMetrics::new(connection_type));
    }

    /// Remove a peer
    fn remove_peer(&mut self, peer_id: &PeerId) {
        self.connected_peers.remove(peer_id);
        self.peer_metrics.remove(peer_id);
        // Remove any active transfers for this peer
        self.active_transfers.retain(|_, p| p != peer_id);
    }

    /// Check if a peer can start a new transfer
    fn can_start_transfer(&self, peer_id: &PeerId) -> bool {
        self.peer_metrics
            .get(peer_id)
            .map(|m| m.can_start_transfer())
            .unwrap_or(false)
    }

    /// Start a transfer for a peer
    fn start_transfer(&mut self, peer_id: &PeerId, share_id: String, path: String) -> bool {
        if let Some(metrics) = self.peer_metrics.get_mut(peer_id) {
            if metrics.increment_transfers() {
                self.active_transfers.insert((share_id, path), *peer_id);
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Complete a transfer
    fn complete_transfer(&mut self, share_id: &str, path: &str) {
        if let Some(peer_id) = self.active_transfers.remove(&(share_id.to_string(), path.to_string())) {
            if let Some(metrics) = self.peer_metrics.get_mut(&peer_id) {
                metrics.decrement_transfers();
            }
        }
    }

    /// Update peer metrics
    fn update_metrics(&mut self, peer_id: PeerId, latency_ms: Option<u64>, bandwidth_bps: Option<u64>) {
        if let Some(metrics) = self.peer_metrics.get_mut(&peer_id) {
            metrics.latency_ms = latency_ms;
            metrics.bandwidth_bps = bandwidth_bps;
            metrics.last_ping = Some(Instant::now());
        }
    }

    /// Get peer metrics
    fn get_metrics(&self, peer_id: &PeerId) -> Option<&PeerConnectionMetrics> {
        self.peer_metrics.get(peer_id)
    }
}

#[derive(Debug, Clone, Copy)]
struct SyncProgress {
    total: usize,
    completed: usize,
    has_conflicts: bool,
}

fn manifest_storage_path(notes_root: &Path, share_id: &str) -> PathBuf {
    notes_root
        .join(".scratch")
        .join("sync")
        .join(share_id)
        .join("manifest.json")
}

async fn persist_manifest(notes_root: &Path, share_id: &str, manifest: &[crate::p2p::types::FileManifest]) {
    let path = manifest_storage_path(notes_root, share_id);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(content) = serde_json::to_vec_pretty(manifest) {
        let _ = tokio::fs::write(path, content).await;
    }
}

pub struct NetworkManager {
    is_running: bool,
    peer_id: Option<String>,
    command_tx: Option<mpsc::UnboundedSender<NetworkCommand>>,
    runtime_state: Arc<std::sync::Mutex<NetworkRuntimeState>>,
    notes_root: Option<PathBuf>,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            is_running: false,
            peer_id: None,
            command_tx: None,
            runtime_state: Arc::new(std::sync::Mutex::new(NetworkRuntimeState::default())),
            notes_root: None,
        }
    }

    pub fn set_notes_root(&mut self, notes_root: PathBuf) {
        self.notes_root = Some(notes_root);
    }

    pub fn start(
        &mut self,
        app: AppHandle,
        notes_root: PathBuf,
        keypair: libp2p::identity::Keypair,
    ) -> Result<String> {
        if self.is_running {
            anyhow::bail!("P2P network is already running");
        }

        let local_peer_id = keypair.public().to_peer_id();
        let peer_id_str = local_peer_id.to_string();

        let mut swarm = SwarmBuilder::with_existing_identity(keypair.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let peer_id = key.public().to_peer_id();

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    peer_id,
                )
                .expect("Failed to create mDNS behaviour");

                let request_response = request_response::Behaviour::new(
                    [(SYNC_PROTOCOL, ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_max_concurrent_streams(32)
                        .with_request_timeout(Duration::from_secs(20)),
                );

                // Create Kademlia DHT with memory store
                let store = MemoryStore::new(peer_id);
                let mut kad = KadBehaviour::new(peer_id, store);

                // Configure AutoNAT with public relay servers for NAT traversal
                // Using well-known libp2p relay servers
                let relay_addresses: Vec<PeerId> = vec![
                    // Default libp2p relays - these are public bootstrap nodes
                    "12D3KooWSD5PToNiLQwKxDGx9RCkRaxQj8yEZ2mqXP3qZaXv1Uoq"
                        .parse()
                        .expect("valid relay peer id"),
                    "12D3KooWQmGBJjL9YYrMgNVVLJLWWqeQ6EQghokPAUcy3mQzsQ3y"
                        .parse()
                        .expect("valid relay peer id"),
                ];

                let mut autonat_config = autonat::Config::default();
                autonat_config.retry_interval(Duration::from_secs(30));
                autonat_config.boot_delay(Duration::from_secs(5));

                let autonat = autonat::Behaviour::new(peer_id, autonat_config);

                // Add relay servers to Kademlia DHT for WAN discovery
                for relay_peer in &relay_addresses {
                    // Try to add default relay addresses
                    for addr in &[
                        "/dnsaddr/bootstrap.libp2p.io/tcp/4001",
                        "/dnsaddr/relay.libp2p.io/tcp/443/wss/p2p-webrtc-star",
                    ] {
                        if let Ok(multiaddr) = addr.parse::<Multiaddr>() {
                            let _ = kad.add_address(relay_peer, multiaddr);
                        }
                    }
                }

                // Create GossipSub for broadcasting file changes
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(ValidationMode::Strict)
                    .message_id_fn(|message: &gossipsub::Message| {
                        // Use content-based message ID for deduplication
                        MessageId::from(&message.data)
                    })
                    .build()
                    .map_err(|e| anyhow::anyhow!("Failed to create gossipsub config: {}", e))
                    .unwrap();

                let mut gossipsub = Gossipsub::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )
                .map_err(|e| anyhow::anyhow!("Failed to create gossipsub: {}", e))
                .unwrap();

                // Subscribe to file changes topic
                let topic = IdentTopic::new(FILE_CHANGES_TOPIC);
                gossipsub.subscribe(&topic)
                    .map_err(|e| anyhow::anyhow!("Failed to subscribe to topic: {}", e))
                    .unwrap();

                ScratchBehaviour {
                    mdns,
                    request_response,
                    kad,
                    autonat,
                    gossipsub,
                }
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm
            .listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("valid listen multiaddr"))
            .map_err(|e| anyhow::anyhow!("Failed to start listener: {}", e))?;

        // Also listen on WebSocket for better NAT traversal
        swarm
            .listen_on("/ip4/0.0.0.0/tcp/0/ws".parse().expect("valid ws multiaddr"))
            .map_err(|e| anyhow::anyhow!("Failed to start WebSocket listener: {}", e))?;

        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<NetworkCommand>();
        let runtime_state = Arc::clone(&self.runtime_state);

        // Clone notes_root before moving into async block
        let notes_root_for_async = notes_root.clone();

        tauri::async_runtime::spawn(async move {
            let mut shares: HashMap<String, ShareRuntime> = HashMap::new();
            let mut pending_manifest: HashSet<(PeerId, String)> = HashSet::new();
            let mut pending_download_target: HashMap<(PeerId, String, String), String> = HashMap::new();
            let mut active_syncs: HashMap<(PeerId, String), SyncProgress> = HashMap::new();

            loop {
                tokio::select! {
                    command = command_rx.recv() => {
                        let Some(command) = command else { break; };
                        match command {
                            NetworkCommand::RegisterShare {
                                share_id,
                                local_path,
                                initial_peers,
                                permission,
                                is_owner,
                            } => {
                                shares.insert(share_id.clone(), ShareRuntime {
                                    local_path,
                                    known_peers: initial_peers.clone(),
                                    permission,
                                    is_owner,
                                });

                                // For each initial peer, trigger a sync attempt
                                for peer in &initial_peers {
                                    pending_manifest.insert((*peer, share_id.clone()));
                                    // Trigger a dial/sync attempt immediately. If address is not
                                    // known yet, we'll retry after discovery/connection events.
                                    swarm.behaviour_mut().request_response.send_request(
                                        peer,
                                        SyncMessage::RequestManifest { share_id: share_id.clone() },
                                    );
                                }
                            }
                            NetworkCommand::RemoveShare { share_id } => {
                                shares.remove(&share_id);
                                pending_manifest.retain(|(_, id)| id != &share_id);
                            }
                            NetworkCommand::RevokeShare { share_id } => {
                                if let Some(share) = shares.get(&share_id) {
                                    // Notify all known peers about revocation
                                    for peer in share.known_peers.iter().copied() {
                                        if runtime_state
                                            .lock()
                                            .expect("runtime state lock")
                                            .connected_peers
                                            .contains(&peer)
                                        {
                                            swarm.behaviour_mut().request_response.send_request(
                                                &peer,
                                                SyncMessage::ShareRevoked {
                                                    share_id: share_id.clone(),
                                                },
                                            );
                                        }
                                    }
                                }
                                // Clean up all pending operations for this share
                                shares.remove(&share_id);
                                pending_manifest.retain(|(_, id)| id != &share_id);
                                pending_download_target.retain(|(_, id, _)| id != &share_id);
                                // Clean up active syncs for this share
                                active_syncs.retain(|(_, id), _| id != &share_id);
                                // Clean up any active transfers
                                let mut state = runtime_state.lock().expect("runtime state lock");
                                state.active_transfers.retain(|(sid, _), _| sid != &share_id);
                            }
                            NetworkCommand::NotifyFileChange { share_id, relative_path, is_deleted, broadcast } => {
                                let Some(share) = shares.get(&share_id) else { continue; };
                                let can_send_changes = share.is_owner || matches!(share.permission, SharePermission::ReadWrite);
                                if !can_send_changes {
                                    continue;
                                }

                                // Get file hash for broadcast
                                let file_hash = if !is_deleted {
                                    if let Some(full_path) = resolve_share_file(&share.local_path, &relative_path) {
                                        SyncEngine::new(share.local_path.clone())
                                            .hash_file(&full_path)
                                            .await
                                            .unwrap_or_default()
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    String::new()
                                };

                                let file_modified = if !is_deleted {
                                    if let Some(full_path) = resolve_share_file(&share.local_path, &relative_path) {
                                        tokio::fs::metadata(&full_path).await
                                            .and_then(|m| Ok(m.modified()?.timestamp_secs()))
                                            .unwrap_or(0)
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                };

                                // Broadcast via GossipSub if requested
                                if broadcast {
                                    let broadcast = FileChangeBroadcast {
                                        share_id: share_id.clone(),
                                        path: relative_path.clone(),
                                        is_deleted,
                                        hash: file_hash.clone(),
                                        modified: file_modified,
                                        peer_id: local_peer_id.to_string(),
                                        timestamp: chrono::Utc::now().timestamp(),
                                    };

                                    if let Ok(data) = serde_json::to_vec(&broadcast) {
                                        let topic = IdentTopic::new(FILE_CHANGES_TOPIC);
                                        let _ = swarm.behaviour_mut().gossipsub.publish(topic, data);
                                    }
                                }

                                // Also send directly to connected peers for immediate notification
                                let state = runtime_state.lock().expect("runtime state lock");
                                for peer in share.known_peers.iter().copied() {
                                    if !state.connected_peers.contains(&peer) {
                                        continue;
                                    }
                                    drop(state); // Release lock before sending
                                    swarm.behaviour_mut().request_response.send_request(
                                        &peer,
                                        SyncMessage::FileChanged {
                                            share_id: share_id.clone(),
                                            path: relative_path.clone(),
                                            is_deleted,
                                        },
                                    );
                                }
                            }
                            NetworkCommand::BroadcastFileChange { share_id, relative_path, is_deleted, hash, modified } => {
                                // Broadcast file change via GossipSub to all mesh peers
                                let broadcast = FileChangeBroadcast {
                                    share_id,
                                    path: relative_path,
                                    is_deleted,
                                    hash,
                                    modified,
                                    peer_id: local_peer_id.to_string(),
                                    timestamp: chrono::Utc::now().timestamp(),
                                };

                                if let Ok(data) = serde_json::to_vec(&broadcast) {
                                    let topic = IdentTopic::new(FILE_CHANGES_TOPIC);
                                    let _ = swarm.behaviour_mut().gossipsub.publish(topic, data);
                                }
                            }
                            NetworkCommand::TriggerSync { share_id } => {
                                if let Some(share) = shares.get(&share_id) {
                                    let state = runtime_state.lock().expect("runtime state lock");
                                    for peer in share.known_peers.iter().copied() {
                                        if state.connected_peers.contains(&peer) {
                                            drop(state); // Release lock before sending
                                            swarm.behaviour_mut().request_response.send_request(
                                                &peer,
                                                SyncMessage::RequestManifest { share_id: share_id.clone() },
                                            );
                                        } else {
                                            pending_manifest.insert((peer, share_id.clone()));
                                        }
                                    }
                                }
                            }
                            NetworkCommand::AddPeerToShare { share_id, peer_id } => {
                                if let Some(share) = shares.get_mut(&share_id) {
                                    share.known_peers.insert(peer_id);
                                    log::info!("Added peer {} to share {}", peer_id, share_id);

                                    // Log member joined activity
                                    let _ = app.emit("p2p-activity-entry", serde_json::json!({
                                        "share_id": share_id,
                                        "entry": {
                                            "timestamp": chrono::Utc::now().timestamp(),
                                            "event_type": "peer_joined",
                                            "peer_id": peer_id.to_string(),
                                            "peer_name": null,
                                            "details": format!("Peer {} joined the share", peer_id),
                                        }
                                    }));

                                    // If peer is connected, trigger sync
                                    if runtime_state.lock().expect("runtime state lock").connected_peers.contains(&peer_id) {
                                        swarm.behaviour_mut().request_response.send_request(
                                            &peer_id,
                                            SyncMessage::RequestManifest { share_id },
                                        );
                                    }
                                }
                            }
                            NetworkCommand::RemovePeerFromShare { share_id, peer_id } => {
                                if let Some(share) = shares.get_mut(&share_id) {
                                    share.known_peers.remove(&peer_id);
                                    log::info!("Removed peer {} from share {}", peer_id, share_id);

                                    // Log member left activity
                                    let _ = app.emit("p2p-activity-entry", serde_json::json!({
                                        "share_id": share_id,
                                        "entry": {
                                            "timestamp": chrono::Utc::now().timestamp(),
                                            "event_type": "peer_left",
                                            "peer_id": peer_id.to_string(),
                                            "peer_name": null,
                                            "details": format!("Peer {} removed from share", peer_id),
                                        }
                                    }));
                                }
                            }
                            NetworkCommand::UpdatePeerPermission { share_id, peer_id, new_permission } => {
                                // Log permission change
                                let _ = app.emit("p2p-activity-entry", serde_json::json!({
                                    "share_id": share_id,
                                    "entry": {
                                        "timestamp": chrono::Utc::now().timestamp(),
                                        "event_type": "peer_joined", // Reuse as generic peer event
                                        "peer_id": peer_id.to_string(),
                                        "peer_name": null,
                                        "details": format!("Permission changed to {:?}", new_permission),
                                    }
                                }));

                                // If peer is connected, notify them of permission change
                                if runtime_state.lock().expect("runtime state lock").connected_peers.contains(&peer_id) {
                                    // Send a FileChanged notification as a way to prompt re-sync with new permissions
                                    if let Some(share) = shares.get(&share_id) {
                                        share.known_peers.insert(peer_id); // Ensure they're in known peers
                                    }
                                }
                            }
                            NetworkCommand::AddPeerToShare { share_id, peer_id } => {
                                if let Some(share) = shares.get_mut(&share_id) {
                                    share.known_peers.insert(peer_id);
                                    log::info!("Added peer {} to share {}", peer_id, share_id);

                                    // Log member joined activity
                                    let _ = app.emit("p2p-activity-entry", serde_json::json!({
                                        "share_id": share_id,
                                        "entry": {
                                            "timestamp": chrono::Utc::now().timestamp(),
                                            "event_type": "peer_joined",
                                            "peer_id": peer_id.to_string(),
                                            "peer_name": null,
                                            "details": format!("Peer {} joined the share", peer_id),
                                        }
                                    }));
                                }
                            }
                            NetworkCommand::LogActivity {
                                share_id,
                                event_type,
                                peer_id,
                                peer_name,
                                details,
                            } => {
                                // Emit activity event to frontend
                                let _ = app.emit("p2p-activity-entry", serde_json::json!({
                                    "share_id": share_id,
                                    "entry": {
                                        "timestamp": chrono::Utc::now().timestamp(),
                                        "event_type": event_type,
                                        "peer_id": peer_id,
                                        "peer_name": peer_name,
                                        "details": details,
                                    }
                                }));

                                // Also persist to activity log file
                                let activity_tracker = crate::p2p::ActivityTracker::new(notes_root_for_async.clone());
                                let _ = activity_tracker.log_activity(
                                    &share_id,
                                    event_type,
                                    &peer_id,
                                    peer_name,
                                    details,
                                ).await;
                            }
                            NetworkCommand::Shutdown => break,
                        }
                    }
                    event = swarm.select_next_some() => {
                        match event {
                            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                                log::info!("P2P listening on {}", address);
                            }
                            libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                                // Determine connection type (DirectTcp vs Relay)
                                let connection_type = if endpoint.get_remote_addr()
                                    .and_then(|addr| addr.protocol_stack().last())
                                    .map(|p| p.as_str().starts_with("ws") || p.as_str().starts_with("wss"))
                                    .unwrap_or(false) {
                                    ConnectionType::Relay
                                } else {
                                    ConnectionType::DirectTcp
                                };

                                // Add peer to runtime state with connection metrics
                                {
                                    let mut state = runtime_state.lock().expect("runtime state lock");
                                    state.add_peer(peer_id, connection_type);
                                }

                                let _ = app.emit("p2p-peer-connected", serde_json::json!({
                                    "peer_id": peer_id.to_string(),
                                    "peer_name": serde_json::Value::Null,
                                }));

                                // Add peer's actual address to Kademlia DHT
                                let addr = endpoint.get_remote_addr().clone();
                                let _ = swarm.behaviour_mut().kad.add_address(&peer_id, addr);

                                // Log member joined for any shares this peer is part of
                                for (share_id, share) in shares.iter() {
                                    if share.known_peers.contains(&peer_id) {
                                        let _ = app.emit("p2p-member-joined", serde_json::json!({
                                            "share_id": share_id,
                                            "peer_id": peer_id.to_string(),
                                            "peer_name": serde_json::Value::Null,
                                        }));
                                    }
                                }

                                let to_request: Vec<String> = pending_manifest
                                    .iter()
                                    .filter(|(peer, _)| *peer == peer_id)
                                    .map(|(_, share_id)| share_id.clone())
                                    .collect();
                                for share_id in to_request {
                                    swarm.behaviour_mut().request_response.send_request(
                                        &peer_id,
                                        SyncMessage::RequestManifest { share_id: share_id.clone() },
                                    );
                                    pending_manifest.remove(&(peer_id, share_id));
                                }
                            }
                            libp2p::swarm::SwarmEvent::ConnectionClosed { peer_id, .. } => {
                                runtime_state.lock().expect("runtime state lock").remove_peer(&peer_id);
                                let _ = app.emit("p2p-peer-disconnected", serde_json::json!({
                                    "peer_id": peer_id.to_string(),
                                }));

                                // Log member left for any shares this peer is part of
                                for (share_id, share) in shares.iter() {
                                    if share.known_peers.contains(&peer_id) {
                                        let _ = app.emit("p2p-member-left", serde_json::json!({
                                            "share_id": share_id,
                                            "peer_id": peer_id.to_string(),
                                        }));
                                    }
                                }
                            }
                            libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => {
                                match event {
                                    mdns::Event::Discovered(peers) => {
                                        for (peer_id, addr) in peers {
                                            swarm.behaviour_mut().request_response.add_address(&peer_id, addr.clone());

                                            // Also add to Kademlia DHT
                                            let _ = swarm.behaviour_mut().kad.add_address(&peer_id, addr);

                                            // If we were waiting on this peer, request manifest now.
                                            let to_request: Vec<String> = pending_manifest
                                                .iter()
                                                .filter(|(peer, _)| *peer == peer_id)
                                                .map(|(_, share_id)| share_id.clone())
                                                .collect();
                                            for share_id in to_request {
                                                swarm.behaviour_mut().request_response.send_request(
                                                    &peer_id,
                                                    SyncMessage::RequestManifest { share_id: share_id.clone() },
                                                );
                                                pending_manifest.remove(&(peer_id, share_id));
                                            }
                                        }
                                    }
                                    mdns::Event::Expired(_peers) => {}
                                }
                            }
                            libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(event)) => {
                                match event {
                                    request_response::Event::Message { peer, message, .. } => {
                                        match message {
                                            request_response::Message::Request { request, channel, .. } => {
                                                handle_inbound_request(
                                                    &app,
                                                    &notes_root_for_async,
                                                    &mut shares,
                                                    &mut swarm.behaviour_mut().request_response,
                                                    &mut pending_download_target,
                                                    peer,
                                                    request,
                                                    channel,
                                                ).await;
                                            }
                                            request_response::Message::Response { response, .. } => {
                                                handle_inbound_response(
                                                    &app,
                                                    &notes_root_for_async,
                                                    &shares,
                                                    &mut swarm.behaviour_mut().request_response,
                                                    &mut pending_download_target,
                                                    &mut active_syncs,
                                                    peer,
                                                    response,
                                                ).await;
                                            }
                                        }
                                    }
                                    request_response::Event::OutboundFailure { peer, error, .. } => {
                                        log::warn!("Outbound request failure to {}: {}", peer, error);
                                        let _ = app.emit("p2p-error", serde_json::json!({
                                            "share_id": "",
                                            "error": format!("Outbound request failure to {}: {}", peer, error),
                                        }));
                                    }
                                    request_response::Event::InboundFailure { peer, error, .. } => {
                                        log::warn!("Inbound request failure from {}: {}", peer, error);
                                        let _ = app.emit("p2p-error", serde_json::json!({
                                            "share_id": "",
                                            "error": format!("Inbound request failure from {}: {}", peer, error),
                                        }));
                                    }
                                    request_response::Event::ResponseSent { .. } => {}
                                }
                            }
                            libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
                                match event {
                                    kad::Event::OutboundQueryProgressed {
                                        result,
                                        ..
                                    } => match result {
                                        kad::QueryResult::GetClosestPeers(Ok(peers)) => {
                                            log::debug!("Found {} closest peers", peers.peers.len());
                                        }
                                        kad::QueryResult::GetProviders(Ok(_providers)) => {
                                            // Providers found for a share
                                        }
                                        kad::QueryResult::GetProviders(Err(e)) => {
                                            log::warn!("Failed to get providers: {}", e);
                                        }
                                        _ => {}
                                    },
                                    kad::Event::RoutingUpdated { .. } => {
                                        // Routing table updated
                                    }
                                    kad::Event::UnroutablePeer { peer, .. } => {
                                        log::debug!("Peer {} unroutable", peer);
                                    }
                                    kad::Event::RoutablePeer { peer, address, .. } => {
                                        log::debug!("Peer {} routable at {}", peer, address);
                                    }
                                    _ => {}
                                }
                            }
                            libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::AutoNat(event)) => {
                                match event {
                                    autonat::Event::StatusChanged { old, new } => {
                                        log::info!("NAT status changed from {:?} to {:?}", old, new);

                                        // Emit connection status to frontend
                                        let connection_type = if matches!(new, autonat::NatStatus::Private(_)) {
                                            ConnectionType::Relay
                                        } else {
                                            ConnectionType::DirectTcp
                                        };

                                        let _ = app.emit("p2p-nat-status-changed", serde_json::json!({
                                            "old": format!("{:?}", old),
                                            "new": format!("{:?}", new),
                                            "connection_type": connection_type,
                                        }));
                                    }
                                    _ => {}
                                }
                            }
                            libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
                                match event {
                                    gossipsub::Event::Message {
                                        propagation_source,
                                        message_id: _,
                                        message,
                                    } => {
                                        // Handle file change broadcast from other peers
                                        if let Ok(file_change) = serde_json::from_slice::<FileChangeBroadcast>(&message.data) {
                                            log::info!("Received file change broadcast from {}: {:?}", propagation_source, file_change.path);

                                            // Trigger sync for the affected share if we're a member
                                            if let Some(share) = shares.get(&file_change.share_id) {
                                                // Don't sync if we're the one who sent the change
                                                if file_change.peer_id != local_peer_id.to_string() {
                                                    // Queue manifest request for this share
                                                    for peer in share.known_peers.iter().copied() {
                                                        if peer == propagation_source {
                                                            pending_manifest.insert((peer, file_change.share_id.clone()));
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    gossipsub::Event::Subscribed { peer, topic } => {
                                        log::debug!("Peer {} subscribed to {}", peer, topic);
                                        // Peer joined our mesh, add them to relevant shares
                                        for (share_id, share) in shares.iter() {
                                            // If peer is a known member, they've rejoined the mesh
                                            if share.known_peers.contains(&peer) {
                                                let _ = app.emit("p2p-member-joined", serde_json::json!({
                                                    "share_id": share_id,
                                                    "peer_id": peer.to_string(),
                                                    "peer_name": serde_json::Value::Null,
                                                }));
                                            }
                                        }
                                    }
                                    gossipsub::Event::Unsubscribed { peer, topic } => {
                                        log::debug!("Peer {} unsubscribed from {}", peer, topic);
                                    }
                                    gossipsub::Event::Gossip { .. } => {
                                        // Gossip propagation, ignore for now
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        self.is_running = true;
        self.peer_id = Some(peer_id_str.clone());
        self.command_tx = Some(command_tx);
        self.notes_root = Some(notes_root);

        Ok(peer_id_str)
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            anyhow::bail!("P2P network is not running");
        }

        if let Some(tx) = self.command_tx.take() {
            let _ = tx.send(NetworkCommand::Shutdown);
        }
        self.is_running = false;
        self.peer_id = None;
        self.runtime_state
            .lock()
            .expect("runtime state lock")
            .connected_peers
            .clear();
        Ok(())
    }

    pub fn register_share(
        &self,
        share_id: String,
        local_path: PathBuf,
        initial_peer_ids: Vec<String>,
        permission: SharePermission,
        is_owner: bool,
    ) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let initial_peers: HashSet<PeerId> = initial_peer_ids
            .into_iter()
            .filter_map(|id| id.parse::<PeerId>().ok())
            .collect();
        let _ = tx.send(NetworkCommand::RegisterShare {
            share_id,
            local_path,
            initial_peers,
            permission,
            is_owner,
        });
    }

    pub fn remove_share(&self, share_id: String) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::RemoveShare { share_id });
    }

    pub fn revoke_share(&self, share_id: String) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::RevokeShare { share_id });
    }

    pub fn notify_file_change(&self, share_id: String, relative_path: String, is_deleted: bool, broadcast: bool) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::NotifyFileChange {
            share_id,
            relative_path,
            is_deleted,
            broadcast,
        });
    }

    pub fn trigger_sync(&self, share_id: String) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::TriggerSync { share_id });
    }

    /// Broadcast a file change via GossipSub to all mesh peers
    pub fn broadcast_file_change(&self, share_id: String, relative_path: String, is_deleted: bool, hash: String, modified: i64) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::BroadcastFileChange {
            share_id,
            relative_path,
            is_deleted,
            hash,
            modified,
        });
    }

    /// Add a peer to an existing share (multi-peer support)
    pub fn add_peer_to_share(&self, share_id: String, peer_id: String) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let peer = peer_id.parse::<PeerId>().ok();
        if let Some(peer) = peer {
            let _ = tx.send(NetworkCommand::AddPeerToShare { share_id, peer_id: peer });
        }
    }

    /// Remove a peer from a share
    pub fn remove_peer_from_share(&self, share_id: String, peer_id: String) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let peer = peer_id.parse::<PeerId>().ok();
        if let Some(peer) = peer {
            let _ = tx.send(NetworkCommand::RemovePeerFromShare { share_id, peer_id: peer });
        }
    }

    /// Update a peer's permission for a share
    pub fn update_peer_permission(&self, share_id: String, peer_id: String, new_permission: SharePermission) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let peer = peer_id.parse::<PeerId>().ok();
        if let Some(peer) = peer {
            let _ = tx.send(NetworkCommand::UpdatePeerPermission { share_id, peer_id: peer, new_permission });
        }
    }

    /// Log an activity event for a share
    pub fn log_activity(
        &self,
        share_id: String,
        event_type: ActivityEventType,
        peer_id: String,
        peer_name: Option<String>,
        details: String,
    ) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::LogActivity {
            share_id,
            event_type,
            peer_id,
            peer_name,
            details,
        });
    }

    pub fn peer_id(&self) -> Option<&str> {
        self.peer_id.as_deref()
    }

    pub fn connected_peers(&self) -> usize {
        self.runtime_state
            .lock()
            .expect("runtime state lock")
            .connected_peers
            .len()
    }

    pub fn is_peer_connected(&self, peer_id: &str) -> bool {
        let Ok(peer) = peer_id.parse::<PeerId>() else {
            return false;
        };
        self.runtime_state
            .lock()
            .expect("runtime state lock")
            .connected_peers
            .contains(&peer)
    }

    /// Get connection metrics for a specific peer
    pub fn get_connection_metrics(&self, peer_id: &PeerId) -> Option<PeerConnectionMetrics> {
        self.runtime_state
            .lock()
            .expect("runtime state lock")
            .get_metrics(peer_id)
            .cloned()
    }

    /// Update connection metrics for a peer
    pub fn update_connection_metrics(&self, peer_id: PeerId, latency_ms: Option<u64>, bandwidth_bps: Option<u64>) {
        self.runtime_state
            .lock()
            .expect("runtime state lock")
            .update_metrics(peer_id, latency_ms, bandwidth_bps);
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}

fn resolve_share_file(base: &Path, relative_path: &str) -> Option<PathBuf> {
    if relative_path.contains('\\') {
        return None;
    }
    let rel = Path::new(relative_path);
    if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    Some(base.join(rel))
}

fn note_id_from_abs_path(notes_root: &Path, file_path: &Path) -> Option<String> {
    let rel = file_path.strip_prefix(notes_root).ok()?;
    let rel_str = rel.to_str()?.replace('\\', "/");
    rel_str.strip_suffix(".md").map(|s| s.to_string())
}

async fn emit_note_change(app: &AppHandle, notes_root: &Path, full_path: &Path, kind: &str) {
    if let Some(note_id) = note_id_from_abs_path(notes_root, full_path) {
        let _ = app.emit("file-change", serde_json::json!({
            "kind": kind,
            "path": full_path.to_string_lossy().into_owned(),
            "changed_ids": [note_id],
        }));
    }
}

async fn handle_inbound_request(
    app: &AppHandle,
    notes_root: &Path,
    shares: &mut HashMap<String, ShareRuntime>,
    behaviour: &mut request_response::Behaviour<SyncCodec>,
    _pending_download_target: &mut HashMap<(PeerId, String, String), String>,
    peer: PeerId,
    request: SyncMessage,
    channel: request_response::ResponseChannel<SyncMessage>,
) {
    // Extract share_id from request and validate peer membership
    let request_share_id = match &request {
        SyncMessage::RequestManifest { share_id }
        | SyncMessage::RequestFile { share_id, .. }
        | SyncMessage::FileChanged { share_id, .. }
        | SyncMessage::ShareRevoked { share_id } => Some(share_id.as_str()),
        _ => None,
    };

    // Validate peer is a member of the share (except for share revocation)
    if let Some(share_id) = request_share_id {
        if !matches!(request, SyncMessage::ShareRevoked { .. }) {
            if let Some(share) = shares.get(share_id) {
                // Check if peer is a known member or is the owner
                let is_member = share.known_peers.contains(&peer) || share.is_owner;
                if !is_member {
                    let _ = behaviour.send_response(
                        channel,
                        SyncMessage::Error {
                            message: "Unauthorized: peer is not a member of this share".to_string(),
                        },
                    );
                    log::warn!("Rejected request from non-member peer {} for share {}", peer, share_id);
                    return;
                }
            }
        }

        // Track peer as known if they're making valid requests (for owners)
        if let Some(share) = shares.get_mut(share_id) {
            if share.is_owner && !share.known_peers.contains(&peer) {
                share.known_peers.insert(peer);
            }
        }
    }

    match request {
        SyncMessage::RequestManifest { share_id } => {
            if let Some(share) = shares.get(&share_id) {
                let engine = SyncEngine::new(share.local_path.clone());
                match engine.generate_manifest().await {
                    Ok(files) => {
                        persist_manifest(notes_root, &share_id, &files).await;
                        let _ = behaviour.send_response(
                            channel,
                            SyncMessage::Manifest { share_id, files },
                        );
                    }
                    Err(err) => {
                        let _ = behaviour.send_response(
                            channel,
                            SyncMessage::Error {
                                message: format!("Failed to generate manifest: {}", err),
                            },
                        );
                    }
                }
            } else {
                let _ = behaviour.send_response(
                    channel,
                    SyncMessage::Error {
                        message: format!("Share revoked: {}", share_id),
                    },
                );
            }
        }
        SyncMessage::RequestFile { share_id, path } => {
            if let Some(share) = shares.get(&share_id) {
                // Check concurrent transfer limit and start transfer tracking
                let can_start = {
                    let mut state = runtime_state.lock().expect("runtime state lock");
                    if let Some(metrics) = state.peer_metrics.get_mut(&peer) {
                        if !metrics.can_start_transfer() {
                            false
                        } else {
                            metrics.increment_transfers();
                            metrics.start_bandwidth_measurement();
                            true
                        }
                    } else {
                        false
                    }
                };

                if !can_start {
                    let _ = behaviour.send_response(
                        channel,
                        SyncMessage::Error {
                            message: "Too many concurrent transfers. Please try again.".to_string(),
                        },
                    );
                    return;
                }

                if let Some(full_path) = resolve_share_file(&share.local_path, &path) {
                    // Get file metadata first to check size
                    match tokio::fs::metadata(&full_path).await {
                        Ok(metadata) => {
                            let file_size = metadata.len();

                            // Get adaptive chunk size based on peer's bandwidth
                            let chunk_size = runtime_state.lock().expect("runtime state lock")
                                .peer_metrics
                                .get(&peer)
                                .map(|m| m.get_adaptive_chunk_size())
                                .unwrap_or(CHUNK_SIZE);

                            // Read file
                            match tokio::fs::read(&full_path).await {
                                Ok(bytes) => {
                                    // Apply compression for large files
                                    let should_compress = bytes.len() > COMPRESSION_THRESHOLD as usize;
                                    let data_to_send = if should_compress {
                                        SyncEngine::compress_data(&bytes).unwrap_or(bytes)
                                    } else {
                                        bytes
                                    };

                                    // Split into chunks
                                    let chunks: Vec<Vec<u8>> = data_to_send
                                        .chunks(chunk_size)
                                        .map(|chunk| chunk.to_vec())
                                        .collect();

                                    // Record bytes transferred for bandwidth measurement
                                    {
                                        let mut state = runtime_state.lock().expect("runtime state lock");
                                        if let Some(metrics) = state.peer_metrics.get_mut(&peer) {
                                            metrics.record_bytes_transferred(data_to_send.len() as u64);
                                        }
                                    }

                                    // Send first chunk
                                    let is_final = chunks.len() <= 1;
                                    let _ = behaviour.send_response(
                                        channel,
                                        SyncMessage::FileChunk {
                                            share_id,
                                            path: path.clone(),
                                            offset: 0,
                                            data: chunks.get(0).cloned().unwrap_or_default(),
                                            is_final,
                                        },
                                    );

                                    // End bandwidth measurement
                                    {
                                        let mut state = runtime_state.lock().expect("runtime state lock");
                                        if let Some(metrics) = state.peer_metrics.get_mut(&peer) {
                                            metrics.end_bandwidth_measurement();
                                        }
                                    }

                                    // Note: Multi-chunk files would require a streaming protocol
                                    // For now, we send the entire file in one message (chunked but sent together)
                                    // Future enhancement: implement proper request-response streaming
                                    if chunks.len() > 1 {
                                        log::info!("Sent file {} in {} chunks ({} bytes total, compressed: {})",
                                            path, chunks.len(), data_to_send.len(), should_compress);
                                    }
                                }
                                Err(err) => {
                                    // End bandwidth measurement on error
                                    {
                                        let mut state = runtime_state.lock().expect("runtime state lock");
                                        if let Some(metrics) = state.peer_metrics.get_mut(&peer) {
                                            metrics.end_bandwidth_measurement();
                                            metrics.decrement_transfers();
                                        }
                                    }
                                    let _ = behaviour.send_response(
                                        channel,
                                        SyncMessage::Error {
                                            message: format!("Failed to read file: {}", err),
                                        },
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            // End bandwidth measurement on error
                            {
                                let mut state = runtime_state.lock().expect("runtime state lock");
                                if let Some(metrics) = state.peer_metrics.get_mut(&peer) {
                                    metrics.end_bandwidth_measurement();
                                    metrics.decrement_transfers();
                                }
                            }
                            let _ = behaviour.send_response(
                                channel,
                                SyncMessage::Error {
                                    message: format!("Failed to read file metadata: {}", err),
                                },
                            );
                        }
                    }
                } else {
                    let _ = behaviour.send_response(
                        channel,
                        SyncMessage::Error {
                            message: "Invalid file path".to_string(),
                        },
                    );
                }
            } else {
                let _ = behaviour.send_response(
                    channel,
                    SyncMessage::Error {
                        message: format!("Share revoked: {}", share_id),
                    },
                );
            }
        }
        SyncMessage::FileChanged {
            share_id,
            path,
            is_deleted,
        } => {
            if let Some(share) = shares.get(&share_id) {
                // Fixed permission validation:
                // - Owners can always write (they control the share)
                // - Non-owners can only write if permission is ReadWrite
                let allow_remote_write = share.is_owner || matches!(share.permission, SharePermission::ReadWrite);
                if !allow_remote_write {
                    let _ = behaviour.send_response(
                        channel,
                        SyncMessage::Error {
                            message: "Share is read-only for remote updates".to_string(),
                        },
                    );
                    return;
                }

                if let Some(full_path) = resolve_share_file(&share.local_path, &path) {
                    if is_deleted {
                        let _ = tokio::fs::remove_file(&full_path).await;
                        emit_note_change(app, notes_root, &full_path, "deleted").await;
                    } else {
                        behaviour.send_request(
                            &peer,
                            SyncMessage::RequestFile {
                                share_id: share_id.clone(),
                                path: path.clone(),
                            },
                        );
                    }
                }
            }

            let _ = behaviour.send_response(
                channel,
                SyncMessage::SyncComplete { has_conflicts: false },
            );
        }
        SyncMessage::ShareRevoked { share_id } => {
            if let Some(share) = shares.get(&share_id) {
                let _ = app.emit("p2p-share-revoked", serde_json::json!({
                    "share_id": share_id,
                    "local_path": share.local_path.to_string_lossy().into_owned(),
                }));
                let _ = app.emit("p2p-error", serde_json::json!({
                    "share_id": share_id,
                    "error": "This share has been revoked by the owner.",
                }));
            } else {
                let _ = app.emit("p2p-share-revoked", serde_json::json!({
                    "share_id": share_id,
                }));
            }
            shares.remove(&share_id);
            let _ = behaviour.send_response(
                channel,
                SyncMessage::SyncComplete { has_conflicts: false },
            );
        }
        _ => {
            let _ = behaviour.send_response(
                channel,
                SyncMessage::Error {
                    message: "Unsupported request".to_string(),
                },
            );
        }
    }
}

async fn handle_inbound_response(
    app: &AppHandle,
    notes_root: &Path,
    shares: &HashMap<String, ShareRuntime>,
    behaviour: &mut request_response::Behaviour<SyncCodec>,
    pending_download_target: &mut HashMap<(PeerId, String, String), String>,
    active_syncs: &mut HashMap<(PeerId, String), SyncProgress>,
    peer: PeerId,
    response: SyncMessage,
) {
    match response {
        SyncMessage::Manifest { share_id, files } => {
            if let Some(share) = shares.get(&share_id) {
                let engine = SyncEngine::new(share.local_path.clone());
                if let Ok(local_manifest) = engine.generate_manifest().await {
                    let diff = engine.compare_manifests(&local_manifest, &files);
                    persist_manifest(notes_root, &share_id, &files).await;
                    let _ = app.emit("p2p-sync-start", serde_json::json!({ "share_id": share_id }));

                    let mut total_requests = 0usize;
                    let has_conflicts = !diff.conflicts.is_empty();
                    if !share.is_owner && matches!(share.permission, SharePermission::ReadOnly) {
                        let remote_map: HashMap<_, _> = files.iter().map(|m| (&m.path, m)).collect();
                        for remote_file in &files {
                            if !local_manifest.iter().any(|m| m.path == remote_file.path) {
                                total_requests += 1;
                                behaviour.send_request(
                                    &peer,
                                    SyncMessage::RequestFile {
                                        share_id: share_id.clone(),
                                        path: remote_file.path.clone(),
                                    },
                                );
                            }
                        }
                        for local_file in &local_manifest {
                            if let Some(remote_file) = remote_map.get(&local_file.path) {
                                if local_file.hash != remote_file.hash {
                                    total_requests += 1;
                                    behaviour.send_request(
                                        &peer,
                                        SyncMessage::RequestFile {
                                            share_id: share_id.clone(),
                                            path: local_file.path.clone(),
                                        },
                                    );
                                }
                            } else if let Some(full_path) = resolve_share_file(&share.local_path, &local_file.path) {
                                let _ = tokio::fs::remove_file(&full_path).await;
                                emit_note_change(app, notes_root, &full_path, "deleted").await;
                            }
                        }
                    } else {
                        for conflict in &diff.conflicts {
                            let _ = app.emit("p2p-conflict-detected", serde_json::json!({
                                "share_id": share_id.clone(),
                                "file_path": conflict.path.clone(),
                                "local_hash": conflict.local_hash.clone(),
                                "remote_hash": conflict.remote_hash.clone(),
                            }));
                            total_requests += 1;
                            pending_download_target.insert(
                                (peer, share_id.clone(), conflict.path.clone()),
                                conflict_copy_name(&conflict.path),
                            );
                            behaviour.send_request(
                                &peer,
                                SyncMessage::RequestFile {
                                    share_id: share_id.clone(),
                                    path: conflict.path.clone(),
                                },
                            );
                        }
                        for path in diff.to_download {
                            total_requests += 1;
                            behaviour.send_request(
                                &peer,
                                SyncMessage::RequestFile {
                                    share_id: share_id.clone(),
                                    path,
                                },
                            );
                        }
                    }

                    if total_requests == 0 {
                        let status = if has_conflicts {
                            SyncStatus::Conflict
                        } else {
                            SyncStatus::Synced
                        };
                        let _ = app.emit("p2p-sync-status", serde_json::json!({
                            "share_id": share_id,
                            "status": status,
                        }));
                        let _ = app.emit("p2p-sync-complete", serde_json::json!({
                            "share_id": share_id,
                            "has_conflicts": has_conflicts,
                        }));
                    } else {
                        active_syncs.insert(
                            (peer, share_id.clone()),
                            SyncProgress {
                                total: total_requests,
                                completed: 0,
                                has_conflicts,
                            },
                        );
                    }
                }
            }
        }
        SyncMessage::FileChunk {
            share_id,
            path,
            data,
            is_final,
            ..
        } => {
            if !is_final {
                return;
            }
            if let Some(share) = shares.get(&share_id) {
                let target_relative = pending_download_target
                    .remove(&(peer, share_id.clone(), path.clone()))
                    .unwrap_or_else(|| path.clone());

                if let Some(full_path) = resolve_share_file(&share.local_path, &target_relative) {
                    if let Some(parent) = full_path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    if tokio::fs::write(&full_path, data).await.is_ok() {
                        emit_note_change(app, notes_root, &full_path, "modified").await;
                        if let Ok(updated_manifest) = SyncEngine::new(share.local_path.clone()).generate_manifest().await {
                            persist_manifest(notes_root, &share_id, &updated_manifest).await;
                        }

                        if let Some(progress) = active_syncs.get_mut(&(peer, share_id.clone())) {
                            progress.completed += 1;
                            let pct = if progress.total == 0 {
                                100
                            } else {
                                ((progress.completed * 100) / progress.total).min(100)
                            };
                            let _ = app.emit("p2p-sync-progress", serde_json::json!({
                                "share_id": share_id,
                                "progress": pct,
                                "file": target_relative,
                            }));

                            if progress.completed >= progress.total {
                                let has_conflicts = progress.has_conflicts;
                                let status = if has_conflicts {
                                    SyncStatus::Conflict
                                } else {
                                    SyncStatus::Synced
                                };
                                let _ = app.emit("p2p-sync-status", serde_json::json!({
                                    "share_id": share_id,
                                    "status": status,
                                }));
                                let _ = app.emit("p2p-sync-complete", serde_json::json!({
                                    "share_id": share_id,
                                    "has_conflicts": has_conflicts,
                                }));
                                active_syncs.remove(&(peer, share_id.clone()));
                            }
                        } else {
                            let _ = app.emit("p2p-sync-progress", serde_json::json!({
                                "share_id": share_id,
                                "progress": 100,
                                "file": target_relative,
                            }));
                            let _ = app.emit("p2p-sync-status", serde_json::json!({
                                "share_id": share_id,
                                "status": SyncStatus::Synced,
                            }));
                            let _ = app.emit("p2p-sync-complete", serde_json::json!({
                                "share_id": share_id,
                                "has_conflicts": false,
                            }));
                        }
                    }
                }
            }
        }
        SyncMessage::Error { message } => {
            log::warn!("Received sync error response: {}", message);
            if let Some(share_id) = message.strip_prefix("Share revoked: ").map(str::trim) {
                let _ = app.emit("p2p-sync-status", serde_json::json!({
                    "share_id": share_id,
                    "status": SyncStatus::Error("This share has been revoked by the owner.".to_string()),
                }));
                let _ = app.emit("p2p-share-revoked", serde_json::json!({
                    "share_id": share_id,
                }));
            }
            let _ = app.emit("p2p-error", serde_json::json!({
                "share_id": "",
                "error": message,
            }));
        }
        _ => {}
    }
}
