import { useMemo } from "react";
import { useShare } from "../../context/ShareContext";
import { HistoryIcon, ClockIcon } from "../icons";
import type { ActivityEntry } from "../../types/share";

interface ActivityLogProps {
  shareId: string;
  activities: ActivityEntry[];
}

export default function ActivityLog({ shareId, activities }: ActivityLogProps) {
  const { getActivityLog } = useShare();

  // Group activities by date for better organization
  const { groupedActivities, sortedActivities } = useMemo(() => {
    const sorted = [...activities].sort((a, b) => b.timestamp - a.timestamp);
    const grouped: Record<string, ActivityEntry[]> = {};

    sorted.forEach((activity) => {
      const date = new Date(activity.timestamp * 1000);
      const today = new Date();
      const yesterday = new Date(today);
      yesterday.setDate(yesterday.getDate() - 1);

      let groupKey: string;
      if (date.toDateString() === today.toDateString()) {
        groupKey = "Today";
      } else if (date.toDateString() === yesterday.toDateString()) {
        groupKey = "Yesterday";
      } else {
        groupKey = date.toLocaleDateString();
      }

      if (!grouped[groupKey]) {
        grouped[groupKey] = [];
      }
      grouped[groupKey].push(activity);
    });

    return { groupedActivities: grouped, sortedActivities: sorted };
  }, [activities]);

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);

    if (diffMins < 1) return "Just now";
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;

    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  };

  const getEventIcon = (eventType: ActivityEntry["event_type"]) => {
    switch (eventType) {
      case "peer_joined":
        return "👋";
      case "peer_left":
        return "👋";
      case "file_synced":
        return "📄";
      case "conflict_resolved":
        return "✅";
      default:
        return "•";
    }
  };

  const getEventColor = (eventType: ActivityEntry["event_type"]) => {
    switch (eventType) {
      case "peer_joined":
        return "text-green-500";
      case "peer_left":
        return "text-yellow-500";
      case "file_synced":
        return "text-blue-500";
      case "conflict_resolved":
        return "text-accent";
      default:
        return "text-text-muted";
    }
  };

  const getEventTitle = (eventType: ActivityEntry["event_type"]) => {
    switch (eventType) {
      case "peer_joined":
        return "joined";
      case "peer_left":
        return "left";
      case "file_synced":
        return "synced";
      case "conflict_resolved":
        return "resolved conflict";
      default:
        return "activity";
    }
  };

  // Handle refresh
  const handleRefresh = async () => {
    await getActivityLog(shareId, 50);
  };

  if (sortedActivities.length === 0) {
    return (
      <div className="space-y-2">
        <div className="flex items-center gap-2 text-sm text-text-muted">
          <HistoryIcon className="w-4 h-4" />
          <span>Activity Log</span>
        </div>
        <div className="text-sm text-text-muted px-3 py-4 border border-dashed border-border rounded-md">
          No activity yet. Start syncing files to see activity here.
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm">
          <HistoryIcon className="w-4 h-4 text-text-muted" />
          <span className="text-text-muted">Activity Log</span>
          <span className="text-xs text-text-muted">({sortedActivities.length})</span>
        </div>
        <button
          onClick={handleRefresh}
          className="text-xs text-text-muted hover:text-text transition-colors"
        >
          Refresh
        </button>
      </div>

      {/* Activity timeline */}
      <div className="space-y-4 max-h-64 overflow-y-auto">
        {Object.entries(groupedActivities).map(([groupKey, groupActivities]) => (
          <div key={groupKey} className="space-y-2">
            {/* Group header */}
            <div className="text-xs font-medium text-text-muted sticky top-0 bg-bg py-1">
              {groupKey}
            </div>

            {/* Activity items in this group */}
            <div className="space-y-1.5">
              {groupActivities.map((activity) => (
                <div
                  key={activity.timestamp}
                  className="flex items-start gap-2 px-3 py-2 rounded-md hover:bg-bg-muted/30 border border-border/50 transition-colors"
                >
                  {/* Icon */}
                  <div className={`w-6 h-6 rounded-full flex items-center justify-center shrink-0 ${
                    activity.event_type === "peer_joined"
                      ? "bg-green-500/10"
                      : activity.event_type === "peer_left"
                      ? "bg-yellow-500/10"
                      : activity.event_type === "file_synced"
                      ? "bg-blue-500/10"
                      : "bg-accent/10"
                  }`}>
                    <span className="text-sm">{getEventIcon(activity.event_type)}</span>
                  </div>

                  {/* Content */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm truncate">
                        {activity.peer_name || activity.peer_id.slice(0, 8)}
                      </span>
                      <span className={`text-xs ${getEventColor(activity.event_type)}`}>
                        {getEventTitle(activity.event_type)}
                      </span>
                    </div>
                    {activity.details && (
                      <div className="text-xs text-text-muted truncate">
                        {activity.details}
                      </div>
                    )}
                  </div>

                  {/* Timestamp */}
                  <div className="flex items-center gap-1 text-xs text-text-muted shrink-0">
                    <ClockIcon className="w-3 h-3" />
                    <span>{formatTimestamp(activity.timestamp)}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
