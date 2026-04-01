import { useState } from "react";
import { createPortal } from "react-dom";
import { Button } from "../ui";
import { CopyIcon, FolderIcon, ShareIcon, XIcon } from "../icons";
import { useShare } from "../../context/ShareContext";
import type { SharePermission } from "../../types/share";
import { permissionDisplayName } from "../../types/share";
import QrCodeDisplay from "./QrCodeDisplay";

interface ShareModalProps {
  folderPath: string;
  folderName: string;
  onClose: () => void;
}

export default function ShareModal({ folderPath, folderName, onClose }: ShareModalProps) {
  const { createShare, isCreating, error, clearError } = useShare();
  const [permission, setPermission] = useState<SharePermission>("read_write");
  const [inviteCode, setInviteCode] = useState<string | null>(null);
  const [showQrCode, setShowQrCode] = useState(false);
  const [copied, setCopied] = useState(false);

  const handleCreateShare = async () => {
    clearError();
    const result = await createShare(folderPath, permission);
    if (result) {
      setInviteCode(result.invite_code);
    }
  };

  const handleCopy = async () => {
    if (inviteCode) {
      await navigator.clipboard.writeText(inviteCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const content = (
    <>
      <div
        className="fixed inset-0 z-[80] bg-black/45 backdrop-blur-sm flex items-center justify-center p-4"
        onMouseDown={(e) => {
          if (e.target === e.currentTarget) onClose();
        }}
      >
        <div className="w-full max-w-lg rounded-xl border border-border bg-bg shadow-2xl">
          <div className="flex items-center justify-between px-5 py-4 border-b border-border">
            <div className="flex items-center gap-2.5">
              <span className="inline-flex h-8 w-8 items-center justify-center rounded-md bg-accent/15 text-accent">
                <ShareIcon className="w-4 h-4 stroke-[1.8]" />
              </span>
              <div>
                <h2 className="text-base font-semibold text-text">Share Folder</h2>
                <p className="text-xs text-text-muted">Generate an invite for secure sharing</p>
              </div>
            </div>
            <button onClick={onClose} className="p-1.5 rounded-md hover:bg-bg-muted" aria-label="Close">
              <XIcon className="w-4.5 h-4.5" />
            </button>
          </div>

          <div className="px-5 py-4 space-y-4">
            <div className="flex items-center gap-3 p-3 rounded-lg bg-bg-secondary border border-border">
              <span className="inline-flex h-9 w-9 items-center justify-center rounded-md bg-bg-muted">
                <FolderIcon className="w-4.5 h-4.5 text-text-muted stroke-[1.7]" />
              </span>
              <div className="min-w-0">
                <div className="font-medium truncate">{folderName}</div>
                <div className="text-xs text-text-muted truncate">{folderPath || "Root folder"}</div>
              </div>
            </div>

            {inviteCode ? (
              <>
                <div className="space-y-2">
                  <label className="text-xs uppercase tracking-wide text-text-muted">Invite Code</label>
                  <div className="p-3 rounded-lg border border-border bg-bg-secondary font-mono text-xs leading-relaxed break-all text-text">
                    {inviteCode}
                  </div>
                </div>

                <div className="flex gap-2">
                  <Button variant="outline" onClick={handleCopy} className="flex-1">
                    <CopyIcon className="w-4 h-4 mr-2" />
                    {copied ? "Copied" : "Copy Code"}
                  </Button>
                  <Button variant="outline" onClick={() => setShowQrCode(true)} className="flex-1">
                    Show QR
                  </Button>
                </div>

                <p className="text-xs text-text-muted">
                  Send this code to the collaborator. They can paste it in “Accept Share” to join.
                </p>
              </>
            ) : (
              <>
                <div className="space-y-2">
                  <label className="text-xs uppercase tracking-wide text-text-muted">Permission</label>
                  <div className="space-y-2">
                    <label
                      className={`flex items-start gap-3 p-3 rounded-lg border transition-colors cursor-pointer ${
                        permission === "read_write"
                          ? "border-accent bg-accent/10"
                          : "border-border hover:bg-bg-secondary"
                      }`}
                    >
                      <input
                        type="radio"
                        name="permission"
                        value="read_write"
                        checked={permission === "read_write"}
                        onChange={(e) => setPermission(e.target.value as SharePermission)}
                        className="mt-1 w-4 h-4"
                      />
                      <div>
                        <div className="font-medium">{permissionDisplayName("read_write")}</div>
                        <div className="text-xs text-text-muted">Can view, edit, and sync changes.</div>
                      </div>
                    </label>
                    <label
                      className={`flex items-start gap-3 p-3 rounded-lg border transition-colors cursor-pointer ${
                        permission === "read_only"
                          ? "border-accent bg-accent/10"
                          : "border-border hover:bg-bg-secondary"
                      }`}
                    >
                      <input
                        type="radio"
                        name="permission"
                        value="read_only"
                        checked={permission === "read_only"}
                        onChange={(e) => setPermission(e.target.value as SharePermission)}
                        className="mt-1 w-4 h-4"
                      />
                      <div>
                        <div className="font-medium">{permissionDisplayName("read_only")}</div>
                        <div className="text-xs text-text-muted">Can view and sync, but cannot edit.</div>
                      </div>
                    </label>
                  </div>
                </div>

                {error && <div className="p-2.5 rounded-md bg-red-500/10 text-red-500 text-sm">{error}</div>}
              </>
            )}
          </div>

          <div className="flex justify-end gap-2 px-5 py-4 border-t border-border">
            {inviteCode ? (
              <Button variant="default" onClick={onClose}>Done</Button>
            ) : (
              <>
                <Button variant="ghost" onClick={onClose}>Cancel</Button>
                <Button variant="default" onClick={handleCreateShare} disabled={isCreating}>
                  {isCreating ? "Creating..." : "Create Share"}
                </Button>
              </>
            )}
          </div>
        </div>
      </div>

      {showQrCode && inviteCode && (
        <QrCodeDisplay
          inviteCode={inviteCode}
          folderName={folderName}
          onClose={() => setShowQrCode(false)}
        />
      )}
    </>
  );

  return createPortal(content, document.body);
}
