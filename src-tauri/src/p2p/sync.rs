// File sync engine

use crate::p2p::types::{FileManifest, CHUNK_SIZE, MAX_FILE_SIZE, COMPRESSION_THRESHOLD};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;
use walkdir::WalkDir;

/// Sync engine for managing file synchronization
pub struct SyncEngine {
    base_path: PathBuf,
    manifests: HashMap<String, Vec<FileManifest>>,
}

impl SyncEngine {
    /// Create a new sync engine for a folder
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            manifests: HashMap::new(),
        }
    }

    /// Generate a file manifest for the shared folder
    pub async fn generate_manifest(&self) -> Result<Vec<FileManifest>> {
        let mut manifests = Vec::new();

        if !self.base_path.exists() {
            return Ok(manifests);
        }

        let entries = WalkDir::new(&self.base_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file());

        for entry in entries {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&self.base_path)
                .map_err(|e| anyhow::anyhow!("Failed to get relative path: {}", e))?;

            let path_str = relative_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path"))?
                .to_string();

            // Get file metadata
            let metadata = fs::metadata(path)
                .await
                .context("Failed to read file metadata")?;

            let size = metadata.len();

            // Check file size limit
            if size > MAX_FILE_SIZE {
                log::warn!("File too large, skipping: {}", path_str);
                continue;
            }

            // Get modified time
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // Calculate file hash
            let hash = self.calculate_file_hash(path).await?;

            manifests.push(FileManifest {
                path: path_str,
                hash,
                size,
                modified,
                is_deleted: false,
            });
        }

        Ok(manifests)
    }

    /// Public method to calculate hash of a file
    pub async fn hash_file(&self, path: &Path) -> String {
        self.calculate_file_hash(path).await.unwrap_or_default()
    }

    /// Calculate SHA-256 hash of a file
    async fn calculate_file_hash(&self, path: &Path) -> Result<String> {
        let contents = fs::read(path)
            .await
            .context("Failed to read file for hashing")?;

        let mut hasher = Sha256::new();
        hasher.update(&contents);
        let result = hasher.finalize();

        Ok(hex::encode(result))
    }

    /// Compare two manifests and find differences
    pub fn compare_manifests(
        &self,
        local: &[FileManifest],
        remote: &[FileManifest],
    ) -> ManifestDiff {
        let local_map: HashMap<_, _> = local.iter().map(|m| (&m.path, m)).collect();
        let remote_map: HashMap<_, _> = remote.iter().map(|m| (&m.path, m)).collect();

        let mut to_download = Vec::new();
        let mut to_upload = Vec::new();
        let mut conflicts = Vec::new();
        let mut unchanged = Vec::new();

        // Check remote files
        for (path, remote_file) in &remote_map {
            if let Some(local_file) = local_map.get(path) {
                // File exists on both sides
                if local_file.hash == remote_file.hash {
                    // No change
                    unchanged.push(path.to_string());
                } else if local_file.modified > remote_file.modified {
                    // Local is newer
                    to_upload.push(path.to_string());
                } else if remote_file.modified > local_file.modified {
                    // Remote is newer
                    to_download.push(path.to_string());
                } else {
                    // Same timestamp but different content = conflict
                    conflicts.push(ConflictInfo {
                        path: path.to_string(),
                        local_hash: local_file.hash.clone(),
                        remote_hash: remote_file.hash.clone(),
                    });
                }
            } else {
                // File only exists remotely
                to_download.push(path.to_string());
            }
        }

        // Check local files that don't exist remotely
        for (path, local_file) in &local_map {
            if !remote_map.contains_key(path) {
                // File only exists locally
                to_upload.push(path.to_string());
            }
        }

        ManifestDiff {
            to_download,
            to_upload,
            conflicts,
            unchanged,
        }
    }

    /// Read a file in chunks
    pub async fn read_file_chunks(&self, path: &str) -> Result<Vec<Vec<u8>>> {
        let full_path = self.base_path.join(path);
        let contents = fs::read(&full_path).await?;

        Ok(contents.chunks(CHUNK_SIZE).map(|chunk| chunk.to_vec()).collect())
    }

    /// Write a file from chunks
    pub async fn write_file_chunks(&self, path: &str, chunks: Vec<Vec<u8>>) -> Result<()> {
        let full_path = self.base_path.join(path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory")?;
        }

        let mut file = fs::File::create(&full_path)
            .await
            .context("Failed to create file")?;

        use tokio::io::AsyncWriteExt;
        for chunk in &chunks {
            file.write_all(chunk)
                .await
                .context("Failed to write chunk")?;
        }

        file.flush().await.context("Failed to flush file")?;

        Ok(())
    }

    /// Delete a file
    pub async fn delete_file(&self, path: &str) -> Result<()> {
        let full_path = self.base_path.join(path);

        if full_path.exists() {
            fs::remove_file(&full_path)
                .await
                .context("Failed to delete file")?;
        }

        Ok(())
    }

    /// Get the full path for a relative path
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        self.base_path.join(path)
    }

    /// Validate that a path is within the base path (security check)
    pub fn validate_path(&self, path: &str) -> Result<()> {
        let full_path = self.resolve_path(path);

        // Check if the resolved path starts with the base path
        full_path
            .canonicalize()
            .and_then(|resolved| {
                self.base_path
                    .canonicalize()
                    .map(|base| resolved.starts_with(base))
            })
            .map_err(|_| anyhow::anyhow!("Invalid path: outside shared folder"))?
            .then_some(())
            .ok_or_else(|| anyhow::anyhow!("Path traversal detected"))
    }

    /// Compress data using zstd if it's large enough
    pub fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < COMPRESSION_THRESHOLD as usize {
            // Don't compress small files
            return Ok(data.to_vec());
        }

        let compressed = zstd::encode_all(data.as_ref(), 3)
            .map_err(|e| anyhow::anyhow!("Compression failed: {}", e))?;

        // Only use compressed data if it's actually smaller
        Ok(if compressed.len() < data.len() {
            compressed
        } else {
            data.to_vec()
        })
    }

    /// Decompress data (attempts zstd decompression)
    pub fn decompress_data(data: &[u8]) -> Result<Vec<u8>> {
        // Try to decompress as zstd
        match zstd::decode_all(data.as_ref()) {
            Ok(decompressed) => Ok(decompressed),
            Err(_) => {
                // If decompression fails, assume data wasn't compressed
                Ok(data.to_vec())
            }
        }
    }

    /// Check if data appears to be zstd compressed
    pub fn is_compressed(data: &[u8]) -> bool {
        // Zstd magic number check (simplified)
        data.len() > 3 && data[0] == 0xFD && data[1] == 0x2F && data[2] == 0xB5
    }
}

/// Difference between two manifests
#[derive(Debug, Clone)]
pub struct ManifestDiff {
    /// Files to download from remote (newer or new on remote)
    pub to_download: Vec<String>,
    /// Files to upload to remote (newer or new on local)
    pub to_upload: Vec<String>,
    /// Files with conflicts (changed on both sides)
    pub conflicts: Vec<ConflictInfo>,
    /// Files that are unchanged
    pub unchanged: Vec<String>,
}

impl ManifestDiff {
    /// Check if there are any changes
    pub fn has_changes(&self) -> bool {
        !self.to_download.is_empty()
            || !self.to_upload.is_empty()
            || !self.conflicts.is_empty()
    }

    /// Get the total number of changes
    pub fn change_count(&self) -> usize {
        self.to_download.len() + self.to_upload.len() + self.conflicts.len()
    }
}

/// Information about a file conflict
#[derive(Debug, Clone)]
pub struct ConflictInfo {
    pub path: String,
    pub local_hash: String,
    pub remote_hash: String,
}

/// Create a conflict copy filename
pub fn conflict_copy_name(path: &str) -> String {
    if let Some(ext_pos) = path.rfind('.') {
        let (name, ext) = path.split_at(ext_pos);
        format!("{} (conflict copy){}", name, ext)
    } else {
        format!("{} (conflict copy)", path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_copy_name() {
        assert_eq!(
            conflict_copy_name("test.md"),
            "test (conflict copy).md"
        );
        assert_eq!(
            conflict_copy_name("folder/note.txt"),
            "folder/note (conflict copy).txt"
        );
        assert_eq!(
            conflict_copy_name("noextension"),
            "noextension (conflict copy)"
        );
    }

    #[test]
    fn test_manifest_diff() {
        let engine = SyncEngine::new(PathBuf::from("/tmp"));

        let local = vec![
            FileManifest {
                path: "file1.md".to_string(),
                hash: "hash1".to_string(),
                size: 100,
                modified: 100,
                is_deleted: false,
            },
            FileManifest {
                path: "file2.md".to_string(),
                hash: "hash2".to_string(),
                size: 200,
                modified: 200,
                is_deleted: false,
            },
        ];

        let remote = vec![
            FileManifest {
                path: "file1.md".to_string(),
                hash: "hash1".to_string(),
                size: 100,
                modified: 100,
                is_deleted: false,
            }, // Unchanged
            FileManifest {
                path: "file2.md".to_string(),
                hash: "hash3".to_string(),
                size: 250,
                modified: 250,
                is_deleted: false,
            }, // Remote newer
            FileManifest {
                path: "file3.md".to_string(),
                hash: "hash4".to_string(),
                size: 300,
                modified: 300,
                is_deleted: false,
            }, // New on remote
        ];

        let diff = engine.compare_manifests(&local, &remote);

        assert_eq!(diff.unchanged, vec!["file1.md"]);
        assert_eq!(diff.to_download.len(), 2);
        assert!(diff.to_download.iter().any(|p| p == "file2.md"));
        assert!(diff.to_download.iter().any(|p| p == "file3.md"));
        assert!(diff.to_upload.is_empty());
        assert!(diff.conflicts.is_empty());
    }
}
