import { useState, useEffect } from "react";
import { createPortal } from "react-dom";
import { Button } from "../ui";
import { FolderIcon, ShareIcon, XIcon } from "../icons";
import { useShare } from "../../context/ShareContext";
import { parseInviteCode, validateInviteCode } from "../../services/share";

interface AcceptShareModalProps {
  inviteCode?: string;
  onClose: () => void;
}

export default function AcceptShareModal({ inviteCode = "", onClose }: AcceptShareModalProps) {
  const { acceptShare, isAccepting, error, clearError } = useShare();
  const [inviteCodeInput, setInviteCodeInput] = useState(inviteCode);
  const [destinationPath, setDestinationPath] = useState("");
  const [isValid, setIsValid] = useState(false);
  const [folderInfo, setFolderInfo] = useState<{ folderName: string; permission: string } | null>(null);

  useEffect(() => {
    if (!inviteCodeInput.trim()) {
      setIsValid(false);
      setFolderInfo(null);
      return;
    }

    validateInviteCode(inviteCodeInput).then((valid) => {
      setIsValid(valid);
      if (valid) {
        const info = parseInviteCode(inviteCodeInput);
        setFolderInfo(info);
      } else {
        setFolderInfo(null);
      }
    });
  }, [inviteCodeInput]);

  const handleAccept = async () => {
    if (!inviteCodeInput.trim()) return;
    clearError();
    const result = await acceptShare(inviteCodeInput.trim(), destinationPath);
    if (result) {
      onClose();
    }
  };

  const content = (
    <div
      className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-[80] p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-md rounded-xl border border-border bg-bg shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-[var(--border)]">
          <div className="flex items-center gap-2">
            <ShareIcon className="w-4 h-4 text-accent stroke-[1.8]" />
            <h2 className="text-lg font-semibold">Accept Shared Folder</h2>
          </div>
          <button
            onClick={onClose}
            className="p-1 hover:bg-[var(--bg-muted)] rounded"
            aria-label="Close"
          >
            <XIcon className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Invite code</label>
            <textarea
              value={inviteCodeInput}
              onChange={(e) => setInviteCodeInput(e.target.value)}
              placeholder="Paste invite code here (starts with scratch-share-...)"
              rows={3}
              className="w-full px-3 py-2 bg-[var(--bg-secondary)] border border-[var(--border)] rounded-lg focus:outline-none focus:ring-2 focus:ring-[var(--accent)] font-mono text-xs resize-none"
            />
            {inviteCodeInput.trim() && !isValid && (
              <div className="text-xs text-red-500">
                Invalid invite code. Please check and try again.
              </div>
            )}
          </div>

          {!inviteCodeInput.trim() ? (
            <div className="text-center py-3 text-sm text-[var(--text-muted)]">
              Paste an invite code to preview and accept the shared folder.
            </div>
          ) : (
            <>
              {/* Folder preview */}
              {folderInfo && (
                <div className="flex items-center gap-3 p-3 bg-[var(--bg-secondary)] rounded-lg">
                  <div className="w-10 h-10 bg-[var(--bg-muted)] rounded-lg flex items-center justify-center">
                    <FolderIcon className="w-5 h-5 text-text-muted stroke-[1.7]" />
                  </div>
                  <div className="flex-1">
                    <div className="font-medium">{folderInfo.folderName}</div>
                    <div className="text-sm text-[var(--text-muted)]">
                      Permission: {folderInfo.permission}
                    </div>
                  </div>
                </div>
              )}

              {/* Destination path */}
              <div className="space-y-2">
                <label className="text-sm font-medium">Save to folder</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={destinationPath}
                    onChange={(e) => setDestinationPath(e.target.value)}
                    placeholder="Leave empty to save to root"
                    className="flex-1 px-3 py-2 bg-[var(--bg-secondary)] border border-[var(--border)] rounded-lg focus:outline-none focus:ring-2 focus:ring-[var(--accent)]"
                  />
                </div>
                <div className="text-xs text-[var(--text-muted)]">
                  The folder will be created inside your notes folder
                </div>
              </div>

              {error && (
                <div className="p-3 bg-red-500/10 text-red-500 rounded-lg text-sm">
                  {error}
                </div>
              )}

              <div className="text-sm text-[var(--text-muted)]">
                Once accepted, this folder will sync automatically. You can find it in your
                folder tree.
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 p-4 border-t border-[var(--border)]">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="default"
            onClick={handleAccept}
            disabled={!inviteCodeInput.trim() || !isValid || isAccepting}
          >
            {isAccepting ? "Accepting..." : "Accept"}
          </Button>
        </div>
      </div>
    </div>
  );

  return createPortal(content, document.body);
}
