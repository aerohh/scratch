import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useShare } from "../../context/ShareContext";
import { Button } from "../ui";
import AcceptShareModal from "./AcceptShareModal";
import ConflictResolutionDialog from "./ConflictResolutionDialog";
import SyncStatusIndicator from "./SyncStatusIndicator";
import ShareMembersList from "./ShareMembersList";
import ActivityLog from "./ActivityLog";
import {
  FolderIcon,
  LinkIcon,
  RefreshCwIcon,
  ShareIcon,
  TrashIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  UsersIcon,
} from "../icons";

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
    resolveConflict,
    refreshShares,
    syncProgress,
    conflicts,
    activities,
    getActivityLog,
  } = useShare();
  const [acceptOpen, setAcceptOpen] = useState(false);
  const [conflictShareId, setConflictShareId] = useState<string | null>(null);
  const [expandedShares, setExpandedShares] = useState<Set<string>>(new Set());
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

  const formatLastSynced = (timestamp: number) => {
    if (!timestamp) return "Never synced";
    const date = new Date(timestamp * 1000);
    return `Last synced ${date.toLocaleString()}`;
  };

  const toggleExpand = async (shareId: string) => {
    const newExpanded = new Set(expandedShares);
    if (newExpanded.has(shareId)) {
      newExpanded.delete(shareId);
    } else {
      newExpanded.add(shareId);
      // Load activities if not already loaded
      if (!activities[shareId]) {
        try {
          await getActivityLog(shareId, 50);
        } catch {
          // Silently fail - activity log is optional
        }
      }
    }
    setExpandedShares(newExpanded);
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
            const isExpanded = expandedShares.has(share.id);
            const showExpand = share.members && share.members.length > 1;

            return (
              <div key={share.id} className="rounded-lg border border-border bg-bg overflow-hidden">
                {/* Main share card */}
                <div className="px-4 py-3 space-y-3">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        {showExpand && (
                          <button
                            onClick={() => toggleExpand(share.id)}
                            className="p-0.5 rounded hover:bg-bg-muted transition-colors"
                            aria-label={isExpanded ? "Collapse" : "Expand"}
                          >
                            {isExpanded ? (
                              <ChevronDownIcon className="w-4 h-4 text-text-muted" />
                            ) : (
                              <ChevronRightIcon className="w-4 h-4 text-text-muted" />
                            )}
                          </button>
                        )}
                        <FolderIcon className="w-4 h-4 text-text-muted stroke-[1.7]" />
                        <div className="font-medium truncate">{share.remote_path || share.local_path || "Shared Folder"}</div>
                      </div>
                      <div className="text-xs text-text-muted mt-1 truncate">
                        {share.local_path || "(root)"} • {share.permission === "read_write" ? "Can edit" : "View only"}
                      </div>
                      <div className="text-xs text-text-muted truncate">
                        {share.is_owner ? "Owner" : "Joined"} • Peer {share.peer_id}
                      </div>
                      <div className="text-xs text-text-muted truncate">
                        {formatLastSynced(share.last_synced)}
                      </div>
                    </div>

                    <div className="flex gap-1.5">
                      {showExpand && !isExpanded && (
                        <button
                          onClick={() => toggleExpand(share.id)}
                          className="text-xs text-text-muted hover:text-text flex items-center gap-1 px-2 py-1 rounded hover:bg-bg-muted transition-colors"
                        >
                          <UsersIcon className="w-3.5 h-3.5" />
                          {share.members?.length || 0}
                        </button>
                      )}
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
                          const action = share.is_owner ? "revoke" : "leave";
                          const confirmed = window.confirm(
                            share.is_owner
                              ? "Revoke this share for connected peers?"
                              : "Leave this shared folder and remove local shared data?"
                          );
                          if (!confirmed) return;
                          const ok = await revokeShare(share.id, !share.is_owner);
                          if (ok) toast.success(action === "revoke" ? "Share revoked" : "Left shared folder");
                        }}
                      >
                        <TrashIcon className="w-3.5 h-3.5 mr-1.5" />
                        {share.is_owner ? "Revoke" : "Leave"}
                      </Button>
                      {(conflicts[share.id] || []).length > 0 && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => setConflictShareId(share.id)}
                        >
                          Resolve
                        </Button>
                      )}
                    </div>
                  </div>

                  <SyncStatusIndicator
                    status={share.sync_status}
                    progress={progress?.progress}
                    file={progress?.file}
                  />
                </div>

                {/* Expandable section for members and activity */}
                {isExpanded && showExpand && (
                  <div className="border-t border-border px-4 py-3 space-y-4 bg-bg-secondary/30">
                    {/* Members List */}
                    {share.members && share.members.length > 0 && (
                      <ShareMembersList
                        shareId={share.id}
                        members={share.members}
                        isOwner={share.is_owner}
                        currentPeerId={p2pStatus?.peer_id || ""}
                      />
                    )}

                    {/* Activity Log */}
                    <ActivityLog
                      shareId={share.id}
                      activities={activities[share.id] || []}
                    />
                  </div>
                )}
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
      {conflictShareId ? (
        <ConflictResolutionDialog
          shareName={sortedShares.find((share) => share.id === conflictShareId)?.remote_path || "Shared Folder"}
          conflicts={conflicts[conflictShareId] || []}
          onResolve={async (filePath, resolution) => {
            const ok = await resolveConflict(conflictShareId, filePath, resolution);
            if (ok) {
              toast.success("Conflict resolved");
            }
          }}
          onClose={() => setConflictShareId(null)}
        />
      ) : null}
    </section>
  );
}
