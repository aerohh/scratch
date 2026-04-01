use crate::p2p::protocol::{SyncCodec, SYNC_PROTOCOL};
use crate::p2p::sync::SyncEngine;
use crate::p2p::types::{SyncMessage, SyncStatus};
use anyhow::Result;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{futures::StreamExt, mdns, noise, tcp, yamux, PeerId, SwarmBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "BehaviourEvent", prelude = "libp2p::swarm::derive_prelude")]
struct ScratchBehaviour {
    mdns: mdns::tokio::Behaviour,
    request_response: request_response::Behaviour<SyncCodec>,
}

#[derive(Debug)]
enum BehaviourEvent {
    Mdns(mdns::Event),
    RequestResponse(request_response::Event<SyncMessage, SyncMessage>),
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

#[derive(Debug, Clone)]
struct ShareRuntime {
    local_path: PathBuf,
    remote_peer: Option<PeerId>,
}

#[derive(Debug)]
enum NetworkCommand {
    RegisterShare {
        share_id: String,
        local_path: PathBuf,
        remote_peer: Option<PeerId>,
    },
    RemoveShare {
        share_id: String,
    },
    NotifyFileChange {
        share_id: String,
        relative_path: String,
        is_deleted: bool,
    },
    Shutdown,
}

#[derive(Debug, Default)]
struct NetworkRuntimeState {
    connected_peers: HashSet<PeerId>,
}

pub struct NetworkManager {
    is_running: bool,
    peer_id: Option<String>,
    command_tx: Option<mpsc::UnboundedSender<NetworkCommand>>,
    runtime_state: Arc<std::sync::Mutex<NetworkRuntimeState>>,
}

impl NetworkManager {
    pub fn new() -> Self {
        Self {
            is_running: false,
            peer_id: None,
            command_tx: None,
            runtime_state: Arc::new(std::sync::Mutex::new(NetworkRuntimeState::default())),
        }
    }

    pub fn start(&mut self, app: AppHandle, notes_root: PathBuf) -> Result<String> {
        if self.is_running {
            anyhow::bail!("P2P network is already running");
        }

        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = keypair.public().to_peer_id();
        let peer_id_str = local_peer_id.to_string();

        let mut swarm = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )
                .expect("Failed to create mDNS behaviour");
                let request_response = request_response::Behaviour::new(
                    [(SYNC_PROTOCOL, ProtocolSupport::Full)],
                    request_response::Config::default()
                        .with_max_concurrent_streams(32)
                        .with_request_timeout(Duration::from_secs(20)),
                );

                ScratchBehaviour {
                    mdns,
                    request_response,
                }
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm
            .listen_on("/ip4/0.0.0.0/tcp/0".parse().expect("valid listen multiaddr"))
            .map_err(|e| anyhow::anyhow!("Failed to start listener: {}", e))?;

        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<NetworkCommand>();
        let runtime_state = Arc::clone(&self.runtime_state);

        tauri::async_runtime::spawn(async move {
            let mut shares: HashMap<String, ShareRuntime> = HashMap::new();
            let mut pending_manifest: HashSet<(PeerId, String)> = HashSet::new();
            let mut pending_file_chunks: HashMap<(PeerId, String, String), Vec<Vec<u8>>> = HashMap::new();

            loop {
                tokio::select! {
                    command = command_rx.recv() => {
                        let Some(command) = command else { break; };
                        match command {
                            NetworkCommand::RegisterShare { share_id, local_path, remote_peer } => {
                                shares.insert(share_id.clone(), ShareRuntime {
                                    local_path,
                                    remote_peer,
                                });

                                if let Some(peer) = remote_peer {
                                    if runtime_state.lock().expect("runtime state lock").connected_peers.contains(&peer) {
                                        swarm.behaviour_mut().request_response.send_request(
                                            &peer,
                                            SyncMessage::RequestManifest { share_id },
                                        );
                                    } else {
                                        pending_manifest.insert((peer, share_id));
                                    }
                                }
                            }
                            NetworkCommand::RemoveShare { share_id } => {
                                shares.remove(&share_id);
                                pending_manifest.retain(|(_, id)| id != &share_id);
                            }
                            NetworkCommand::NotifyFileChange { share_id, relative_path, is_deleted } => {
                                for peer in runtime_state
                                    .lock()
                                    .expect("runtime state lock")
                                    .connected_peers
                                    .iter()
                                    .copied()
                                {
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
                            NetworkCommand::Shutdown => break,
                        }
                    }
                    event = swarm.select_next_some() => {
                        match event {
                            libp2p::swarm::SwarmEvent::NewListenAddr { address, .. } => {
                                log::info!("P2P listening on {}", address);
                            }
                            libp2p::swarm::SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                runtime_state.lock().expect("runtime state lock").connected_peers.insert(peer_id);
                                let _ = app.emit("p2p-peer-connected", serde_json::json!({
                                    "peer_id": peer_id.to_string(),
                                    "peer_name": serde_json::Value::Null,
                                }));

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
                                runtime_state.lock().expect("runtime state lock").connected_peers.remove(&peer_id);
                                let _ = app.emit("p2p-peer-disconnected", serde_json::json!({
                                    "peer_id": peer_id.to_string(),
                                }));
                            }
                            libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => {
                                match event {
                                    mdns::Event::Discovered(peers) => {
                                        for (peer_id, addr) in peers {
                                            swarm.behaviour_mut().request_response.add_address(&peer_id, addr);
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
                                                    &notes_root,
                                                    &shares,
                                                    &mut swarm.behaviour_mut().request_response,
                                                    &mut pending_file_chunks,
                                                    peer,
                                                    request,
                                                    channel,
                                                ).await;
                                            }
                                            request_response::Message::Response { response, .. } => {
                                                handle_inbound_response(
                                                    &app,
                                                    &notes_root,
                                                    &shares,
                                                    &mut swarm.behaviour_mut().request_response,
                                                    &mut pending_file_chunks,
                                                    peer,
                                                    response,
                                                ).await;
                                            }
                                        }
                                    }
                                    request_response::Event::OutboundFailure { peer, error, .. } => {
                                        log::warn!("Outbound request failure to {}: {}", peer, error);
                                    }
                                    request_response::Event::InboundFailure { peer, error, .. } => {
                                        log::warn!("Inbound request failure from {}: {}", peer, error);
                                    }
                                    request_response::Event::ResponseSent { .. } => {}
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

    pub fn register_share(&self, share_id: String, local_path: PathBuf, remote_peer_id: Option<String>) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let remote_peer = remote_peer_id
            .and_then(|id| id.parse::<PeerId>().ok());
        let _ = tx.send(NetworkCommand::RegisterShare {
            share_id,
            local_path,
            remote_peer,
        });
    }

    pub fn remove_share(&self, share_id: String) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::RemoveShare { share_id });
    }

    pub fn notify_file_change(&self, share_id: String, relative_path: String, is_deleted: bool) {
        let Some(tx) = self.command_tx.as_ref() else { return; };
        let _ = tx.send(NetworkCommand::NotifyFileChange {
            share_id,
            relative_path,
            is_deleted,
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
    shares: &HashMap<String, ShareRuntime>,
    behaviour: &mut request_response::Behaviour<SyncCodec>,
    _pending_file_chunks: &mut HashMap<(PeerId, String, String), Vec<Vec<u8>>>,
    peer: PeerId,
    request: SyncMessage,
    channel: request_response::ResponseChannel<SyncMessage>,
) {
    match request {
        SyncMessage::RequestManifest { share_id } => {
            if let Some(share) = shares.get(&share_id) {
                let engine = SyncEngine::new(share.local_path.clone());
                match engine.generate_manifest().await {
                    Ok(files) => {
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
                        message: "Unknown share".to_string(),
                    },
                );
            }
        }
        SyncMessage::RequestFile { share_id, path } => {
            if let Some(share) = shares.get(&share_id) {
                if let Some(full_path) = resolve_share_file(&share.local_path, &path) {
                    match tokio::fs::read(&full_path).await {
                        Ok(bytes) => {
                            let _ = behaviour.send_response(
                                channel,
                                SyncMessage::FileChunk {
                                    share_id,
                                    path,
                                    offset: 0,
                                    data: bytes,
                                    is_final: true,
                                },
                            );
                        }
                        Err(err) => {
                            let _ = behaviour.send_response(
                                channel,
                                SyncMessage::Error {
                                    message: format!("Failed to read file: {}", err),
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
                        message: "Unknown share".to_string(),
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
    _pending_file_chunks: &mut HashMap<(PeerId, String, String), Vec<Vec<u8>>>,
    peer: PeerId,
    response: SyncMessage,
) {
    match response {
        SyncMessage::Manifest { share_id, files } => {
            if let Some(share) = shares.get(&share_id) {
                let engine = SyncEngine::new(share.local_path.clone());
                if let Ok(local_manifest) = engine.generate_manifest().await {
                    let diff = engine.compare_manifests(&local_manifest, &files);
                    for path in diff.to_download {
                        behaviour.send_request(
                            &peer,
                            SyncMessage::RequestFile {
                                share_id: share_id.clone(),
                                path,
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
                if let Some(full_path) = resolve_share_file(&share.local_path, &path) {
                    if let Some(parent) = full_path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    if tokio::fs::write(&full_path, data).await.is_ok() {
                        emit_note_change(app, notes_root, &full_path, "modified").await;
                        let _ = app.emit("p2p-sync-status", serde_json::json!({
                            "share_id": share_id,
                            "status": SyncStatus::Synced,
                        }));
                    }
                }
            }
        }
        SyncMessage::Error { message } => {
            log::warn!("Received sync error response: {}", message);
        }
        _ => {}
    }
}
