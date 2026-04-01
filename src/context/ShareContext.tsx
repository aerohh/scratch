import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";
import * as shareService from "../services/share";
import type {
  SharedFolder,
  P2PStatus,
  SharePermission,
  CreateShareResult,
  SyncStatus,
} from "../types/share";

function normalizeSyncStatus(status: unknown): SyncStatus {
  if (typeof status === "string") {
    const normalized = status.toLowerCase();
    if (
      normalized === "idle" ||
      normalized === "discovering_peer" ||
      normalized === "connecting" ||
      normalized === "syncing" ||
      normalized === "synced" ||
      normalized === "conflict" ||
      normalized === "error"
    ) {
      return normalized;
    }
    return "error";
  }

  if (status && typeof status === "object" && "Error" in (status as Record<string, unknown>)) {
    return "error";
  }

  return "error";
}

interface ShareContextValue {
  // State
  shares: SharedFolder[];
  p2pStatus: P2PStatus | null;
  isP2PRunning: boolean;
  isLoading: boolean;
  isCreating: boolean;
  isAccepting: boolean;
  syncProgress: Record<string, { progress: number; file?: string }>;
  error: string | null;

  // Actions
  startP2P: () => Promise<void>;
  stopP2P: () => Promise<void>;
  createShare: (folderPath: string, permission: SharePermission) => Promise<CreateShareResult | null>;
  acceptShare: (inviteCode: string, destinationPath: string) => Promise<SharedFolder | null>;
  revokeShare: (shareId: string) => Promise<boolean>;
  manualSync: (shareId: string) => Promise<boolean>;
  refreshShares: () => Promise<void>;
  refreshP2PStatus: () => Promise<void>;
  clearError: () => void;
}

const ShareContext = createContext<ShareContextValue | null>(null);

export function ShareProvider({ children }: { children: ReactNode }) {
  const [shares, setShares] = useState<SharedFolder[]>([]);
  const [p2pStatus, setP2PStatus] = useState<P2PStatus | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [isAccepting, setIsAccepting] = useState(false);
  const [syncProgress, setSyncProgress] = useState<Record<string, { progress: number; file?: string }>>({});
  const [error, setError] = useState<string | null>(null);

  // Use refs to avoid stale closure issues
  const refreshRequestIdRef = useRef(0);
  const p2pStatusRequestIdRef = useRef(0);
  const sharesRef = useRef(shares);
  sharesRef.current = shares;

  // Computed value
  const isP2PRunning = useMemo(() => p2pStatus?.is_running ?? false, [p2pStatus]);

  // Clear error helper
  const clearError = useCallback(() => {
    setError(null);
  }, []);

  // Refresh P2P status
  const refreshP2PStatus = useCallback(async () => {
    const requestId = ++p2pStatusRequestIdRef.current;
    try {
      const status = await shareService.getP2PStatus();
      if (requestId === p2pStatusRequestIdRef.current) {
        setP2PStatus(status);
      }
    } catch (err) {
      if (requestId === p2pStatusRequestIdRef.current) {
        setError(err instanceof Error ? err.message : "Failed to get P2P status");
      }
    }
  }, []);

  // Refresh shares list
  const refreshShares = useCallback(async () => {
    const requestId = ++refreshRequestIdRef.current;
    setIsLoading(true);
    try {
      const sharesList = await shareService.listShares();
      if (requestId === refreshRequestIdRef.current) {
        setShares(sharesList);
      }
    } catch (err) {
      if (requestId === refreshRequestIdRef.current) {
        setError(err instanceof Error ? err.message : "Failed to load shares");
      }
    } finally {
      if (requestId === refreshRequestIdRef.current) {
        setIsLoading(false);
      }
    }
  }, []);

  // Ensure P2P is running before share operations.
  const ensureP2PRunning = useCallback(async () => {
    const status = await shareService.getP2PStatus();
    if (status.is_running) {
      setP2PStatus(status);
      return status;
    }

    const started = await shareService.startP2P();
    setP2PStatus(started);
    return started;
  }, []);

  // Start P2P
  const startP2P = useCallback(async () => {
    try {
      const status = await shareService.startP2P();
      setP2PStatus(status);
      setError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to start P2P";
      setError(message);
      throw err;
    }
  }, []);

  // Stop P2P
  const stopP2P = useCallback(async () => {
    try {
      await shareService.stopP2P();
      setP2PStatus(null);
      setShares([]);
      setError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to stop P2P";
      setError(message);
      throw err;
    }
  }, []);

  // Create share
  const createShare = useCallback(
    async (folderPath: string, permission: SharePermission): Promise<CreateShareResult | null> => {
      setIsCreating(true);
      setError(null);
      try {
        await ensureP2PRunning();
        const result = await shareService.createShare({ folder_path: folderPath, permission });
        // Refresh shares list
        await refreshShares();
        return result;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to create share";
        setError(message);
        return null;
      } finally {
        setIsCreating(false);
      }
    },
    [ensureP2PRunning, refreshShares]
  );

  // Accept share
  const acceptShare = useCallback(
    async (inviteCode: string, destinationPath: string): Promise<SharedFolder | null> => {
      setIsAccepting(true);
      setError(null);
      try {
        await ensureP2PRunning();
        const share = await shareService.acceptShare({
          invite_code: inviteCode,
          destination_path: destinationPath,
        });
        // Refresh shares list
        await refreshShares();
        return share;
      } catch (err) {
        const message = err instanceof Error ? err.message : "Failed to accept share";
        setError(message);
        return null;
      } finally {
        setIsAccepting(false);
      }
    },
    [ensureP2PRunning, refreshShares]
  );

  // Revoke share
  const revokeShare = useCallback(async (shareId: string): Promise<boolean> => {
    setError(null);
    try {
      await shareService.revokeShare(shareId);
      // Refresh shares list
      await refreshShares();
      return true;
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to revoke share";
      setError(message);
      return false;
    }
  }, [refreshShares]);

  // Manual sync
  const manualSync = useCallback(async (shareId: string): Promise<boolean> => {
    setError(null);
    try {
      await ensureP2PRunning();
      await shareService.manualSync(shareId);
      return true;
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to trigger manual sync";
      setError(message);
      return false;
    }
  }, [ensureP2PRunning]);

  // Initialize P2P on mount
  useEffect(() => {
    const initP2P = async () => {
      try {
        // Try to get current status (will fail if not started)
        await refreshP2PStatus();
        await refreshShares();
      } catch {
        // P2P not started, that's okay
        setP2PStatus({ is_running: false, peer_id: "", connected_peers: 0 });
      }
    };

    initP2P();
  }, [refreshP2PStatus, refreshShares]);

  // Listen for P2P events
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    // Listen for sync status changes
    listen("p2p-sync-status", (event: unknown) => {
      // Update share status in list
      const data = event as { payload: { share_id: string; status: unknown } };
      const status = normalizeSyncStatus(data.payload.status);
      setShares((prev) =>
        prev.map((share) =>
          share.id === data.payload.share_id
            ? { ...share, sync_status: status }
            : share
        )
      );
    }).then((unlisten) => unlisteners.push(unlisten));

    listen("p2p-sync-start", (event: unknown) => {
      const data = event as { payload: { share_id: string } };
      setSyncProgress((prev) => ({
        ...prev,
        [data.payload.share_id]: { progress: 0 },
      }));
    }).then((unlisten) => unlisteners.push(unlisten));

    listen("p2p-sync-progress", (event: unknown) => {
      const data = event as { payload: { share_id: string; progress: number; file?: string } };
      setSyncProgress((prev) => ({
        ...prev,
        [data.payload.share_id]: {
          progress: data.payload.progress ?? 0,
          file: data.payload.file,
        },
      }));
    }).then((unlisten) => unlisteners.push(unlisten));

    listen("p2p-sync-complete", (event: unknown) => {
      const data = event as { payload: { share_id: string } };
      setSyncProgress((prev) => ({
        ...prev,
        [data.payload.share_id]: { progress: 100 },
      }));
      setTimeout(() => {
        setSyncProgress((prev) => {
          const next = { ...prev };
          delete next[data.payload.share_id];
          return next;
        });
      }, 2000);
    }).then((unlisten) => unlisteners.push(unlisten));

    // Listen for peer events
    listen("p2p-peer-connected", (event: unknown) => {
      console.log("Peer connected:", event);
      refreshP2PStatus();
    }).then((unlisten) => unlisteners.push(unlisten));

    listen("p2p-peer-disconnected", (event: unknown) => {
      console.log("Peer disconnected:", event);
      refreshP2PStatus();
    }).then((unlisten) => unlisteners.push(unlisten));

    // Cleanup
    return () => {
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [refreshP2PStatus]);

  const value: ShareContextValue = useMemo(
    () => ({
      shares,
      p2pStatus,
      isP2PRunning,
      isLoading,
      isCreating,
      isAccepting,
      syncProgress,
      error,
      startP2P,
      stopP2P,
      createShare,
      acceptShare,
      revokeShare,
      manualSync,
      refreshShares,
      refreshP2PStatus,
      clearError,
    }),
    [
      shares,
      p2pStatus,
      isP2PRunning,
      isLoading,
      isCreating,
      isAccepting,
      syncProgress,
      error,
      startP2P,
      stopP2P,
      createShare,
      acceptShare,
      revokeShare,
      manualSync,
      refreshShares,
      refreshP2PStatus,
      clearError,
    ]
  );

  return <ShareContext.Provider value={value}>{children}</ShareContext.Provider>;
}

export function useShare(): ShareContextValue {
  const context = useContext(ShareContext);
  if (!context) {
    throw new Error("useShare must be used within ShareProvider");
  }
  return context;
}
