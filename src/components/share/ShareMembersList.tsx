import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useShare } from "../../context/ShareContext";
import { Button } from "../ui";
import {
  UsersIcon,
  UserCheckIcon,
  UserMinusIcon,
  ShieldCheckIcon,
  ClockIcon,
  EyeIcon,
} from "../icons";
import type { ShareMember } from "../../types/share";

interface ShareMembersListProps {
  shareId: string;
  members: ShareMember[];
  isOwner: boolean;
  currentPeerId: string;
}

export default function ShareMembersList({
  shareId,
  members,
  isOwner,
  currentPeerId,
}: ShareMembersListProps) {
  const { removeMember, updateMemberPermission } = useShare();
  const [updatingPeerId, setUpdatingPeerId] = useState<string | null>(null);

  // Check if a peer is currently connected (simplified - checks if peer_id is in connected list)
  const isPeerOnline = useMemo(() => {
    // In a real implementation, this would check against actual connection state
    // For now, we'll use a simple heuristic based on last_seen
    return (peerId: string) => {
      const member = members.find((m) => m.peer_id === peerId);
      if (!member) return false;
      // Consider online if seen in the last 5 minutes
      const fiveMinutesAgo = Math.floor(Date.now() / 1000) - 300;
      return member.last_seen > fiveMinutesAgo;
    };
  }, [members]);

  const handleRemoveMember = async (peerId: string, peerName: string | null) => {
    const confirmed = window.confirm(
      `Remove ${peerName || peerId.slice(0, 8)} from this share?`
    );
    if (!confirmed) return;

    try {
      await removeMember(shareId, peerId);
      toast.success("Member removed");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to remove member");
    }
  };

  const handleTogglePermission = async (
    peerId: string,
    currentPermission: "read_only" | "read_write"
  ) => {
    const newPermission = currentPermission === "read_only" ? "read_write" : "read_only";
    setUpdatingPeerId(peerId);

    try {
      await updateMemberPermission(shareId, peerId, newPermission);
      toast.success(`Permission updated to ${newPermission === "read_write" ? "Can edit" : "View only"}`);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to update permission");
    } finally {
      setUpdatingPeerId(null);
    }
  };

  const formatLastSeen = (timestamp: number) => {
    if (!timestamp) return "Never";
    const now = Math.floor(Date.now() / 1000);
    const diff = now - timestamp;

    if (diff < 60) return "Just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`;
    return "Long ago";
  };

  // Separate members: current user vs others
  const { currentMember, otherMembers } = useMemo(() => {
    const current = members.find((m) => m.peer_id === currentPeerId);
    const others = members.filter((m) => m.peer_id !== currentPeerId);
    return { currentMember: current, otherMembers: others };
  }, [members, currentPeerId]);

  return (
    <div className="space-y-2">
      {/* Header with count */}
      <div className="flex items-center gap-2 text-sm">
        <UsersIcon className="w-4 h-4 text-text-muted" />
        <span className="text-text-muted">
          {members.length} {members.length === 1 ? "member" : "members"}
        </span>
      </div>

      {/* Current member (you) */}
      {currentMember && (
        <div className="flex items-center gap-2 px-3 py-2 rounded-md bg-bg-muted/50 border border-border">
          <div className="flex items-center gap-2 flex-1 min-w-0">
            <div className="w-8 h-8 rounded-full bg-accent/10 flex items-center justify-center shrink-0">
              <UserCheckIcon className="w-4 h-4 text-accent" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium truncate">
                  {currentMember.peer_name || "You"}
                </span>
                {isOwner && (
                  <ShieldCheckIcon className="w-3.5 h-3.5 text-accent" />
                )}
              </div>
              <div className="flex items-center gap-2 text-xs text-text-muted">
                <span>
                  {currentMember.permission === "read_write" ? "Can edit" : "View only"}
                </span>
                <span>•</span>
                <span className="text-green-500">Online</span>
              </div>
            </div>
          </div>
          <span className="text-xs text-text-muted">(you)</span>
        </div>
      )}

      {/* Other members */}
      <div className="space-y-1.5">
        {otherMembers.length === 0 ? (
          <div className="text-sm text-text-muted px-3 py-2">
            No other members yet. Share the invite code to add more people.
          </div>
        ) : (
          otherMembers.map((member) => {
            const online = isPeerOnline(member.peer_id);
            const isUpdating = updatingPeerId === member.peer_id;

            return (
              <div
                key={member.peer_id}
                className="flex items-center gap-2 px-3 py-2 rounded-md hover:bg-bg-muted/30 border border-border/50 transition-colors"
              >
                <div className="flex items-center gap-2 flex-1 min-w-0">
                  <div className="relative">
                    <div className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 ${
                      online
                        ? "bg-green-500/10"
                        : "bg-bg-muted"
                    }`}>
                      <UsersIcon className={`w-4 h-4 ${
                        online ? "text-green-500" : "text-text-muted"
                      }`} />
                    </div>
                    {online && (
                      <div className="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-green-500 rounded-full border-2 border-bg" />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium truncate">
                        {member.peer_name || member.peer_id.slice(0, 8)}
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-xs text-text-muted">
                      <span>
                        {member.permission === "read_write" ? "Can edit" : "View only"}
                      </span>
                      <span>•</span>
                      <span className="flex items-center gap-1">
                        <ClockIcon className="w-3 h-3" />
                        {online ? "Online" : formatLastSeen(member.last_seen)}
                      </span>
                    </div>
                  </div>
                </div>

                {/* Actions - only for owners */}
                {isOwner && (
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleTogglePermission(member.peer_id, member.permission)}
                      disabled={isUpdating}
                      title={`Change to ${member.permission === "read_only" ? "Can edit" : "View only"}`}
                    >
                      {member.permission === "read_only" ? (
                        <EyeIcon className="w-3.5 h-3.5" />
                      ) : (
                        <ShieldCheckIcon className="w-3.5 h-3.5" />
                      )}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleRemoveMember(member.peer_id, member.peer_name)}
                      title="Remove member"
                    >
                      <UserMinusIcon className="w-3.5 h-3.5 text-red-500" />
                    </Button>
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
