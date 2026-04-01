# P2P Folder Sharing Feature - Requirements & Specification

> **Status:** Implementation Planned
> **Last Updated:** 2026-04-01
> **Phases:** 3 (9 weeks total)

---

## Overview

Enable users to share folders with other users across LAN and WAN via peer-to-peer technology. Users generate invite codes (with QR codes) to share folders, and changes sync automatically with end-to-end encryption.

### Key Features

- Share folders via invite codes (text + QR code)
- P2P sync using libp2p (LAN via mDNS, WAN via WebRTC)
- Automatic bidirectional sync with conflict detection
- Shared folders management in settings
- End-to-end encryption (Noise protocol)
- Last-write-wins conflict resolution with conflict copies

---

## Architecture

### Technology Stack

| Layer | Technology |
|-------|-----------|
| P2P Networking | libp2p (Rust) |
| Encryption | Noise Protocol (via libp2p) |
| LAN Discovery | mDNS |
| WAN Transport | WebRTC + Relay |
| QR Codes | qrcode.react (Frontend) |
| Serialization | serde/bincode (Rust) |

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

### Phase 3: WAN + Polish (Weeks 7-9)

**Goal:** WebRTC transport, relay server, multi-peer sharing, polish.

#### Week 7: WebRTC & NAT Traversal

**Tasks:**
1. Add WebRTC transport to libp2p
2. Implement relay client support
3. Add Auto NAT traversal
4. Test WAN connections

**Deliverables:**
- WebRTC transport integration
- Relay server configuration
- Auto NAT traversal
- WAN peer discovery

**Additional libp2p features:**
```toml
libp2p = { version = "0.54", features = [
    # ... existing features ...
    "webrtc",            # WebRTC transport
    "kad",               # Kademlia DHT
] }
```

**Tauri Commands (Week 7):**
```rust
#[tauri::command]
async fn p2p_discover_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>, String>
```

#### Week 8: Multi-peer Sharing

**Tasks:**
1. Support sharing folder with multiple people
2. Implement Kademlia DHT for peer discovery
3. Add connection fallback strategies
4. Multi-peer sync coordination

**Deliverables:**
- Multi-peer sharing support
- Kademlia DHT integration
- Connection fallback (WebRTC → Relay)
- Activity log UI

#### Week 9: Polish & Testing

**Tasks:**
1. Performance optimization
2. Comprehensive testing
3. Error handling improvements
4. Documentation
5. Edge case handling

**Deliverables:**
- Performance optimizations (chunking, compression)
- Comprehensive test coverage
- User documentation
- Edge case handling

**Optimization Targets:**
- Chunked file transfer (64KB chunks)
- Concurrent transfers (up to 3 files)
- Debouncing (2 second delay after changes)
- Binary diff for large files

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
    RequestManifest,
    Manifest(Vec<FileManifest>),
    RequestFile { path: String },
    FileChunk { path: String, offset: u64, data: Vec<u8>, is_final: bool },
    FileChanged { path: String },
    SyncComplete { has_conflicts: bool },
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
  file_count?: number;
  total_size?: number;
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
└── discovery.rs     # Peer discovery
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
    └── ConflictResolutionDialog.tsx # Conflict resolution
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
   - [ ] Test on different networks
   - [ ] Verify WebRTC connection establishes
   - [ ] Verify relay fallback works
   - [ ] Test sync over WAN

9. **Multi-peer Sharing**
   - [ ] Share folder with multiple people
   - [ ] Verify all peers receive updates
   - [ ] Test concurrent edits from multiple peers

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
| File chunk size | 64 KB |
| Concurrent transfers | Up to 3 files |
| Sync debounce delay | 2 seconds |
| Max file size | 100 MB |
| Max folder size | 5 GB |
| Max concurrent shares | 10 |
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
    "webrtc", "kad",  # Phase 3
    "serde",
] }
sha2 = "0.10"
rand = "0.8"
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

### Phase 3: WAN + Polish
- [ ] Add WebRTC transport
- [ ] Implement relay client
- [ ] Add Auto NAT
- [ ] Implement Kademlia DHT
- [ ] Implement multi-peer sharing
- [ ] Create activity log
- [ ] Performance optimization
- [ ] Comprehensive testing
- [ ] Documentation

---

**Document Status:** Ready for Implementation
