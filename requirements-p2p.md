# P2P Folder Sharing Feature - Requirements & Specification

> **Status:** All 3 Phases Implemented
> **Last Updated:** 2026-04-02
> **Phases:** 3 (9 weeks total)

---

## Overview

Enable users to share folders with other users across LAN and WAN via peer-to-peer technology. Users generate invite codes (with QR codes) to share folders, and changes sync automatically with end-to-end encryption.

### Key Features

- Share folders via invite codes (text + QR code)
- P2P sync using libp2p (LAN via mDNS, WAN via TCP + Relay with Kademlia DHT)
- All-to-all mesh topology (any peer can sync with any other peer)
- Automatic bidirectional sync with conflict detection
- Shared folders management in settings
- End-to-end encryption (Noise protocol)
- Last-write-wins conflict resolution with conflict copies
- Multi-peer sharing with per-member permissions

---

## Architecture

### Technology Stack

| Layer | Technology |
|-------|-----------|
| P2P Networking | libp2p (Rust) |
| Encryption | Noise Protocol (via libp2p) |
| LAN Discovery | mDNS |
| WAN Transport | TCP + Relay (via Kademlia DHT + AutoNAT) |
| QR Codes | qrcode.react (Frontend) |
| Serialization | serde/bincode (Rust) |
| Compression | zstd (Week 9) |

### Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                     Frontend (React)                            │
│  ┌──────────────┐  ┌───────────────┐  ┌─────────────────────┐  │
│  │ ShareContext │◄─┤ ShareService  │◄─┤ UI Components       │  │
│  │  (State)     │  │  (invoke)     │  │ - ShareModal        │  │
│  │              │  │               │  │ - QrCodeDisplay     │  │
│  │              │  │               │  │ - AcceptShareModal  │  │
│  │              │  │               │  │ - SharedFoldersPage │  │
│  └──────────────┘  └───────────────┘  └─────────────────────┘  │
└────────────────────────┬────────────────────────────────────────┘
                         │ Tauri IPC
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Backend (Rust)                             │
│  ┌──────────────┐  ┌───────────────┐  ┌─────────────────────┐  │
│  │ P2PState     │◄─┤ libp2p Swarm  │◄─┤ File Sync Engine    │  │
│  │ - shares     │  │ - Network     │  │ - Manifest compare  │  │
│  │ - peers      │  │ - Protocols   │  │ - Binary diff       │  │
│  │ - invites    │  │ - Encryption  │  │ - Conflict resolve  │  │
│  └──────────────┘  └───────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Core LAN Sharing (Weeks 1-3)

**Goal:** Basic P2P network with LAN discovery, invite codes, and file sync.

#### Week 1: Backend Foundation

**Tasks:**
1. Add libp2p dependencies to `Cargo.toml`
2. Create P2P module structure (`src-tauri/src/p2p/`)
3. Implement libp2p swarm setup (TCP + mDNS + noise + yamux)
4. Create P2P state management in `AppState`
5. Implement invite code generation/encoding

**Deliverables:**
- `src-tauri/src/p2p/mod.rs` - Module exports
- `src-tauri/src/p2p/network.rs` - Swarm setup and management
- `src-tauri/src/p2p/types.rs` - Core data types
- `src-tauri/src/p2p/invite.rs` - Invite code generation
- `src-tauri/src/p2p/discovery.rs` - mDNS discovery
- Modified `src-tauri/src/lib.rs` with P2P state and basic commands

**Dependencies:**
```toml
libp2p = { version = "0.54", features = [
    "mdns",              # Local network discovery
    "tcp",               # TCP transport
    "noise",             # Encryption
    "yamux",             # Stream multiplexing
    "gossipsub",         # Pub/sub messaging
    "request-response",  # Direct messaging
    "identify",          # Peer identification
    "relay",             # NAT traversal relay
    "autonat",           # Auto NAT traversal
    "ping",              # Keep-alive protocol
    "serde",             # Serialization
] }
sha2 = "0.10"
rand = "0.8"
```

**Tauri Commands (Week 1):**
```rust
#[tauri::command]
async fn p2p_start(state: State<'_, AppState>) -> Result<P2PStatus, String>

#[tauri::command]
async fn p2p_get_status(state: State<'_, AppState>) -> Result<P2PStatus, String>
```

#### Week 2: Sync Protocol & Frontend Foundation

**Tasks:**
1. Implement sync protocol (request-response)
2. Create file manifest generation
3. Implement basic sync engine (manifest comparison)
4. Create TypeScript types and services
5. Implement ShareContext provider

**Deliverables:**
- `src-tauri/src/p2p/protocol.rs` - Sync protocol definitions
- `src-tauri/src/p2p/sync.rs` - File sync engine
- `src/types/share.ts` - TypeScript types
- `src/services/share.ts` - Tauri command wrappers
- `src/context/ShareContext.tsx` - React context provider

**Tauri Commands (Week 2):**
```rust
#[tauri::command]
async fn p2p_create_share(
    folder_path: String,
    permission: SharePermission,
    state: State<'_, AppState>,
) -> Result<CreateShareResult, String>

#[tauri::command]
async fn p2p_accept_share(
    invite_code: String,
    destination_path: String,
    state: State<'_, AppState>,
) -> Result<SharedFolder, String>

#[tauri::command]
async fn p2p_list_shares(state: State<'_, AppState>) -> Result<Vec<SharedFolder>, String>
```

#### Week 3: UI Components & Integration

**Tasks:**
1. Create Share modal component
2. Implement QR code generation
3. Create Accept Share modal
4. Add "Share Folder" context menu
5. End-to-end testing for LAN sharing

**Deliverables:**
- `src/components/share/ShareModal.tsx` - Share creation modal
- `src/components/share/QrCodeDisplay.tsx` - QR code component
- `src/components/share/AcceptShareModal.tsx` - Accept share modal
- Modified `src/components/notes/FolderTreeView.tsx` - Context menu
- Modified `src/main.tsx` - Add ShareProvider
- Modified `package.json` - Add qrcode.react

**Frontend Dependency:**
```json
{
  "qrcode.react": "^4.0.0"
}
```

---

### Phase 2: Enhanced Sharing (Weeks 4-6)

**Goal:** Settings page, revoke access, sync status, conflict resolution.

#### Week 4: Settings Page & Persistence

**Tasks:**
1. Create Shared Folders settings section
2. Implement share persistence (`.scratch/shares.json`)
3. Implement sync manifest storage
4. Add list shares command

**Deliverables:**
- `src/components/share/SharedFoldersSection.tsx` - Settings page
- `src/components/share/SyncStatusIndicator.tsx` - Status indicator
- Share persistence in `{NOTES_FOLDER}/.scratch/shares.json`
- Manifest storage in `{NOTES_FOLDER}/.scratch/sync/{share_id}/manifest.json`

**Tauri Commands (Week 4):**
```rust
// Persistence is handled internally via existing commands
// Ensure shares are loaded from disk on startup
```

#### Week 5: Revoke & Real-time Sync

**Tasks:**
1. Implement revoke share functionality
2. Add real-time sync status tracking
3. Emit sync events to frontend
4. Add manual sync trigger

**Deliverables:**
- Revoke share implementation
- Real-time events: "p2p-sync-start", "p2p-sync-progress", "p2p-sync-complete"
- Manual sync button in UI

**Tauri Commands (Week 5):**
```rust
#[tauri::command]
async fn p2p_revoke_share(share_id: String, state: State<'_, AppState>) -> Result<(), String>

#[tauri::command]
async fn p2p_manual_sync(share_id: String, state: State<'_, AppState>) -> Result<(), String>
```

#### Week 6: Conflict Resolution

**Tasks:**
1. Implement conflict detection
2. Create conflict resolution dialog
3. Implement conflict resolution command
4. Add conflict copy creation

**Deliverables:**
- `src/components/share/ConflictResolutionDialog.tsx` - Conflict UI
- Conflict detection in sync engine
- Last-write-wins with conflict copies

**Tauri Commands (Week 6):**
```rust
#[tauri::command]
async fn p2p_resolve_conflict(
    share_id: String,
    file_path: String,
    resolution: ConflictResolution,
    state: State<'_, AppState>,
) -> Result<(), String>
```

**Conflict Resolution Options:**
- `KeepLocal` - Keep local version, discard remote
- `KeepRemote` - Keep remote version, discard local
- `KeepBoth` - Create conflict copy: `file (conflict copy).md`

---

### Phase 3: WAN + Multi-peer + Polish (Weeks 7-9)

**Goal:** TCP WAN connections with Kademlia DHT, all-to-all mesh sharing, performance optimizations.

#### Week 7: TCP WAN & NAT Traversal

**Tasks:**
1. Add Kademlia DHT to libp2p for WAN peer discovery
2. Implement proper AutoNAT configuration
3. Add relay client fallback for NAT traversal
4. Test WAN connections across different networks

**Deliverables:**
- Kademlia DHT integration for peer discovery
- AutoNAT properly configured with public relay servers
- TCP relay client fallback connections
- WAN peer discovery and connection

**Additional libp2p features:**
```toml
libp2p = { version = "0.54", features = [
    # ... existing features ...
    "kad",               # Kademlia DHT for WAN peer discovery
    "dcvr",              # DHT content routing for provider discovery
] }
```

**Tauri Commands (Week 7):**
```rust
#[tauri::command]
async fn p2p_discover_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>, String>

#[tauri::command]
async fn p2p_get_connection_info(share_id: String, state: State<'_, AppState>) -> Result<ConnectionInfo, String>
```

#### Week 8: Multi-peer Mesh Sharing

**Tasks:**
1. Update ShareRuntime to support multiple peers per share
2. Implement all-to-all mesh sync topology (any peer can sync with any other)
3. Add GossipSub for broadcasting file changes to all peers
4. Implement per-member permission management
5. Create activity log tracking

**Deliverables:**
- Multi-peer sharing support (all-to-all mesh topology)
- Per-member permission management (owner can add/remove/modify individual peers)
- Members list UI showing all peers with online status
- Activity log displaying sync events
- Kademlia DHT for discovering other peers in the same share

**New Types:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareMember {
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub permission: SharePermission,
    pub joined_at: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub share_id: String,
    pub connection_type: ConnectionType,  // DirectTcp, Relay
    pub latency_ms: Option<u64>,
    pub bandwidth_bps: Option<u64>,
}
```

**New Tauri Commands (Week 8):**
```rust
#[tauri::command]
async fn p2p_add_member(
    share_id: String,
    invite_code: String,
    state: State<'_, AppState>,
) -> Result<ShareMember, String>

#[tauri::command]
async fn p2p_remove_member(
    share_id: String,
    peer_id: String,
    state: State<'_, AppState>,
) -> Result<(), String>

#[tauri::command]
async fn p2p_update_member_permission(
    share_id: String,
    peer_id: String,
    permission: SharePermission,
    state: State<'_, AppState>,
) -> Result<(), String>

#[tauri::command]
async fn p2p_get_activity_log(share_id: String, state: State<'_, AppState>) -> Result<Vec<ActivityEntry>, String>
```

#### Week 9: Polish & Testing

**Tasks:**
1. Performance optimization (concurrent transfers, adaptive chunking, compression)
2. Connection fallback strategy (direct TCP → relay)
3. Comprehensive error handling and recovery
4. Network interruption recovery testing
5. User documentation and troubleshooting guide

**Deliverables:**
- Concurrent file transfers (up to 3 files per connection)
- Adaptive chunking (32KB for slow, 128KB for fast connections)
- Zstd compression for files > 1MB
- Connection pooling (reuse connections for 60s)
- Comprehensive test coverage
- User documentation

**Optimization Targets:**
- Chunked file transfer: 32-128 KB (adaptive based on bandwidth)
- Concurrent transfers: Up to 3 files per connection
- Compression: Zstd for files > 1MB
- Connection pooling: Keep idle connections alive for 60 seconds
- Sync debounce: 2 second delay after changes

---

## Data Structures

### Rust Types

```rust
use serde::{Deserialize, Serialize};

/// Share permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SharePermission {
    ReadOnly,
    ReadWrite,
}

/// Sync status for a shared folder
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Idle,
    DiscoveringPeer,
    Connecting,
    Syncing,
    Synced,
    Conflict,
    Error(String),
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
    pub sync_status: SyncStatus,
    pub last_synced: i64,
    pub created_at: i64,
    pub members: Vec<ShareMember>,  // Phase 3: Track all members
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

/// Share member information (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareMember {
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub permission: SharePermission,
    pub joined_at: i64,
    pub last_seen: i64,
}

/// Connection information (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub share_id: String,
    pub connection_type: ConnectionType,
    pub latency_ms: Option<u64>,
    pub bandwidth_bps: Option<u64>,
}

/// Connection type (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConnectionType {
    DirectTcp,
    Relay,
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

/// Activity event type (Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActivityEventType {
    PeerJoined,
    PeerLeft,
    FileSynced,
    ConflictResolved,
}

/// Conflict resolution options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    KeepLocal,
    KeepRemote,
    KeepBoth,
}

/// Sync message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SyncMessage {
    /// Request manifest from peer for a specific share
    RequestManifest { share_id: String },
    /// Respond with manifest for a specific share
    Manifest {
        share_id: String,
        files: Vec<FileManifest>,
    },
    /// Request file content from a specific share
    RequestFile { share_id: String, path: String },
    /// Respond with file content (chunked)
    FileChunk {
        share_id: String,
        path: String,
        offset: u64,
        data: Vec<u8>,
        is_final: bool,
    },
    /// Notification of file change in a share
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
```

### TypeScript Types

```typescript
// src/types/share.ts

export type SharePermission = 'read_only' | 'read_write';

export type SyncStatus =
  | 'idle'
  | 'discovering_peer'
  | 'connecting'
  | 'syncing'
  | 'synced'
  | 'conflict'
  | 'error';

export interface SharedFolder {
  id: string;
  local_path: string;
  remote_path: string;
  peer_id: string;
  peer_name: string | null;
  permission: SharePermission;
  sync_status: SyncStatus;
  last_synced: number;
  created_at: number;
  members?: ShareMember[];  // Phase 3: Track all members
  file_count?: number;
  total_size?: number;
}

export interface ShareMember {  // Phase 3
  peer_id: string;
  peer_name: string | null;
  permission: SharePermission;
  joined_at: number;
  last_seen: number;
}

export type ConnectionType = 'direct_tcp' | 'relay';  // Phase 3

export interface ConnectionInfo {  // Phase 3
  share_id: string;
  connection_type: ConnectionType;
  latency_ms: number | null;
  bandwidth_bps: number | null;
}

export type ActivityEventType = 'peer_joined' | 'peer_left' | 'file_synced' | 'conflict_resolved';  // Phase 3

export interface ActivityEntry {  // Phase 3
  timestamp: number;
  event_type: ActivityEventType;
  peer_id: string;
  peer_name: string | null;
  details: string;
}

export interface P2PStatus {
  is_running: boolean;
  peer_id: string;
  connected_peers: number;
}

export interface CreateShareOptions {
  folder_path: string;
  permission: SharePermission;
}

export interface AcceptShareOptions {
  invite_code: string;
  destination_path: string;
}

export type ConflictResolution = 'keep_local' | 'keep_remote' | 'keep_both';
```

---

## Tauri Commands API

### P2P Lifecycle

```rust
/// Start P2P networking
async fn p2p_start(state: State<'_, AppState>) -> Result<P2PStatus, String>

/// Stop P2P networking
async fn p2p_stop(state: State<'_, AppState>) -> Result<(), String>

/// Get current P2P status
async fn p2p_get_status(state: State<'_, AppState>) -> Result<P2PStatus, String>
```

### Share Management

```rust
/// Create a new share for a folder
async fn p2p_create_share(
    folder_path: String,
    permission: SharePermission,
    state: State<'_, AppState>,
) -> Result<CreateShareResult, String>

/// Accept a share invite
async fn p2p_accept_share(
    invite_code: String,
    destination_path: String,
    state: State<'_, AppState>,
) -> Result<SharedFolder, String>

/// List all shares
async fn p2p_list_shares(state: State<'_, AppState>) -> Result<Vec<SharedFolder>, String>

/// Revoke a share
async fn p2p_revoke_share(share_id: String, state: State<'_, AppState>) -> Result<(), String>

/// Get sync status for a specific share
async fn p2p_get_sync_status(share_id: String, state: State<'_, AppState>) -> Result<SyncStatus, String>

/// Manually trigger sync for a share
async fn p2p_manual_sync(share_id: String, state: State<'_, AppState>) -> Result<(), String>
```

### Conflict Resolution

```rust
/// Resolve a file conflict
async fn p2p_resolve_conflict(
    share_id: String,
    file_path: String,
    resolution: ConflictResolution,
    state: State<'_, AppState>,
) -> Result<(), String>
```

### Peer Discovery

```rust
/// Discover available peers
async fn p2p_discover_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>, String>

/// Get connection info for a share (Phase 3)
async fn p2p_get_connection_info(
    share_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionInfo, String>
```

### Multi-peer Management (Phase 3)

```rust
/// Add a member to an existing share
async fn p2p_add_member(
    share_id: String,
    invite_code: String,
    state: State<'_, AppState>,
) -> Result<ShareMember, String>

/// Remove a member from a share
async fn p2p_remove_member(
    share_id: String,
    peer_id: String,
    state: State<'_, AppState>,
) -> Result<(), String>

/// Update a member's permission
async fn p2p_update_member_permission(
    share_id: String,
    peer_id: String,
    permission: SharePermission,
    state: State<'_, AppState>,
) -> Result<(), String>

/// Get activity log for a share
async fn p2p_get_activity_log(
    share_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ActivityEntry>, String>
```

---

## Events Emitted to Frontend

| Event Name | Payload | Description |
|-----------|---------|-------------|
| `p2p-sync-start` | `{ share_id: string }` | Sync started |
| `p2p-sync-progress` | `{ share_id: string, progress: number, file: string }` | Sync progress update |
| `p2p-sync-complete` | `{ share_id: string, has_conflicts: boolean }` | Sync completed |
| `p2p-peer-connected` | `{ peer_id: string, peer_name: string }` | Peer connected |
| `p2p-peer-disconnected` | `{ peer_id: string }` | Peer disconnected |
| `p2p-conflict-detected` | `{ share_id: string, file_path: string, local_hash: string, remote_hash: string }` | Conflict detected |
| `p2p-error` | `{ share_id: string, error: string }` | Error occurred |
| `p2p-member-joined` | `{ share_id: string, peer_id: string, peer_name: string }` | Member joined share (Phase 3) |
| `p2p-member-left` | `{ share_id: string, peer_id: string }` | Member left share (Phase 3) |
| `p2p-connection-changed` | `{ share_id: string, connection_type: string, latency_ms: number }` | Connection type changed (Phase 3) |

---

## File Structure

### New Rust Files

```
src-tauri/src/p2p/
├── mod.rs           # Module exports
├── types.rs         # Shared types
├── network.rs       # libp2p swarm setup
├── protocol.rs      # Sync protocol
├── sync.rs          # File sync engine
├── invite.rs        # Invite code generation
├── discovery.rs     # Peer discovery
└── activity.rs      # Activity log tracking (Phase 3)
```

### New TypeScript Files

```
src/
├── types/
│   └── share.ts                     # Share types
├── services/
│   └── share.ts                     # Share services
├── context/
│   └── ShareContext.tsx             # Share context
└── components/share/
    ├── ShareModal.tsx               # Share creation modal
    ├── QrCodeDisplay.tsx            # QR code component
    ├── AcceptShareModal.tsx         # Accept share modal
    ├── SharedFoldersSection.tsx     # Settings page
    ├── SyncStatusIndicator.tsx      # Status indicator
    ├── ConflictResolutionDialog.tsx # Conflict resolution
    ├── ShareMembersList.tsx         # Members list (Phase 3)
    └── ActivityLog.tsx              # Activity log (Phase 3)
```

### Modified Files

```
src-tauri/
├── Cargo.toml           # Add dependencies
├── src/
│   ├── main.rs          # Add mod p2p
│   └── lib.rs           # Add P2P state, commands

src/
├── package.json         # Add qrcode.react
├── main.tsx             # Add ShareProvider
├── components/
│   ├── notes/
│   │   └── FolderTreeView.tsx       # Add context menu
│   ├── settings/
│   │   └── SettingsPage.tsx         # Add tab
│   └── icons/
│       └── index.tsx                # Add icons
```

---

## Storage

### Shares Configuration

**Location:** `{NOTES_FOLDER}/.scratch/shares.json`

```json
{
  "shares": [
    {
      "id": "share-abc123",
      "local_path": "Recipes",
      "remote_path": "Recipes",
      "peer_id": "12D3KooW...",
      "peer_name": "Alice",
      "permission": "read_write",
      "created_at": 1234567890,
      "encryption_key": "base64-encoded-key"
    }
  ],
  "peer_identity": {
    "peer_id": "12D3KooW...",
    "private_key": "encrypted-key-pair"
  }
}
```

### Sync Manifest

**Location:** `{NOTES_FOLDER}/.scratch/sync/{share_id}/manifest.json`

```json
{
  "version": 1,
  "files": [
    {
      "path": "Recipes/pasta.md",
      "hash": "abc123...",
      "size": 2048,
      "modified": 1234567890,
      "is_deleted": false
    }
  ],
  "last_sync": 1234567890
}
```

### Activity Log

**Location:** `{NOTES_FOLDER}/.scratch/sync/{share_id}/activity.json` (Phase 3)

```json
{
  "entries": [
    {
      "timestamp": 1234567890,
      "event_type": "peer_joined",
      "peer_id": "12D3KooW...",
      "peer_name": "Alice",
      "details": "Joined share"
    },
    {
      "timestamp": 1234567895,
      "event_type": "file_synced",
      "peer_id": "12D3KooW...",
      "peer_name": "Alice",
      "details": "Synced 3 files"
    }
  ]
}
```

---

## Security

### Encryption
- All traffic encrypted via libp2p's Noise protocol
- Invite codes contain encrypted encryption keys
- Peer authentication via libp2p peer IDs (ed25519)

### Path Validation
- Chroot shared folders to prevent path traversal
- Validate all paths are within shared folder boundaries
- Reject paths containing `..` or absolute paths

### Access Control
- Permission levels (read_only, read_write)
- Revoke access by removing peer from allowlist
- Peer can leave share at any time

---

## Testing Steps

### Manual Testing Checklist

#### Phase 1 Testing

1. **Start P2P**
   - [ ] Launch Scratch, verify P2P starts automatically
   - [ ] Check peer ID is generated and displayed in settings

2. **Create Share (LAN)**
   - [ ] Right-click folder → "Share Folder..."
   - [ ] Select permission (read_write)
   - [ ] Click "Share" → Invite code appears
   - [ ] Copy invite code to clipboard
   - [ ] Display QR code

3. **Accept Share (LAN)**
   - [ ] On second device, click "Enter Invite Code"
   - [ ] Paste invite code
   - [ ] Select destination folder
   - [ ] Click "Accept"
   - [ ] Verify folder appears with files

4. **Verify Sync**
   - [ ] Create new file in shared folder on Device A
   - [ ] Verify file appears on Device B
   - [ ] Modify file on Device A
   - [ ] Verify changes appear on Device B
   - [ ] Delete file on Device A
   - [ ] Verify file deleted on Device B

#### Phase 2 Testing

5. **Shared Folders Settings**
   - [ ] Open Settings → Shared Folders
   - [ ] Verify shared folder appears in list
   - [ ] Verify sync status indicator shows correct state
   - [ ] Verify "Last synced" timestamp updates

6. **Revoke Share**
   - [ ] Click "Revoke" on shared folder
   - [ ] Confirm revoke
   - [ ] Verify folder removed from other device
   - [ ] Verify other device loses access

7. **Conflict Resolution**
   - [ ] Edit same file on both devices simultaneously
   - [ ] Verify conflict detected
   - [ ] Conflict dialog appears
   - [ ] Test "Keep Local" resolution
   - [ ] Test "Keep Remote" resolution
   - [ ] Test "Keep Both" resolution
   - [ ] Verify conflict copy created

#### Phase 3 Testing

8. **WAN Connection**
   - [ ] Test on different networks (home, office, mobile)
   - [ ] Verify Kademlia DHT peer discovery works
   - [ ] Verify AutoNAT establishes direct connections when possible
   - [ ] Verify relay fallback works when NAT traversal fails
   - [ ] Test sync over WAN with different connection types

9. **Multi-peer Mesh Sharing**
   - [ ] Share folder with 3+ people
   - [ ] Verify all peers can discover each other via DHT
   - [ ] Verify all peers receive updates from any peer
   - [ ] Test concurrent edits from multiple peers simultaneously
   - [ ] Verify per-member permission changes work
   - [ ] Test removing individual members from share

10. **Performance Tests**
   - [ ] Large file sync (>10MB) with adaptive chunking
   - [ ] Many files sync (100+ files) with concurrent transfers
   - [ ] Verify compression works for files > 1MB
   - [ ] Test connection pooling and reuse
   - [ ] Verify bandwidth-based chunk size adaptation

### Edge Cases

- [ ] Network interruption during sync
- [ ] Large file sync (>10MB)
- [ ] Many files sync (100+ files)
- [ ] Invalid invite code
- [ ] Expired invite
- [ ] Revoke while syncing
- [ ] Offline both devices, then online
- [ ] Rename/move shared folder
- [ ] Delete shared folder

---

## Performance Targets

| Metric | Target |
|--------|--------|
| File chunk size | 32-128 KB (adaptive based on bandwidth) |
| Concurrent transfers | Up to 3 files per connection |
| Sync debounce delay | 2 seconds |
| Connection pool timeout | 60 seconds |
| Compression threshold | 1 MB (zstd) |
| Max file size | 100 MB |
| Max folder size | 5 GB |
| Max concurrent shares | 10 |
| Max peers per share | 20 |
| Transfer speed limit | 10 MB/s |

---

## Invite Code Format

**Structure:** `scratch-share-` + base64(encrypted_payload) + signature

**Payload (before encryption):**
```json
{
  "version": 1,
  "sender_peer_id": "12D3KooW...",
  "folder_id": "recipes-abc123",
  "folder_name": "Recipes",
  "permissions": "read_write",
  "encryption_key": "base64-key...",
  "timestamp": 1234567890,
  "relay_hint": "relay.example.com:1337"
}
```

**Encoding:**
1. Serialize payload to JSON
2. Encrypt with Noise protocol
3. Base64 encode
4. Add signature
5. Prefix with `scratch-share-`

---

## UI Mockups

### Share Modal

```
┌──────────────────────────────────────────┐
│   Share Folder                           │
├──────────────────────────────────────────┤
│  📁 Recipes                              │
│                                          │
│  Permission:                             │
│  ● Can edit (recommended)                │
│  ○ View only                             │
│                                          │
│  ─────────────────────────────────────  │
│                                          │
│  Invite Code:                            │
│  ┌────────────────────────────────────┐ │
│  │ scratch-share-abc123...             │ │
│  └────────────────────────────────────┘ │
│                                          │
│  [📋 Copy]  [📱 Show QR Code]           │
│                                          │
│  Share this code with the person you     │
│  want to share this folder with.         │
│                                          │
│  [Cancel]                    [Share]     │
└──────────────────────────────────────────┘
```

### Accept Share Modal

```
┌──────────────────────────────────────────┐
│   Accept Shared Folder                   │
├──────────────────────────────────────────┤
│  📁 Recipes (from Alice)                 │
│                                          │
│  Permission: Can edit                    │
│  Files: 24 notes                         │
│  Size: ~256 KB                           │
│                                          │
│  Save to folder:                         │
│  ┌────────────────────────────────────┐ │
│  │ Shared/Recipes                     │ │
│  └────────────────────────────────────┘ │
│                                          │
│  [Browse...]                             │
│                                          │
│  This folder will sync automatically.    │
│                                          │
│  [Cancel]                    [Accept]    │
└──────────────────────────────────────────┘
```

### Shared Folders Settings

```
Settings → Shared Folders
┌──────────────────────────────────────────┐
│   Shared Folders                         │
├──────────────────────────────────────────┤
│                                          │
│  Folders you're sharing:                 │
│  ┌────────────────────────────────────┐ │
│  │ 📁 Recipes             Share       │ │
│  │    Shared with: Bob                │ │
│  │    ● Synced 2m ago                 │ │
│  │    [Revoke]  [New Invite]          │ │
│  └────────────────────────────────────┘ │
│                                          │
│  Folders shared with you:                │
│  ┌────────────────────────────────────┐ │
│  │ 📁 Work Notes          Alice       │ │
│  │    ● Synced 5m ago                 │ │
│  │    [Leave]                         │ │
│  └────────────────────────────────────┘ │
│                                          │
│  ─────────────────────────────────────  │
│                                          │
│  [Enter Invite Code]                     │
└──────────────────────────────────────────┘
```

---

## Error Messages

| Error | Message |
|-------|---------|
| Network unreachable | "Could not connect to peer. Make sure they're online and on the same network." |
| Invalid invite | "This invite code is invalid or expired." |
| Permission denied | "You don't have permission to edit this folder." |
| Conflict detected | "Conflict detected: '{file}' was modified on both devices." |
| Path traversal | "Invalid path: cannot access files outside shared folder." |
| File too large | "File is too large to sync (max 100 MB)." |
| Share revoked | "This share has been revoked by the owner." |
| Peer offline | "Peer is offline. Changes will sync when they come online." |

---

## Dependencies Summary

### Cargo.toml

```toml
[dependencies]
libp2p = { version = "0.54", features = [
    "mdns", "gossipsub", "identify", "ping",
    "request-response", "yamux", "noise",
    "tcp", "relay", "autonat",
    "kad",      # Phase 3: Kademlia DHT for WAN peer discovery
    "dcvr",     # Phase 3: DHT content routing
    "serde",
] }
sha2 = "0.10"
rand = "0.8"
zstd = "0.13"  # Phase 3: Compression for large files
```

### package.json

```json
{
  "dependencies": {
    "qrcode.react": "^4.0.0"
  }
}
```

---

## Implementation Checklist

### Phase 1: Core LAN Sharing
- [ ] Add libp2p dependencies
- [ ] Create p2p module structure
- [ ] Implement network.rs (swarm setup)
- [ ] Implement types.rs (data types)
- [ ] Implement invite.rs (invite codes)
- [ ] Implement discovery.rs (mDNS)
- [ ] Implement protocol.rs (sync protocol)
- [ ] Implement sync.rs (sync engine)
- [ ] Add P2P state to AppState
- [ ] Create share.ts types
- [ ] Create share.ts services
- [ ] Create ShareContext
- [ ] Create ShareModal
- [ ] Create QrCodeDisplay
- [ ] Create AcceptShareModal
- [ ] Add context menu item
- [ ] Test LAN sharing

### Phase 2: Enhanced Sharing
- [ ] Create SharedFoldersSection
- [ ] Create SyncStatusIndicator
- [ ] Implement share persistence
- [ ] Implement manifest storage
- [ ] Implement revoke share
- [ ] Add sync events
- [ ] Implement manual sync
- [ ] Implement conflict detection
- [ ] Create ConflictResolutionDialog
- [ ] Test all features

### Phase 3: WAN + Multi-peer + Polish
- [ ] Add Kademlia DHT for WAN peer discovery
- [ ] Configure AutoNAT with public relay servers
- [ ] Implement TCP relay client fallback
- [ ] Update ShareRuntime for multi-peer support
- [ ] Implement all-to-all mesh sync topology
- [ ] Add GossipSub for broadcasting changes
- [ ] Implement per-member permission management
- [ ] Create ShareMembersList component
- [ ] Create ActivityLog component
- [ ] Implement concurrent file transfers (3 per connection)
- [ ] Add adaptive chunking (32-128 KB based on bandwidth)
- [ ] Add Zstd compression for files > 1MB
- [ ] Implement connection pooling (60s keep-alive)
- [ ] Comprehensive testing (WAN, multi-peer, interruptions)
- [ ] User documentation and troubleshooting guide

---

**Document Status:** Ready for Implementation
