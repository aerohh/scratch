// P2P sharing types

use serde::{Deserialize, Serialize};

/// Share permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SharePermission {
    ReadOnly,
    ReadWrite,
}

impl std::fmt::Display for SharePermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SharePermission::ReadOnly => write!(f, "read_only"),
            SharePermission::ReadWrite => write!(f, "read_write"),
        }
    }
}

/// Sync status for a shared folder
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Idle,
    #[serde(alias = "discoveringpeer")]
    DiscoveringPeer,
    #[serde(alias = "connecting")]
    Connecting,
    #[serde(alias = "syncing")]
    Syncing,
    #[serde(alias = "synced")]
    Synced,
    #[serde(alias = "conflict")]
    Conflict,
    Error(String),
}

impl SyncStatus {
    pub fn is_error(&self) -> bool {
        matches!(self, SyncStatus::Error(_))
    }

    pub fn is_synced(&self) -> bool {
        matches!(self, SyncStatus::Synced)
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            SyncStatus::DiscoveringPeer | SyncStatus::Connecting | SyncStatus::Syncing
        )
    }
}

/// Information about a folder share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFolder {
    pub id: String,
    pub local_path: String,
    pub remote_path: String,
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub permission: SharePermission,
    #[serde(default)]
    pub is_owner: bool,
    pub sync_status: SyncStatus,
    pub last_synced: i64,
    pub created_at: i64,
    #[serde(default)]
    pub members: Vec<ShareMember>,
}

/// Share member information (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareMember {
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub permission: SharePermission,
    pub joined_at: i64,
    pub last_seen: i64,
}

/// Connection type (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionType {
    DirectTcp,
    Relay,
}

/// Connection information (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub share_id: String,
    pub connection_type: ConnectionType,
    pub latency_ms: Option<u64>,
    pub bandwidth_bps: Option<u64>,
}

/// Activity event type (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityEventType {
    PeerJoined,
    PeerLeft,
    FileSynced,
    ConflictResolved,
}

/// Activity log entry (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub timestamp: i64,
    pub event_type: ActivityEventType,
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub details: String,
}

/// Invite code payload (encrypted before encoding)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitePayload {
    pub version: u8,
    pub sender_peer_id: String,
    pub folder_id: String,
    pub folder_name: String,
    pub permissions: SharePermission,
    pub encryption_key: String,
    pub timestamp: i64,
    pub relay_hint: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
}

/// P2P network status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PStatus {
    pub is_running: bool,
    pub peer_id: String,
    pub connected_peers: usize,
}

/// File manifest for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifest {
    pub path: String,
    pub hash: String,        // SHA-256
    pub size: u64,
    pub modified: i64,
    pub is_deleted: bool,
}

/// Share result from creating a share
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShareResult {
    pub invite_code: String,
    pub share_id: String,
}

/// Conflict resolution options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    KeepLocal,
    KeepRemote,
    KeepBoth,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub addresses: Vec<String>,
}

/// Sync message types for custom protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SyncMessage {
    /// Request manifest from peer
    RequestManifest { share_id: String },
    /// Respond with manifest
    Manifest {
        share_id: String,
        files: Vec<FileManifest>,
    },
    /// Request file content
    RequestFile { share_id: String, path: String },
    /// Respond with file content (chunked)
    FileChunk {
        share_id: String,
        path: String,
        offset: u64,
        data: Vec<u8>,
        is_final: bool,
    },
    /// Notification of file change
    FileChanged {
        share_id: String,
        path: String,
        is_deleted: bool,
    },
    /// Share was revoked by owner
    ShareRevoked { share_id: String },
    /// Sync complete
    SyncComplete { has_conflicts: bool },
    /// Error
    Error { message: String },
}

/// P2P state managed by the app
#[derive(Debug, Clone, Default)]
pub struct P2PState {
    pub is_running: bool,
    pub peer_id: Option<String>,
    pub shares: Vec<SharedFolder>,
}

impl P2PState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_running(&mut self, running: bool) {
        self.is_running = running;
    }

    pub fn set_peer_id(&mut self, peer_id: String) {
        self.peer_id = Some(peer_id);
    }

    pub fn add_share(&mut self, share: SharedFolder) {
        self.shares.push(share);
    }

    pub fn remove_share(&mut self, share_id: &str) -> bool {
        if let Some(pos) = self.shares.iter().position(|s| s.id == share_id) {
            self.shares.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn get_share(&self, share_id: &str) -> Option<&SharedFolder> {
        self.shares.iter().find(|s| s.id == share_id)
    }

    pub fn get_share_mut(&mut self, share_id: &str) -> Option<&mut SharedFolder> {
        self.shares.iter_mut().find(|s| s.id == share_id)
    }

    pub fn update_share_status(&mut self, share_id: &str, status: SyncStatus) {
        if let Some(share) = self.get_share_mut(share_id) {
            share.sync_status = status;
        }
    }
}

/// Invite code prefix for identification
pub const INVITE_PREFIX: &str = "scratch-share-";

/// Current invite payload version
pub const INVITE_VERSION: u8 = 1;

/// Chunk size for file transfer
pub const CHUNK_SIZE: usize = 64 * 1024; // 64 KB

/// Max file size for sync (100 MB)
pub const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// Default sync debounce delay (2 seconds)
pub const SYNC_DEBOUNCE_MS: u64 = 2000;

/// Max concurrent transfers (Phase 3)
pub const MAX_CONCURRENT_TRANSFERS: usize = 3;

/// Connection pool timeout in seconds (Phase 3)
pub const CONNECTION_POOL_TIMEOUT_SECS: u64 = 60;

/// Compression threshold in bytes (Phase 3)
pub const COMPRESSION_THRESHOLD: u64 = 1024 * 1024; // 1 MB
