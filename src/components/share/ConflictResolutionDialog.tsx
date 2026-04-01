import { createPortal } from "react-dom";
import { Button } from "../ui";

interface ConflictItem {
  file_path: string;
  local_hash: string;
  remote_hash: string;
}

interface ConflictResolutionDialogProps {
  shareName: string;
  conflicts: ConflictItem[];
  onResolve: (filePath: string, resolution: "keep_local" | "keep_remote" | "keep_both") => Promise<void> | void;
  onClose: () => void;
}

export default function ConflictResolutionDialog({
  shareName,
  conflicts,
  onResolve,
  onClose,
}: ConflictResolutionDialogProps) {
  return createPortal(
    <div
      className="fixed inset-0 z-[90] bg-black/45 backdrop-blur-sm flex items-center justify-center p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-2xl rounded-xl border border-border bg-bg shadow-2xl">
        <div className="px-5 py-4 border-b border-border">
          <h2 className="text-base font-semibold">Resolve Conflicts</h2>
          <p className="text-xs text-text-muted mt-1">
            {shareName}: choose how each conflicting file should be resolved.
          </p>
        </div>
        <div className="p-4 space-y-3 max-h-[60vh] overflow-auto">
          {conflicts.map((conflict) => (
            <div key={conflict.file_path} className="rounded-lg border border-border p-3 space-y-2">
              <div className="font-mono text-xs break-all">{conflict.file_path}</div>
              <div className="text-[11px] text-text-muted">
                Local: {conflict.local_hash.slice(0, 8)} | Remote: {conflict.remote_hash.slice(0, 8)}
              </div>
              <div className="flex gap-2">
                <Button size="sm" variant="outline" onClick={() => void onResolve(conflict.file_path, "keep_local")}>
                  Keep Local
                </Button>
                <Button size="sm" variant="outline" onClick={() => void onResolve(conflict.file_path, "keep_remote")}>
                  Keep Remote
                </Button>
                <Button size="sm" variant="default" onClick={() => void onResolve(conflict.file_path, "keep_both")}>
                  Keep Both
                </Button>
              </div>
            </div>
          ))}
        </div>
        <div className="px-5 py-4 border-t border-border flex justify-end">
          <Button variant="ghost" onClick={onClose}>Close</Button>
        </div>
      </div>
    </div>,
    document.body
  );
}
