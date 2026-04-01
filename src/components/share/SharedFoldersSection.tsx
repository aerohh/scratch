import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useShare } from "../../context/ShareContext";
import { Button } from "../ui";
import AcceptShareModal from "./AcceptShareModal";
import SyncStatusIndicator from "./SyncStatusIndicator";
import { FolderIcon, LinkIcon, RefreshCwIcon, ShareIcon, TrashIcon } from "../icons";

export default function SharedFoldersSection() {
  const {
    shares,
    p2pStatus,
    isP2PRunning,
    isLoading,
    error,
    clearError,
    startP2P,
    stopP2P,
    revokeShare,
    manualSync,
    refreshShares,
    syncProgress,
  } = useShare();
  const [acceptOpen, setAcceptOpen] = useState(false);
  const sortedShares = useMemo(
    () =>
      [...shares].sort((a, b) => {
        if (a.created_at !== b.created_at) return b.created_at - a.created_at;
        return a.remote_path.localeCompare(b.remote_path);
      }),
    [shares]
  );

  const handleToggleNetwork = async () => {
    clearError();
    try {
      if (isP2PRunning) {
        await stopP2P();
        toast.success("P2P networking stopped");
      } else {
        await startP2P();
        toast.success("P2P networking started");
      }
    } catch {
      // Surface error via context
    }
  };

  return (
    <section className="space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-lg font-medium">Shared Folders</h2>
          <p className="text-sm text-text-muted">
            Manage collaborative folders, run manual sync, and revoke access.
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setAcceptOpen(true)}>
            <LinkIcon className="w-4 h-4 mr-2" />
            Accept Share
          </Button>
          <Button variant={isP2PRunning ? "ghost" : "default"} onClick={handleToggleNetwork}>
            {isP2PRunning ? "Stop P2P" : "Start P2P"}
          </Button>
        </div>
      </div>

      <div className="rounded-lg border border-border bg-bg-secondary px-4 py-3 flex items-center justify-between">
        <div className="text-sm text-text-muted">
          Peer ID: <span className="font-mono text-xs">{p2pStatus?.peer_id || "not running"}</span>
        </div>
        <div className="text-sm text-text-muted">
          Connected peers: <span className="text-text">{p2pStatus?.connected_peers ?? 0}</span>
        </div>
      </div>

      {error ? (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 text-red-500 text-sm px-3 py-2">
          {error}
        </div>
      ) : null}

      <div className="space-y-2">
        {isLoading ? (
          <div className="text-sm text-text-muted">Loading shared folders...</div>
        ) : sortedShares.length === 0 ? (
          <div className="rounded-lg border border-dashed border-border px-4 py-6 text-sm text-text-muted">
            No shared folders yet.
          </div>
        ) : (
          sortedShares.map((share) => {
            const progress = syncProgress[share.id];
            return (
              <div key={share.id} className="rounded-lg border border-border bg-bg px-4 py-3 space-y-3">
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <FolderIcon className="w-4 h-4 text-text-muted stroke-[1.7]" />
                      <div className="font-medium truncate">{share.remote_path || share.local_path || "Shared Folder"}</div>
                    </div>
                    <div className="text-xs text-text-muted mt-1 truncate">
                      {share.local_path || "(root)"} • {share.permission === "read_write" ? "Can edit" : "View only"}
                    </div>
                    <div className="text-xs text-text-muted truncate">
                      {share.is_owner ? "Owner" : "Joined"} • Peer {share.peer_id}
                    </div>
                  </div>

                  <div className="flex gap-1.5">
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={async () => {
                        const ok = await manualSync(share.id);
                        if (ok) toast.success("Sync started");
                      }}
                    >
                      <RefreshCwIcon className="w-3.5 h-3.5 mr-1.5" />
                      Sync
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={async () => {
                        const ok = await revokeShare(share.id);
                        if (ok) toast.success("Share revoked");
                      }}
                    >
                      <TrashIcon className="w-3.5 h-3.5 mr-1.5" />
                      Revoke
                    </Button>
                  </div>
                </div>

                <SyncStatusIndicator
                  status={share.sync_status}
                  progress={progress?.progress}
                  file={progress?.file}
                />
              </div>
            );
          })
        )}
      </div>

      <div className="flex justify-end">
        <Button variant="ghost" onClick={() => refreshShares()}>
          <ShareIcon className="w-4 h-4 mr-2" />
          Refresh Shares
        </Button>
      </div>

      {acceptOpen ? <AcceptShareModal onClose={() => setAcceptOpen(false)} /> : null}
    </section>
  );
}
