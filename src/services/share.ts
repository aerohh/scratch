// P2P Sharing service

import { invoke } from "@tauri-apps/api/core";
import type {
  SharedFolder,
  P2PStatus,
  CreateShareOptions,
  AcceptShareOptions,
  CreateShareResult,
  SyncStatus,
} from "../types/share";

/**
 * Start P2P networking
 */
export async function startP2P(): Promise<P2PStatus> {
  return invoke("p2p_start");
}

/**
 * Stop P2P networking
 */
export async function stopP2P(): Promise<void> {
  return invoke("p2p_stop");
}

/**
 * Get current P2P status
 */
export async function getP2PStatus(): Promise<P2PStatus> {
  return invoke("p2p_get_status");
}

/**
 * Create a new share for a folder
 */
export async function createShare(
  options: CreateShareOptions
): Promise<CreateShareResult> {
  return invoke("p2p_create_share", {
    folderPath: options.folder_path,
    permission: options.permission,
  });
}

/**
 * Accept a share invite
 */
export async function acceptShare(
  options: AcceptShareOptions
): Promise<SharedFolder> {
  return invoke("p2p_accept_share", {
    inviteCode: options.invite_code,
    destinationPath: options.destination_path,
  });
}

/**
 * List all shares
 */
export async function listShares(): Promise<SharedFolder[]> {
  return invoke("p2p_list_shares");
}

/**
 * Revoke a share
 */
export async function revokeShare(shareId: string): Promise<void> {
  return invoke("p2p_revoke_share", { shareId });
}

/**
 * Leave or revoke a share with optional local folder cleanup.
 */
export async function revokeOrLeaveShare(
  shareId: string,
  deleteLocalData: boolean
): Promise<void> {
  return invoke("p2p_revoke_share", { shareId, deleteLocalData });
}

/**
 * Get sync status for a specific share
 */
export async function getSyncStatus(shareId: string): Promise<SyncStatus> {
  return invoke("p2p_get_sync_status", { shareId });
}

/**
 * Trigger a manual sync for a share
 */
export async function manualSync(shareId: string): Promise<void> {
  return invoke("p2p_manual_sync", { shareId });
}

export async function updateShareState(
  shareId: string,
  status?: SyncStatus,
  lastSynced?: number
): Promise<void> {
  return invoke("p2p_update_share_state", {
    shareId,
    status,
    lastSynced,
  });
}

/**
 * Resolve sync conflict for a shared file
 */
export async function resolveConflict(
  shareId: string,
  filePath: string,
  resolution: "keep_local" | "keep_remote" | "keep_both"
): Promise<void> {
  return invoke("p2p_resolve_conflict", {
    shareId,
    filePath,
    resolution,
  });
}

/**
 * Validate an invite code format
 */
export async function validateInviteCode(inviteCode: string): Promise<boolean> {
  try {
    // Basic format validation
    if (!inviteCode.startsWith("scratch-share-")) {
      return false;
    }

    // Try to decode (will fail if invalid)
    const payload = atob(inviteCode.replace("scratch-share-", ""));
    JSON.parse(payload);
    return true;
  } catch {
    return false;
  }
}

/**
 * Copy invite code to clipboard
 */
export async function copyInviteCode(inviteCode: string): Promise<void> {
  await navigator.clipboard.writeText(inviteCode);
}

/**
 * Parse invite code to get folder info (client-side only)
 */
export function parseInviteCode(inviteCode: string): {
  folderName: string;
  permission: string;
} | null {
  try {
    const payload = atob(inviteCode.replace("scratch-share-", ""));
    const data = JSON.parse(payload);

    return {
      folderName: data.folder_name || "Shared Folder",
      permission: data.permissions === "read_write" ? "Can edit" : "View only",
    };
  } catch {
    return null;
  }
}
