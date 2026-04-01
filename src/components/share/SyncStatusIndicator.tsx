import type { SyncStatus } from "../../types/share";
import { syncStatusDisplayName } from "../../types/share";

interface SyncStatusIndicatorProps {
  status: SyncStatus;
  progress?: number;
  file?: string;
}

export default function SyncStatusIndicator({ status, progress, file }: SyncStatusIndicatorProps) {
  const color =
    status === "synced"
      ? "bg-green-500"
      : status === "conflict" || status === "error"
        ? "bg-red-500"
        : status === "syncing" || status === "connecting" || status === "discovering_peer"
          ? "bg-yellow-500"
          : "bg-gray-400";

  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2 text-sm">
        <span className={`inline-block w-2 h-2 rounded-full ${color}`} />
        <span>{syncStatusDisplayName(status)}</span>
      </div>
      {typeof progress === "number" && (
        <>
          <div className="h-1.5 w-full rounded bg-bg-muted overflow-hidden">
            <div
              className="h-full bg-accent transition-all duration-200"
              style={{ width: `${Math.max(0, Math.min(100, progress))}%` }}
            />
          </div>
          {file ? <div className="text-xs text-text-muted truncate">{file}</div> : null}
        </>
      )}
    </div>
  );
}
