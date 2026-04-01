# Scratch - Future Features & Requirements

> **Last Updated:** 2026-04-01
> **Purpose:** This document tracks planned features and discussions for future development.

---

## Table of Contents

1. [Mobile App (React Native)](#mobile-app-react-native)
2. [P2P Sync Feature](#p2p-sync-feature)
3. [Folder Sharing (P2P)](#folder-sharing-p2p)

---

## Mobile App (React Native)

### Overview
Create a mobile version of Scratch for iOS and Android while maintaining the desktop app. Both apps will coexist and share code where possible.

### Approach: Option 1 - Pure React Native

**Architecture: Monorepo Structure**

```
scratch/
├── packages/
│   ├── shared/              # Shared code
│   │   ├── types/           # TypeScript types
│   │   ├── utils/           # Utilities (cn, folderTree, etc.)
│   │   ├── hooks/           # Business logic hooks
│   │   └── services/        # Service interfaces
│   │
├── apps/
│   ├── desktop/             # Current Tauri app
│   │   ├── src/             # React + DOM
│   │   └── src-tauri/       # Rust backend
│   │
│   └── mobile/              # New React Native app
│       ├── src/             # React Native components
│       └── ios/             # iOS native code
│       └── android/         # Android native code
```

### Code Reuse Estimate

| Layer | Reusability |
|-------|-------------|
| Types | ~95% |
| Business logic / contexts | ~70% |
| Utilities | ~80% |
| UI components | ~10% (complete rewrite) |
| Editor | ~0% (need new solution) |
| Backend logic | ~50% (concepts, not implementation) |

**Overall:** ~40-50% code reuse, ~50-60% new code needed.

### What Replaces Rust on Mobile?

| Rust Feature (Desktop) | React Native Alternative (Mobile) |
|------------------------|-----------------------------------|
| File I/O | `expo-file-system` or `react-native-fs` |
| File watching | `react-native-watch-man` or custom polling |
| Git (CLI wrapper) | `simple-git` (Node) or `isomorphic-git` |
| Full-text search (Tantivy) | `FlexSearch`, `Lunr.js`, or `expo-sqlite` |
| Native dialogs | React Native Alert/ActionSheet/Modal |

### Editor Challenge

**TipTap is DOM-based and won't work in React Native.**

Potential solutions:
- `react-native-markdown-editor` (limited features)
- `react-native-webview` + wrap TipTap (WebView-based)
- Build custom editor with `react-native-webview`
- Look for other RN markdown editors

### Key Libraries to Consider

```json
{
  "dependencies": {
    "react-native": "latest",
    "expo": "~latest",
    "expo-file-system": "~latest",
    "expo-sqlite": "~latest",
    "@react-native-clipboard/clipboard": "^1.x",
    "@react-native-community/push-notification-ios": "^1.x",
    "react-native-vector-icons": "^10.x",
    "react-native-webview": "^13.x",
    "@react-navigation/native": "^6.x",
    "@react-navigation/stack": "^6.x"
  }
}
```

### Monorepo Tools

- **Turborepo** - Fast build system for monorepos
- **Nx** - Full-featured monorepo toolkit
- **pnpm workspaces** - Simple workspace management

---

## P2P Sync Feature

### Overview
Add peer-to-peer synchronization between devices while keeping the app offline-first. Devices can sync on the same network (LAN) or different networks (WAN) without relying on a central server.

### Architecture

```
┌─────────────────┐         ┌─────────────────┐
│   Device A      │         │   Device B      │
│   (Home)        │         │   (Work)        │
├─────────────────┤         ├─────────────────┤
│ • Notes folder  │         │ • Notes folder  │
│ • Rust backend  │         │ • Rust backend  │
│                 │         │                 |
│   Sync Engine   │◄────────┤   Sync Engine   │
│   (libp2p)      │  P2P    │   (libp2p)      │
└─────────────────┘         └─────────────────┘
         │                           │
         └───────► NAT ◄─────────────┘
                  │
         (Optional lightweight
          signaling/relay server
          for discovery & NAT)
```

### Technology: libp2p (Rust)

**Why libp2p?**
- ✅ Written in Rust (fits existing stack)
- ✅ Production-tested (IPFS, Polkadot, etc.)
- ✅ Multiple transports: TCP, WebSockets, WebRTC
- ✅ NAT traversal via relay and hole punching
- ✅ Peer discovery (mDNS for LAN, DHT for WAN)
- ✅ Encrypted by default (noise protocol)
- ✅ No centralized server needed (except optional signaling)

### Dependencies to Add

```toml
# src-tauri/Cargo.toml

[dependencies]
libp2p = { version = "0.54", features = [
    "mdns",              # Local network discovery
    "tcp",               # TCP transport
    "noise",             # Encryption
    "yamux",             # Stream multiplexing
    "gossipsub",         # Pub/sub messaging
    "request-response",  # Direct messaging
    "identify",          # Peer identification
    "relay",             # NAT traversal relay
    "webrtc",            # WebRTC transport (for WAN)
    "metrics",           # Telemetry
] }
futures = "0.3"
tokio = { version = "1", features = ["full"] }
sha2 = "0.10"           # For file hashing
```

### Tauri Commands to Add

```rust
#[tauri::command]
async fn start_sync(device_id: String) -> Result<(), String>

#[tauri::command]
async fn stop_sync() -> Result<(), String>

#[tauri::command]
async fn discover_peers() -> Result<Vec<PeerInfo>, String>

#[tauri::command]
async fn sync_with_peer(peer_id: String) -> Result<SyncResult, String>

#[tauri::command]
async fn get_sync_status() -> Result<SyncStatus, String>

#[tauri::command]
async fn receive_note_update(note: NoteUpdate) -> Result<(), String>
```

### Sync Strategy

#### 1. Discovery
| Network Type | Method |
|--------------|--------|
| Same network (LAN) | mDNS broadcast — devices auto-discover |
| Different networks (WAN) | DHT + relay servers, or QR code pairing |

#### 2. Conflict Resolution Options
| Strategy | Complexity | Description |
|----------|------------|-------------|
| Last-write-wins | Low | Simple, but can lose data |
| Causal timestamps | Medium | Lamport timestamps for ordering |
| CRDTs | High | Complex, but no conflicts (automerge, Yjs) |
| Git-style | Medium | Create conflict markers, let user resolve |
| Parallel versions | Low | Create "Note (conflict copy)" |

**Recommendation:** Start with last-write-wins + conflict copies, evolve to CRDTs if needed.

#### 3. Incremental Sync
- Exchange **manifests** (note ID, modified timestamp, hash)
- Only transfer changed files
- Binary diff for large notes (rsync-style)

#### 4. Authentication Methods
| Method | Description |
|--------|-------------|
| Pairing code | 6-digit code exchanged manually |
| QR code | Scan to pair (secure, one-time) |
| Shared secret | Pre-shared key on first setup |

### UI Changes Needed

| Feature | Location |
|---------|----------|
| Sync status indicator | Sidebar or status bar |
| Peer list | Settings > Sync (new tab) |
| Pairing flow | Modal for QR/code entry |
| Conflict resolution | Dialog when conflicts detected |
| Sync log | Settings > Sync > Activity |

### Implementation Phases

| Phase | Duration | Description |
|-------|----------|-------------|
| Phase 1: Core P2P (LAN) | 1-2 weeks | mDNS discovery, TCP transport, sync engine, basic UI |
| Phase 2: NAT Traversal (WAN) | 2-3 weeks | WebRTC transport, signaling server, relay support, QR pairing |
| Phase 3: Polish | 1-2 weeks | Conflict resolution UI, sync history, selective sync, background sync |

**Total: 4-7 weeks**

---

## Folder Sharing (P2P)

### Overview
Share individual folders with other people on different networks who have the same app. All files in the shared folder sync via P2P. Similar to shared Dropbox folders, but fully decentralized.

### Sharing Flow

```
┌─────────────────────┐                          ┌─────────────────────┐
│   Alice (Sender)    │                          │   Bob (Recipient)   │
├─────────────────────┤                          ├─────────────────────┤
│                     │                          │                     |
│ 1. Right-click      │   Invite Code/QR        │ 4. Enter/Scan code  │
│    "Recipes" folder │ ──────────────────────► │    in app           │
│    → Share          │                          │                     |
│                     │                          │ 5. Preview folder   │
│ 2. Generate invite  │                          │    (name, size)     │
│    (encrypted key)  │                          │                     |
│                     │                          │ 6. Accept share     │
│ 3. Copy/Show QR     │                          │                     |
│                     │                          │ 7. Folder added     │
│                     │                          │    to tree          │
│                     │   P2P Connection         │                     |
│                     │ ◄───────────────────────► │                     │
│                     │                          │                     |
│ 8. Folders sync     │   Bidirectional Sync     │ 9. Changes sync     │
│    automatically    │                          │    automatically    │
└─────────────────────┘                          └─────────────────────┘
```

### Invite Code Format

The invite code contains (encrypted + base64 encoded):

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

**Encoded as:** `scratch-share-` + base64(payload) + signature

### Data Structures

```typescript
// types/share.ts

export type SharePermission = 'read_only' | 'read_write';

export interface ShareInvite {
  sender_peer_id: string;
  folder_id: string;
  folder_name: string;
  permissions: SharePermission;
  encryption_key: string;
  file_count: number;
  total_size: number;
  created_at: number;
}

export interface SharedFolder {
  id: string;
  local_path: string;
  remote_path: string;
  peer_id: string;
  peer_name: string;
  permission: SharePermission;
  sync_status: 'syncing' | 'synced' | 'conflict' | 'offline';
  last_synced: number;
}

export enum SyncStatus {
  Idle,
  DiscoveringPeer,
  Connecting,
  Syncing,
  Synced,
  Conflict,
  Error,
}
```

### Tauri Commands to Add

```rust
#[tauri::command]
async fn create_folder_share(
    folder_path: String,
    permission: SharePermission,
) -> Result<ShareInvite, String>

#[tauri::command]
async fn accept_folder_share(
    invite_code: String,
    destination_path: String,
) -> Result<(), String>

#[tauri::command]
async fn list_shared_folders() -> Result<Vec<SharedFolder>, String>

#[tauri::command]
async fn revoke_folder_share(
    folder_id: String,
    peer_id: String,
) -> Result<(), String>

#[tauri::command]
async fn get_folder_sync_status(
    folder_id: String,
) -> Result<SyncStatus, String>
```

### UI Components Needed

#### 1. Folder Context Menu (Enhanced)
```
Right-click on folder →
┌─────────────────────┐
│  New Note           │
│  New Folder         │
│  ─────────────────  │
│  Rename             │
│  Delete             │
│  Move               │
│  ─────────────────  │
│  👥 Share Folder... │ ← New
└─────────────────────┘
```

#### 2. Share Modal (Sender)
```
┌──────────────────────────────────┐
│   Share Folder                   │
├──────────────────────────────────┤
│  📁 Recipes                       │
│                                  │
│  Permission:                     │
│  ● Can edit (recommended)        │
│  ○ View only                     │
│                                  │
│  ─────────────────────────────  │
│                                  │
│  [📋 Copy Invite Code]           │
│  [📱 Show QR Code]               │
│                                  │
│  [Cancel]           [Share]      │
└──────────────────────────────────┘
```

#### 3. Shared Folders Settings Page
```
Settings → Shared Folders
┌──────────────────────────────────┐
│   Shared Folders                 │
├──────────────────────────────────┤
│                                  │
│  Folders you're sharing:         │
│  ┌────────────────────────────┐ │
│  │ 📁 Recipes          Share  │ │
│  │    Shared with: Bob        │ │
│  │    ● Synced 2m ago         │ │
│  │    [Revoke]                │ │
│  └────────────────────────────┘ │
│                                  │
│  Folders shared with you:        │
│  ┌────────────────────────────┐ │
│  │ 📁 Work Notes       Alice  │ │
│  │    ● Synced 5m ago         │ │
│  │    [Leave]                 │ │
│  └────────────────────────────┘ │
│                                  │
│  ─────────────────────────────  │
│                                  │
│  [Enter Invite Code]             │
└──────────────────────────────────┘
```

### Security & Privacy

| Concern | Solution |
|---------|----------|
| Eavesdropping | End-to-end encryption (Noise protocol via libp2p) |
| Unauthorized access | Invite code contains secret key |
| Leaked invite | Revoke access, regenerate invite |
| Malicious peer | Sandbox shared folder, validate file paths |
| MITM attacks | libp2p authenticates peer IDs |
| Folder traversal | Chroot shared folder, validate all paths |

### Connection Modes

| Scenario | Connection Type |
|----------|----------------|
| Same network (LAN) | Direct TCP via mDNS discovery |
| Different networks, NAT-friendly | WebRTC direct connection |
| Different networks, restrictive NAT | WebRTC via relay server |
| Offline (both) | Queues changes, syncs when online |

### Edge Cases & Handling

| Case | Handling |
|------|----------|
| Peer offline | Queue changes, sync when they come online |
| File deleted on both | Remove from manifest |
| File deleted on one, edited on other | Keep edited version, notify other peer |
| Folder name changed | Update manifest, preserve sync |
| Permission change | Revoke or downgrade, notify peer |
| Invite code leaked | Revoke access, issue new invite |
| Conflicting edits | Create conflict copy + notification |
| Large files | Binary diff, chunked transfer |
| Network interruption | Resumable transfers |
| Many files | Batched sync, incremental updates |

### Additional Dependencies

```toml
# src-tauri/Cargo.toml

[dependencies]
# ... existing libp2p deps ...

base64 = "0.22"           # For invite encoding
sha2 = "0.10"             # For file hashing
rand = "0.8"              # For key generation
serde_json = "1"          # For invite payload
qrcode = "0.14"           # For QR code generation (optional)
```

```json
// package.json (frontend)

{
  "dependencies": {
    "qrcode.react": "^4.0.0"  // QR code display
  }
}
```

### Implementation Phases

| Phase | Duration | Description |
|-------|----------|-------------|
| Phase 1: Basic Sharing | 2-3 weeks | Share context menu, invite generation, accept modal, LAN sync |
| Phase 2: Enhanced Sharing | 2-3 weeks | QR code/scanner, shared folders page, revoke access, sync status |
| Phase 3: WAN + Polish | 2-3 weeks | WebRTC WAN, relay integration, conflict UI, sync history |

**Total: 6-9 weeks**

### Bonus Features (Future)

| Feature | Description |
|---------|-------------|
| Multi-share | Share folder with multiple people |
| Expiration | Invites that expire after X days |
| Read-only → Edit | Request permission upgrade |
| Activity log | See who changed what, when |
| Selective sync | Only sync certain subfolders |
| Mobile support | Share with mobile app (future) |

---

## Notes

- All features maintain **offline-first** functionality
- P2P sync and folder sharing are built on the same libp2p foundation
- Mobile app shares business logic but has separate UI and native implementations
- Consider starting with Phase 1 of each feature to validate the approach
- All invite codes and sync traffic are encrypted by default

---

## Next Steps

1. Choose monorepo tool (Turborepo, Nx, or pnpm workspaces) for mobile project
2. Evaluate and choose a React Native markdown editor
3. Research libp2p documentation and examples
4. Decide on conflict resolution strategy for sync
5. Consider UI/UX for pairing flow

---

**Document Status:** Planning Phase - Not yet implemented
