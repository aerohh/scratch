// P2P Sharing types

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

export interface CreateShareResult {
  invite_code: string;
  share_id: string;
}

export type ConflictResolution = 'keep_local' | 'keep_remote' | 'keep_both';

export interface PeerInfo {
  peer_id: string;
  peer_name: string | null;
  addresses: string[];
}

// Helper functions for type guards
export function isSynced(status: SyncStatus): boolean {
  return status === 'synced';
}

export function isSyncing(status: SyncStatus): boolean {
  return status === 'discovering_peer' || status === 'connecting' || status === 'syncing';
}

export function isError(status: SyncStatus): boolean {
  return status === 'error' || status === 'conflict';
}

// Permission display names
export function permissionDisplayName(permission: SharePermission): string {
  switch (permission) {
    case 'read_only':
      return 'View only';
    case 'read_write':
      return 'Can edit';
  }
}

// Sync status display helpers
export function syncStatusDisplayName(status: SyncStatus): string {
  switch (status) {
    case 'idle':
      return 'Idle';
    case 'discovering_peer':
      return 'Discovering peer...';
    case 'connecting':
      return 'Connecting...';
    case 'syncing':
      return 'Syncing...';
    case 'synced':
      return 'Synced';
    case 'conflict':
      return 'Conflict detected';
    case 'error':
      return 'Error';
  }
}

export function syncStatusColor(status: SyncStatus): string {
  switch (status) {
    case 'idle':
      return 'text-gray-500';
    case 'discovering_peer':
    case 'connecting':
    case 'syncing':
      return 'text-yellow-500';
    case 'synced':
      return 'text-green-500';
    case 'conflict':
    case 'error':
      return 'text-red-500';
  }
}
