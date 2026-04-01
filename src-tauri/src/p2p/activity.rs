// Activity logging for P2P sharing (Phase 3)

use crate::p2p::types::{ActivityEntry, ActivityEventType};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::fs;

const MAX_ACTIVITY_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActivityLog {
    entries: Vec<ActivityEntry>,
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

pub struct ActivityTracker {
    notes_root: PathBuf,
    logs: Arc<Mutex<HashMap<String, ActivityLog>>>,
}

impl ActivityTracker {
    pub fn new(notes_root: PathBuf) -> Self {
        Self {
            notes_root,
            logs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn log_path(&self, share_id: &str) -> PathBuf {
        self.notes_root
            .join(".scratch")
            .join("sync")
            .join(share_id)
            .join("activity.json")
    }

    async fn load_log(&self, share_id: &str) -> Result<ActivityLog> {
        let path = self.log_path(share_id);
        if path.exists() {
            let content = fs::read_to_string(&path).await?;
            let log = serde_json::from_str(&content)?;
            Ok(log)
        } else {
            Ok(ActivityLog::default())
        }
    }

    async fn save_log(&self, share_id: &str, log: &ActivityLog) -> Result<()> {
        let path = self.log_path(share_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let content = serde_json::to_string_pretty(log)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    pub async fn log_activity(
        &self,
        share_id: &str,
        event_type: ActivityEventType,
        peer_id: &str,
        peer_name: Option<String>,
        details: String,
    ) -> Result<()> {
        let entry = ActivityEntry {
            timestamp: Utc::now().timestamp(),
            event_type,
            peer_id: peer_id.to_string(),
            peer_name,
            details,
        };

        // Check if we have cached log, drop lock before async operations
        let need_load = {
            let logs = self.logs.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            !logs.contains_key(share_id)
        };

        // Load existing log or create new (without holding lock)
        let mut log = if need_load {
            self.load_log(share_id).await.unwrap_or_default()
        } else {
            // Get from cache and drop lock immediately
            let logs = self.logs.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            logs.get(share_id).cloned().unwrap_or_default()
        };

        log.entries.push(entry);

        // Trim to max entries
        if log.entries.len() > MAX_ACTIVITY_ENTRIES {
            log.entries = log.entries.split_off(log.entries.len() - MAX_ACTIVITY_ENTRIES);
        }

        // Save to disk
        self.save_log(share_id, &log).await?;

        // Update in-memory cache
        let mut logs = self.logs.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
        logs.insert(share_id.to_string(), log);

        Ok(())
    }

    pub async fn get_activity(&self, share_id: &str, limit: Option<usize>) -> Result<Vec<ActivityEntry>> {
        // Check if log is in cache first
        let cached_log = {
            let logs = self.logs.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            if logs.contains_key(share_id) {
                Some(logs.get(share_id).cloned().unwrap())
            } else {
                None
            }
        };

        let log = match cached_log {
            Some(log) => log,
            None => {
                let log = self.load_log(share_id).await.unwrap_or_default();
                let mut logs = self.logs.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
                logs.insert(share_id.to_string(), log.clone());
                log
            }
        };

        let mut entries = log.entries;
        entries.reverse(); // Most recent first

        if let Some(limit) = limit {
            entries.truncate(limit);
        }

        Ok(entries)
    }

    pub async fn get_activity_since(&self, share_id: &str, since: i64) -> Result<Vec<ActivityEntry>> {
        let all_entries = self.get_activity(share_id, None).await?;
        Ok(all_entries
            .into_iter()
            .filter(|e| e.timestamp > since)
            .collect())
    }
}
