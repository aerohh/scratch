use anyhow::Result;
use base64::Engine;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl};
use tauri::webview::WebviewWindowBuilder;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::fs;
use tokio::io::AsyncWriteExt;

mod git;
mod p2p;

// Note metadata for list display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMetadata {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub modified: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliStatus {
    pub supported: bool,
    pub installed: bool,
    pub path: Option<String>,
}

// Full note content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub path: String,
    pub modified: i64,
}

// Theme color customization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemeColors {
    pub bg: Option<String>,
    pub bg_secondary: Option<String>,
    pub bg_muted: Option<String>,
    pub bg_emphasis: Option<String>,
    pub text: Option<String>,
    pub text_muted: Option<String>,
    pub text_inverse: Option<String>,
    pub border: Option<String>,
    pub accent: Option<String>,
}

// Theme settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    pub mode: String, // "light" | "dark" | "system"
    pub custom_light_colors: Option<ThemeColors>,
    pub custom_dark_colors: Option<ThemeColors>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            mode: "system".to_string(),
            custom_light_colors: None,
            custom_dark_colors: None,
        }
    }
}

// Editor font settings (simplified)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EditorFontSettings {
    pub base_font_family: Option<String>, // "system-sans" | "serif" | "monospace"
    pub base_font_size: Option<f32>,      // in px, default 16
    pub bold_weight: Option<i32>,         // 600, 700, 800 for headings and bold
    pub line_height: Option<f32>,         // default 1.6
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextDirection {
    Auto,
    Ltr,
    Rtl,
}

// App config (stored in app data directory - just the notes folder path)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub notes_folder: Option<String>,
}

// Per-folder settings (stored in .scratch/settings.json within notes folder)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub theme: ThemeSettings,
    #[serde(rename = "editorFont")]
    pub editor_font: Option<EditorFontSettings>,
    #[serde(rename = "gitEnabled")]
    pub git_enabled: Option<bool>,
    #[serde(rename = "pinnedNoteIds")]
    pub pinned_note_ids: Option<Vec<String>>,
    #[serde(rename = "textDirection")]
    pub text_direction: Option<TextDirection>,
    #[serde(rename = "editorWidth")]
    pub editor_width: Option<String>,
    #[serde(rename = "defaultNoteName")]
    pub default_note_name: Option<String>,
    #[serde(rename = "interfaceZoom")]
    pub interface_zoom: Option<f32>,
    #[serde(rename = "customEditorWidthPx")]
    pub custom_editor_width_px: Option<u32>,
    #[serde(rename = "ollamaModel")]
    pub ollama_model: Option<String>,
    #[serde(rename = "foldersEnabled")]
    pub folders_enabled: Option<bool>,
}

// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub modified: i64,
    pub score: f32,
}

// AI execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

// File watcher state
pub struct FileWatcherState {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

// Tantivy search index state
pub struct SearchIndex {
    index: Index,
    reader: IndexReader,
    writer: Mutex<IndexWriter>,
    #[allow(dead_code)]
    schema: Schema,
    id_field: Field,
    title_field: Field,
    content_field: Field,
    modified_field: Field,
}

impl SearchIndex {
    fn new(index_path: &PathBuf) -> Result<Self> {
        // Build schema
        let mut schema_builder = Schema::builder();
        let id_field = schema_builder.add_text_field("id", STRING | STORED);
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let modified_field = schema_builder.add_i64_field("modified", INDEXED | STORED);
        let schema = schema_builder.build();

        // Create or open index
        std::fs::create_dir_all(index_path)?;
        let index = Index::create_in_dir(index_path, schema.clone())
            .or_else(|_| Index::open_in_dir(index_path))?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let writer = index.writer(50_000_000)?; // 50MB buffer

        Ok(Self {
            index,
            reader,
            writer: Mutex::new(writer),
            schema,
            id_field,
            title_field,
            content_field,
            modified_field,
        })
    }

    fn index_note(&self, id: &str, title: &str, content: &str, modified: i64) -> Result<()> {
        let mut writer = self.writer.lock().expect("search writer mutex");

        // Delete existing document with this ID
        let id_term = tantivy::Term::from_field_text(self.id_field, id);
        writer.delete_term(id_term);

        // Add new document
        writer.add_document(doc!(
            self.id_field => id,
            self.title_field => title,
            self.content_field => content,
            self.modified_field => modified,
        ))?;

        writer.commit()?;
        Ok(())
    }

    fn delete_note(&self, id: &str) -> Result<()> {
        let mut writer = self.writer.lock().expect("search writer mutex");
        let id_term = tantivy::Term::from_field_text(self.id_field, id);
        writer.delete_term(id_term);
        writer.commit()?;
        Ok(())
    }

    fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let searcher = self.reader.searcher();
        let query_parser =
            QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);

        // Parse query, fall back to prefix query if parsing fails
        let query = query_parser
            .parse_query(query_str)
            .or_else(|_| query_parser.parse_query(&format!("{}*", query_str)))?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::with_capacity(top_docs.len());
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            let id = doc
                .get_first(self.id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = doc
                .get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = doc
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let modified = doc
                .get_first(self.modified_field)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let preview = generate_preview(content);

            results.push(SearchResult {
                id,
                title,
                preview,
                modified,
                score,
            });
        }

        Ok(results)
    }

    fn rebuild_index(&self, notes_folder: &PathBuf) -> Result<()> {
        let mut writer = self.writer.lock().expect("search writer mutex");
        writer.delete_all_documents()?;

        if notes_folder.exists() {
            use walkdir::WalkDir;
            for entry in WalkDir::new(notes_folder)
                .max_depth(10)
                .into_iter()
                .filter_entry(is_visible_notes_entry)
                .flatten()
            {
                let file_path = entry.path();
                if !file_path.is_file() {
                    continue;
                }
                if let Some(id) = id_from_abs_path(notes_folder, file_path) {
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        let modified = entry
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);

                        let title = extract_title(&content);

                        writer.add_document(doc!(
                            self.id_field => id.as_str(),
                            self.title_field => title,
                            self.content_field => content.as_str(),
                            self.modified_field => modified,
                        ))?;
                    }
                }
            }
        }

        writer.commit()?;
        Ok(())
    }
}

// App state with improved structure
pub struct AppState {
    pub app_config: RwLock<AppConfig>,  // notes_folder path (stored in app data)
    pub settings: RwLock<Settings>,      // per-folder settings (stored in .scratch/)
    pub notes_cache: RwLock<HashMap<String, NoteMetadata>>,
    pub file_watcher: Mutex<Option<FileWatcherState>>,
    pub search_index: Mutex<Option<SearchIndex>>,
    pub debounce_map: Arc<Mutex<HashMap<PathBuf, Instant>>>,
    pub p2p_state: Mutex<p2p::P2PState>,  // P2P network and share state
    pub p2p_network: Mutex<Option<p2p::NetworkManager>>,  // P2P network manager
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            app_config: RwLock::new(AppConfig::default()),
            settings: RwLock::new(Settings::default()),
            notes_cache: RwLock::new(HashMap::new()),
            file_watcher: Mutex::new(None),
            search_index: Mutex::new(None),
            debounce_map: Arc::new(Mutex::new(HashMap::new())),
            p2p_state: Mutex::new(p2p::P2PState::new()),
            p2p_network: Mutex::new(None),
        }
    }
}

// Utility: Sanitize filename from title
fn sanitize_filename(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .filter(|c| *c != '\u{00A0}' && *c != '\u{FEFF}')
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => c,
        })
        .collect();

    let trimmed = sanitized.trim();
    if trimmed.is_empty() || is_effectively_empty(trimmed) {
        "Untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Expands template tags in a note name template using local timezone
fn expand_note_name_template(template: &str) -> String {
    use chrono::Local;

    let mut result = template.to_string();

    // Get current time in local timezone
    let now = Local::now();

    // Timestamp tag (Unix timestamp)
    result = result.replace("{timestamp}", &now.timestamp().to_string());

    // Date tags
    result = result.replace("{date}", &now.format("%Y-%m-%d").to_string());
    result = result.replace("{year}", &now.format("%Y").to_string());
    result = result.replace("{month}", &now.format("%m").to_string());
    result = result.replace("{day}", &now.format("%d").to_string());

    // Time tags (use dash instead of colon for filename safety)
    result = result.replace("{time}", &now.format("%H-%M-%S").to_string());

    // Note: {counter} is handled in create_note function

    result
}

/// Extracts a display title from a note ID (filename)
fn extract_title_from_id(id: &str) -> String {
    // Get last path component (filename)
    let filename = id.rsplit('/').next().unwrap_or(id);

    // Convert to display title (replace dashes/underscores with spaces)
    let title = filename.replace(['-', '_'], " ");

    // Title case
    title
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// Utility: Check if a string is effectively empty
fn is_effectively_empty(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_whitespace() || c == '\u{00A0}' || c == '\u{FEFF}')
}

/// Strip YAML frontmatter (leading `---` ... `---` block) from content.
fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if trimmed.starts_with("---") {
        // Find the closing --- (skip the opening line)
        if let Some(rest) = trimmed.strip_prefix("---") {
            if let Some(end) = rest.find("\n---") {
                // Skip past closing --- and the newline after it (handle CRLF)
                let after_close = &rest[end + 4..];
                return after_close
                    .strip_prefix("\r\n")
                    .or_else(|| after_close.strip_prefix('\n'))
                    .unwrap_or(after_close);
            }
        }
    }
    content
}

// Utility: Extract title from markdown content
fn extract_title(content: &str) -> String {
    let body = strip_frontmatter(content);
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            let title = title.trim();
            if !is_effectively_empty(title) {
                return title.to_string();
            }
        }
        if !is_effectively_empty(trimmed) {
            return trimmed.chars().take(50).collect();
        }
    }
    "Untitled".to_string()
}

// Utility: Generate preview from content (strip markdown formatting)
fn generate_preview(content: &str) -> String {
    let body = strip_frontmatter(content);
    // Skip the first line (title), find first non-empty line
    for line in body.lines().skip(1) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let stripped = strip_markdown(trimmed);
            if !stripped.is_empty() {
                return stripped.chars().take(100).collect();
            }
        }
    }
    String::new()
}

// Strip common markdown formatting from text
fn strip_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // Remove heading markers (##, ###, etc.)
    let trimmed = result.trim_start();
    if trimmed.starts_with('#') {
        result = trimmed.trim_start_matches('#').trim_start().to_string();
    }

    // Remove strikethrough (~~text~~) - before other markers
    while let Some(start) = result.find("~~") {
        if let Some(end) = result[start + 2..].find("~~") {
            let inner = &result[start + 2..start + 2 + end];
            result = format!("{}{}{}", &result[..start], inner, &result[start + 4 + end..]);
        } else {
            break;
        }
    }

    // Remove bold (**text** or __text__) - before italic
    while let Some(start) = result.find("**") {
        if let Some(end) = result[start + 2..].find("**") {
            let inner = &result[start + 2..start + 2 + end];
            result = format!("{}{}{}", &result[..start], inner, &result[start + 4 + end..]);
        } else {
            break;
        }
    }
    while let Some(start) = result.find("__") {
        if let Some(end) = result[start + 2..].find("__") {
            let inner = &result[start + 2..start + 2 + end];
            result = format!("{}{}{}", &result[..start], inner, &result[start + 4 + end..]);
        } else {
            break;
        }
    }

    // Remove inline code (`code`)
    while let Some(start) = result.find('`') {
        if let Some(end) = result[start + 1..].find('`') {
            let inner = &result[start + 1..start + 1 + end];
            result = format!("{}{}{}", &result[..start], inner, &result[start + 2 + end..]);
        } else {
            break;
        }
    }

    // Remove images ![alt](url) - must come before links
    let img_re = regex::Regex::new(r"!\[([^\]]*)\]\([^)]+\)").unwrap();
    result = img_re.replace_all(&result, "$1").to_string();

    // Remove links [text](url)
    let link_re = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    result = link_re.replace_all(&result, "$1").to_string();

    // Remove italic (*text* or _text_) - simple approach after bold is removed
    // Match *text* where text doesn't contain *
    while let Some(start) = result.find('*') {
        if let Some(end) = result[start + 1..].find('*') {
            if end > 0 {
                let inner = &result[start + 1..start + 1 + end];
                result = format!("{}{}{}", &result[..start], inner, &result[start + 2 + end..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    // Match _text_ where text doesn't contain _
    while let Some(start) = result.find('_') {
        if let Some(end) = result[start + 1..].find('_') {
            if end > 0 {
                let inner = &result[start + 1..start + 1 + end];
                result = format!("{}{}{}", &result[..start], inner, &result[start + 2 + end..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // Remove task list markers
    result = result
        .replace("- [ ] ", "")
        .replace("- [x] ", "")
        .replace("- [X] ", "");

    // Remove list markers at start (-, *, +, 1.)
    let list_re = regex::Regex::new(r"^(\s*[-+*]|\s*\d+\.)\s+").unwrap();
    result = list_re.replace(&result, "").to_string();

    result.trim().to_string()
}

/// Directories to exclude from note discovery and ID resolution.
const EXCLUDED_DIRS: &[&str] = &[".git", ".scratch", ".obsidian", ".trash", "assets"];
const PROFILE_ENV_VAR: &str = "SCRATCH_PROFILE";

/// Filter for WalkDir: skips excluded directories.
fn is_visible_notes_entry(entry: &walkdir::DirEntry) -> bool {
    if entry.file_type().is_dir() {
        let name = entry.file_name().to_str().unwrap_or("");
        return !EXCLUDED_DIRS.contains(&name);
    }
    true
}

/// Convert an absolute file path to a note ID (relative path from notes root, no .md extension, POSIX separators).
/// Returns None if the path is outside the root, not a .md file, or in an excluded directory.
fn id_from_abs_path(notes_root: &Path, file_path: &Path) -> Option<String> {
    let rel = file_path.strip_prefix(notes_root).ok()?;

    // Skip files inside excluded directories (.git, .scratch, assets, etc.)
    // Only block specific known dirs so that dot-prefixed *files* like ".foo.md" are still visible.
    for component in rel.parent().unwrap_or(Path::new("")).components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_str()?;
            if EXCLUDED_DIRS.contains(&name_str) {
                return None;
            }
        }
    }

    // Must be a .md file
    if file_path.extension()?.to_str()? != "md" {
        return None;
    }

    // Build ID: relative path without .md suffix, using POSIX separators.
    // Strip .md by converting to string and trimming (avoids with_extension
    // which breaks on stems containing dots like "meeting.2024-01-15.md").
    let rel_str = rel.to_str()?;
    let id = rel_str.strip_suffix(".md")?.replace(std::path::MAIN_SEPARATOR, "/");

    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Convert a note ID to an absolute file path. Validates against path traversal.
fn abs_path_from_id(notes_root: &Path, id: &str) -> Result<PathBuf, String> {
    if id.contains('\\') {
        return Err("Invalid note ID: backslashes not allowed".to_string());
    }

    let rel = Path::new(id);

    for component in rel.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err("Invalid note ID: parent directory references not allowed".to_string());
            }
            std::path::Component::CurDir => {
                return Err("Invalid note ID: current directory references not allowed".to_string());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("Invalid note ID: absolute paths not allowed".to_string());
            }
            _ => {}
        }
    }

    // Append ".md" via OsString to avoid with_extension replacing dots in stems
    // (e.g. "meeting.2024-01-15" would become "meeting.md" with with_extension)
    let joined = notes_root.join(rel);
    let mut file_path_os = joined.into_os_string();
    file_path_os.push(".md");
    let file_path = PathBuf::from(file_path_os);

    if !file_path.starts_with(notes_root) {
        return Err("Invalid note ID: path escapes notes folder".to_string());
    }

    Ok(file_path)
}

// Get app config file path (in app data directory)
fn get_app_config_path(app: &AppHandle) -> Result<PathBuf> {
    let app_data = get_profiled_app_data_dir(app)?;
    Ok(app_data.join("config.json"))
}

// Get per-folder settings file path (in .scratch/ within notes folder)
fn get_settings_path(notes_folder: &str) -> PathBuf {
    let scratch_dir = PathBuf::from(notes_folder).join(".scratch");
    std::fs::create_dir_all(&scratch_dir).ok();
    scratch_dir.join("settings.json")
}

// Get search index path
fn get_search_index_path(app: &AppHandle) -> Result<PathBuf> {
    let app_data = get_profiled_app_data_dir(app)?;
    Ok(app_data.join("search_index"))
}

fn sanitize_profile_id(profile: &str) -> Option<String> {
    let trimmed = profile.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return None;
    }

    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }

    Some(trimmed.to_string())
}

fn extract_profile_from_args(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(value) = arg.strip_prefix("--profile=") {
            if let Some(valid) = sanitize_profile_id(value) {
                return Some(valid);
            }
        } else if arg == "--profile" {
            if let Some(next) = args.get(i + 1) {
                if let Some(valid) = sanitize_profile_id(next) {
                    return Some(valid);
                }
            }
            i += 1;
        }
        i += 1;
    }
    None
}

fn current_profile_id() -> Option<String> {
    std::env::var(PROFILE_ENV_VAR)
        .ok()
        .and_then(|value| sanitize_profile_id(&value))
}

fn get_profiled_app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    let app_data = app.path().app_data_dir()?;
    let path = if let Some(profile) = current_profile_id() {
        app_data.join("profiles").join(profile)
    } else {
        app_data
    };

    std::fs::create_dir_all(&path)?;
    Ok(path)
}

// Load app config from disk (notes folder path)
fn load_app_config(app: &AppHandle) -> AppConfig {
    let path = match get_app_config_path(app) {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };

    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

// Save app config to disk
fn save_app_config(app: &AppHandle, config: &AppConfig) -> Result<()> {
    let path = get_app_config_path(app)?;
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

// Load per-folder settings from disk
fn load_settings(notes_folder: &str) -> Settings {
    let path = get_settings_path(notes_folder);

    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    } else {
        Settings::default()
    }
}

// Save per-folder settings to disk
fn save_settings(notes_folder: &str, settings: &Settings) -> Result<()> {
    let path = get_settings_path(notes_folder);
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn get_shares_path(notes_folder: &str) -> PathBuf {
    let scratch_dir = PathBuf::from(notes_folder).join(".scratch");
    std::fs::create_dir_all(&scratch_dir).ok();
    scratch_dir.join("shares.json")
}

fn load_p2p_shares(notes_folder: &str) -> Vec<p2p::SharedFolder> {
    let path = get_shares_path(notes_folder);
    if !path.exists() {
        return Vec::new();
    }

    std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<Vec<p2p::SharedFolder>>(&content).ok())
        .unwrap_or_default()
}

fn save_p2p_shares(notes_folder: &str, shares: &[p2p::SharedFolder]) -> Result<()> {
    let path = get_shares_path(notes_folder);
    let content = serde_json::to_string_pretty(shares)?;
    std::fs::write(path, content)?;
    Ok(())
}

// Clean up old entries from debounce map (entries older than 5 seconds)
fn cleanup_debounce_map(map: &Mutex<HashMap<PathBuf, Instant>>) {
    let mut map = map.lock().expect("debounce map mutex");
    let now = Instant::now();
    map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(5));
}

// Normalize notes folder path from plain paths and legacy file:// URIs.
fn normalize_notes_folder_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("Notes folder path is empty".to_string());
    }

    if trimmed.starts_with("file://") {
        let parsed = url::Url::parse(trimmed)
            .map_err(|e| format!("Invalid file URL for notes folder: {}", e))?;
        return parsed
            .to_file_path()
            .map_err(|_| "Invalid file URL for notes folder".to_string());
    }

    Ok(PathBuf::from(trimmed))
}

/// Shared initialization logic for setting a notes folder.
/// Creates required directories, verifies write access, updates config/settings,
/// adds asset protocol scope, and rebuilds the search index.
fn initialize_notes_folder(app: &AppHandle, path_buf: &PathBuf, state: &AppState) -> Result<String, String> {
    let normalized_path = path_buf.to_string_lossy().into_owned();

    // Verify it's a valid directory
    if !path_buf.exists() {
        std::fs::create_dir_all(path_buf).map_err(|e| e.to_string())?;
    }

    // Create assets folder
    let assets = path_buf.join("assets");
    std::fs::create_dir_all(&assets).map_err(|e| e.to_string())?;

    // Create .scratch config folder
    let scratch_dir = path_buf.join(".scratch");
    std::fs::create_dir_all(&scratch_dir).map_err(|e| e.to_string())?;

    // Verify write access early to avoid later silent failures
    let write_test_path = scratch_dir.join(".write-test");
    std::fs::write(&write_test_path, b"ok")
        .map_err(|e| format!("Notes folder is not writable: {}", e))?;
    let _ = std::fs::remove_file(&write_test_path);

    // Load per-folder settings (starts fresh with defaults if none exist)
    let settings = load_settings(&normalized_path);

    // Update app config
    {
        let mut app_config = state.app_config.write().expect("app_config write lock");
        app_config.notes_folder = Some(normalized_path.clone());
    }

    // Update settings in memory
    {
        let mut current_settings = state.settings.write().expect("settings write lock");
        *current_settings = settings;
    }

    // Load persisted P2P shares for this notes folder
    {
        let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state.shares = load_p2p_shares(&normalized_path);
    }

    // Save app config to disk
    {
        let app_config = state.app_config.read().expect("app_config read lock");
        save_app_config(app, &app_config).map_err(|e| e.to_string())?;
    }

    // Add notes folder to asset protocol scope so images can be served
    let _ = app.asset_protocol_scope().allow_directory(path_buf, true);

    // Initialize search index
    if let Ok(index_path) = get_search_index_path(app) {
        if let Ok(search_index) = SearchIndex::new(&index_path) {
            let _ = search_index.rebuild_index(path_buf);
            let mut index = state.search_index.lock().expect("search index mutex");
            *index = Some(search_index);
        }
    }

    Ok(normalized_path)
}

// TAURI COMMANDS

#[tauri::command]
fn get_notes_folder(state: State<AppState>) -> Option<String> {
    state
        .app_config
        .read()
        .expect("app_config read lock")
        .notes_folder
        .clone()
}

#[tauri::command]
fn set_notes_folder(app: AppHandle, path: String, state: State<AppState>) -> Result<(), String> {
    let path_buf = normalize_notes_folder_path(&path)?;
    initialize_notes_folder(&app, &path_buf, &state)?;
    Ok(())
}

#[tauri::command]
async fn list_notes(state: State<'_, AppState>) -> Result<Vec<NoteMetadata>, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    let path = PathBuf::from(&folder);
    if !path.exists() {
        return Ok(vec![]);
    }

    let path_clone = path.clone();
    let discovered = tokio::task::spawn_blocking(move || {
        use walkdir::WalkDir;
        let mut results: Vec<(String, String, String, i64)> = Vec::new();
        for entry in WalkDir::new(&path_clone)
            .follow_links(true)
            .max_depth(10)
            .into_iter()
            .filter_entry(is_visible_notes_entry)
            .flatten()
        {
            let file_path = entry.path();
            if !file_path.is_file() {
                continue;
            }
            if let Some(id) = id_from_abs_path(&path_clone, file_path) {
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    let modified = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    let title = extract_title(&content);
                    let preview = generate_preview(&content);
                    results.push((id, title, preview, modified));
                }
            }
        }
        results
    })
    .await
    .map_err(|e| e.to_string())?;

    let mut notes: Vec<NoteMetadata> = discovered
        .into_iter()
        .map(|(id, title, preview, modified)| NoteMetadata {
            id,
            title,
            preview,
            modified,
        })
        .collect();

    // Load pinned note IDs from settings
    let pinned_ids: HashSet<String> = {
        let settings = state.settings.read().expect("settings read lock");
        settings
            .pinned_note_ids
            .as_ref()
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default()
    };

    // Sort: pinned notes first (by date), then unpinned notes (by date)
    notes.sort_by(|a, b| {
        let a_pinned = pinned_ids.contains(&a.id);
        let b_pinned = pinned_ids.contains(&b.id);

        match (a_pinned, b_pinned) {
            (true, false) => std::cmp::Ordering::Less,    // a pinned, b not -> a first
            (false, true) => std::cmp::Ordering::Greater, // b pinned, a not -> b first
            _ => b.modified.cmp(&a.modified),             // both same status -> sort by date (newest first)
        }
    });

    // Update cache efficiently
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        cache.clear();
        for note in &notes {
            cache.insert(note.id.clone(), note.clone());
        }
    }

    Ok(notes)
}

#[tauri::command]
async fn read_note(id: String, state: State<'_, AppState>) -> Result<Note, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    let folder_path = PathBuf::from(&folder);
    let file_path = abs_path_from_id(&folder_path, &id)?;
    if !file_path.exists() {
        return Err("Note not found".to_string());
    }

    let content = fs::read_to_string(&file_path)
        .await
        .map_err(|e| e.to_string())?;
    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|e| e.to_string())?;

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    Ok(Note {
        id,
        title: extract_title(&content),
        content,
        path: file_path.to_string_lossy().into_owned(),
        modified,
    })
}

#[tauri::command]
async fn save_note(
    id: Option<String>,
    content: String,
    state: State<'_, AppState>,
) -> Result<Note, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };
    let folder_path = PathBuf::from(&folder);

    let title = extract_title(&content);
    let sanitized_leaf = sanitize_filename(&title);

    // Determine the file ID and path, handling renames
    let (final_id, file_path, old_id) = if let Some(existing_id) = id {
        // Preserve directory prefix for notes in subfolders
        let (dir_prefix, desired_id) = if let Some(pos) = existing_id.rfind('/') {
            let prefix = &existing_id[..pos];
            (Some(prefix.to_string()), format!("{}/{}", prefix, sanitized_leaf))
        } else {
            (None, sanitized_leaf.clone())
        };

        let old_file_path = abs_path_from_id(&folder_path, &existing_id)?;

        if existing_id != desired_id {
            let mut new_id = desired_id.clone();
            let mut counter = 1;

            while new_id != existing_id
                && abs_path_from_id(&folder_path, &new_id)
                    .map(|p| p.exists())
                    .unwrap_or(false)
            {
                new_id = if let Some(ref prefix) = dir_prefix {
                    format!("{}/{}-{}", prefix, sanitized_leaf, counter)
                } else {
                    format!("{}-{}", sanitized_leaf, counter)
                };
                counter += 1;
            }

            let new_file_path = abs_path_from_id(&folder_path, &new_id)?;
            (new_id, new_file_path, Some((existing_id, old_file_path)))
        } else {
            (existing_id, old_file_path, None)
        }
    } else {
        // New notes go in root
        let mut new_id = sanitized_leaf.clone();
        let mut counter = 1;

        while abs_path_from_id(&folder_path, &new_id)
            .map(|p| p.exists())
            .unwrap_or(false)
        {
            new_id = format!("{}-{}", sanitized_leaf, counter);
            counter += 1;
        }

        let new_file_path = abs_path_from_id(&folder_path, &new_id)?;
        (new_id, new_file_path, None)
    };

    // Write the file to the new path
    fs::write(&file_path, &content)
        .await
        .map_err(|e| e.to_string())?;

    // Delete old file AFTER successful write (to prevent data loss)
    if let Some((_, ref old_file_path)) = old_id {
        if old_file_path.exists() && *old_file_path != file_path {
            let _ = fs::remove_file(old_file_path).await;
        }
    }

    let metadata = fs::metadata(&file_path)
        .await
        .map_err(|e| e.to_string())?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Update search index (delete old entry if renamed, then add new)
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            if let Some((ref old_id_str, _)) = old_id {
                let _ = search_index.delete_note(old_id_str);
            }
            let _ = search_index.index_note(&final_id, &title, &content, modified);
        }
    }

    // Update cache (remove old entry if renamed)
    if let Some((ref old_id_str, _)) = old_id {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        cache.remove(old_id_str);
    }

    if let Some((ref old_id_str, _)) = old_id {
        if old_id_str != &final_id {
            notify_p2p_note_change(&state, old_id_str, true);
        }
    }
    notify_p2p_note_change(&state, &final_id, false);

    Ok(Note {
        id: final_id,
        title,
        content,
        path: file_path.to_string_lossy().into_owned(),
        modified,
    })
}

#[tauri::command]
async fn delete_note(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    let folder_path = PathBuf::from(&folder);
    let file_path = abs_path_from_id(&folder_path, &id)?;
    if file_path.exists() {
        fs::remove_file(&file_path)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Update search index
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let _ = search_index.delete_note(&id);
        }
    }

    // Remove from cache
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        cache.remove(&id);
    }

    notify_p2p_note_change(&state, &id, true);

    Ok(())
}

#[tauri::command]
async fn create_note(target_folder: Option<String>, state: State<'_, AppState>) -> Result<Note, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };
    let folder_path = PathBuf::from(&folder);

    // Get template from settings (default "Untitled")
    let template = {
        let settings = state.settings.read().expect("settings read lock");
        settings
            .default_note_name
            .clone()
            .unwrap_or_else(|| "Untitled".to_string())
    };

    // Expand template tags
    let expanded = expand_note_name_template(&template);

    // Sanitize filename
    let sanitized = sanitize_filename(&expanded);

    // Prepend folder prefix if specified
    let sanitized = if let Some(ref folder_prefix) = target_folder {
        if folder_prefix.is_empty() {
            sanitized
        } else {
            format!("{}/{}", folder_prefix.trim_end_matches('/'), sanitized)
        }
    } else {
        sanitized
    };

    // Handle {counter} tag
    let has_counter = template.contains("{counter}");
    let base_id = if has_counter {
        sanitized.replace("{counter}", "1")
    } else {
        sanitized.clone()
    };

    let mut final_id = base_id.clone();
    let mut counter = if has_counter { 2 } else { 1 };

    // Ensure filename uniqueness
    while abs_path_from_id(&folder_path, &final_id)
        .map(|p| p.exists())
        .unwrap_or(false)
    {
        if has_counter {
            final_id = sanitized.replace("{counter}", &counter.to_string());
        } else {
            final_id = format!("{}-{}", base_id, counter);
        }
        counter += 1;
    }

    // Extract display title from filename
    let display_title = extract_title_from_id(&final_id);

    let content = format!("# {}\n\n", display_title);
    let file_path = abs_path_from_id(&folder_path, &final_id)?;

    // Create parent directories (for templates like {year}/{month}/{day})
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }

    fs::write(&file_path, &content)
        .await
        .map_err(|e| e.to_string())?;

    let modified = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Update search index
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let _ = search_index.index_note(&final_id, &display_title, &content, modified);
        }
    }

    notify_p2p_note_change(&state, &final_id, false);

    Ok(Note {
        id: final_id,
        title: display_title,
        content,
        path: file_path.to_string_lossy().into_owned(),
        modified,
    })
}

/// Validate a relative folder path against traversal attacks
const RESERVED_FOLDER_NAMES: &[&str] = &[".git", ".scratch", ".obsidian", ".trash", "assets"];

fn validate_folder_path(path: &str) -> Result<(), String> {
    if path.contains('\\') {
        return Err("Invalid path: backslashes not allowed".to_string());
    }
    if path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    let rel = Path::new(path);
    for component in rel.components() {
        match component {
            std::path::Component::ParentDir => {
                return Err("Path traversal not allowed".to_string());
            }
            std::path::Component::CurDir => {
                return Err("Invalid path: current directory references not allowed".to_string());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("Invalid path: absolute paths not allowed".to_string());
            }
            std::path::Component::Normal(name) => {
                if let Some(name_str) = name.to_str() {
                    if RESERVED_FOLDER_NAMES.contains(&name_str) {
                        return Err(format!("'{}' is a reserved folder name", name_str));
                    }
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn list_folders(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };
    let folder_path = PathBuf::from(&folder);

    let fp = folder_path.clone();
    tokio::task::spawn_blocking(move || {
        let mut folders = Vec::new();
        use walkdir::WalkDir;
        for entry in WalkDir::new(&fp)
            .follow_links(true)
            .max_depth(10)
            .into_iter()
            .filter_entry(is_visible_notes_entry)
            .flatten()
        {
            if entry.file_type().is_dir() && entry.path() != fp {
                if let Ok(rel) = entry.path().strip_prefix(&fp) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if !rel_str.is_empty() {
                        folders.push(rel_str);
                    }
                }
            }
        }
        folders.sort();
        folders
    })
    .await
    .map_err(|e| format!("Failed to list folders: {}", e))
}

#[tauri::command]
async fn create_folder(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    validate_folder_path(&path)?;

    let target = PathBuf::from(&folder).join(path.replace('/', std::path::MAIN_SEPARATOR_STR));

    if !target.starts_with(&folder) {
        return Err("Invalid path: escapes notes folder".to_string());
    }

    fs::create_dir_all(&target)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn delete_folder(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    validate_folder_path(&path)?;

    let target = PathBuf::from(&folder).join(path.replace('/', std::path::MAIN_SEPARATOR_STR));

    if !target.starts_with(&folder) {
        return Err("Invalid path: escapes notes folder".to_string());
    }

    if !target.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    // Remove notes from search index
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let cache = state.notes_cache.read().expect("cache read lock");
            let prefix = format!("{}/", path);
            for note_id in cache.keys() {
                if note_id.starts_with(&prefix) {
                    let _ = search_index.delete_note(note_id);
                }
            }
        }
    }

    // Remove notes from cache
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        let prefix = format!("{}/", path);
        cache.retain(|id, _| !id.starts_with(&prefix));
    }

    fs::remove_dir_all(&target)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn rename_folder(
    old_path: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    validate_folder_path(&old_path)?;

    // Sanitize new name (no slashes allowed in the name itself)
    let sanitized_name = new_name
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
        .trim()
        .to_string();
    if sanitized_name.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }

    let folder_root = PathBuf::from(&folder);
    let old_target = folder_root.join(old_path.replace('/', std::path::MAIN_SEPARATOR_STR));

    if !old_target.starts_with(&folder_root) {
        return Err("Invalid path: escapes notes folder".to_string());
    }
    if !old_target.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    // Build new path: same parent, new name
    let new_target = old_target
        .parent()
        .ok_or("Cannot determine parent directory")?
        .join(&sanitized_name);

    if new_target.exists() {
        return Err("A folder with that name already exists".to_string());
    }

    // Compute old and new path prefixes for updating IDs
    let old_prefix = format!("{}/", old_path);
    let new_path = if old_path.contains('/') {
        let parent = &old_path[..old_path.rfind('/').unwrap()];
        format!("{}/{}", parent, sanitized_name)
    } else {
        sanitized_name.clone()
    };
    let new_prefix = format!("{}/", new_path);

    // Rename on disk
    tokio::fs::rename(&old_target, &new_target)
        .await
        .map_err(|e| e.to_string())?;

    // Update pinned note IDs in settings
    {
        let mut settings = state.settings.write().expect("settings write lock");
        if let Some(ref mut pinned) = settings.pinned_note_ids {
            for id in pinned.iter_mut() {
                if id.starts_with(&old_prefix) {
                    *id = format!("{}{}", new_prefix, &id[old_prefix.len()..]);
                } else if *id == old_path {
                    *id = new_path.clone();
                }
            }
        }
        // Save settings
        let _ = save_settings(&folder, &settings);
    }

    // Update cache
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        let updates: Vec<(String, String)> = cache
            .keys()
            .filter(|id| id.starts_with(&old_prefix))
            .map(|id| {
                let new_id = format!("{}{}", new_prefix, &id[old_prefix.len()..]);
                (id.clone(), new_id)
            })
            .collect();
        for (old_id, new_id) in updates {
            if let Some(mut meta) = cache.remove(&old_id) {
                meta.id = new_id.clone();
                cache.insert(new_id, meta);
            }
        }
    }

    // Rebuild search index for affected notes
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let _ = search_index.rebuild_index(&folder_root);
        }
    }

    Ok(())
}

#[tauri::command]
async fn move_note(
    id: String,
    target_folder: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };
    let folder_root = PathBuf::from(&folder);
    let source_path = abs_path_from_id(&folder_root, &id)?;

    if !source_path.exists() {
        return Err("Note not found".to_string());
    }

    // Extract the filename (leaf) from the note ID
    let leaf = id.rsplit('/').next().unwrap_or(&id);

    // Build new ID
    let new_id = if target_folder.is_empty() {
        leaf.to_string()
    } else {
        validate_folder_path(&target_folder)?;
        format!("{}/{}", target_folder, leaf)
    };

    if new_id == id {
        return Ok(id);
    }

    let dest_path = abs_path_from_id(&folder_root, &new_id)?;

    // Ensure target directory exists
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }

    // Handle collision
    if dest_path.exists() {
        return Err("A note with that name already exists in the target folder".to_string());
    }

    tokio::fs::rename(&source_path, &dest_path)
        .await
        .map_err(|e| e.to_string())?;

    // Update pinned note IDs
    {
        let mut settings = state.settings.write().expect("settings write lock");
        if let Some(ref mut pinned) = settings.pinned_note_ids {
            for pin_id in pinned.iter_mut() {
                if *pin_id == id {
                    *pin_id = new_id.clone();
                }
            }
        }
        let _ = save_settings(&folder, &settings);
    }

    // Update cache
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        if let Some(mut meta) = cache.remove(&id) {
            meta.id = new_id.clone();
            cache.insert(new_id.clone(), meta);
        }
    }

    // Rebuild search index
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let _ = search_index.rebuild_index(&folder_root);
        }
    }

    notify_p2p_note_change(&state, &id, true);
    notify_p2p_note_change(&state, &new_id, false);

    Ok(new_id)
}

#[tauri::command]
async fn move_folder(
    path: String,
    target_parent: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    validate_folder_path(&path)?;
    if !target_parent.is_empty() {
        validate_folder_path(&target_parent)?;
    }

    let folder_root = PathBuf::from(&folder);
    let source = folder_root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));

    if !source.is_dir() {
        return Err("Source is not a directory".to_string());
    }

    // Get folder name
    let name = source
        .file_name()
        .ok_or("Cannot determine folder name")?
        .to_string_lossy()
        .to_string();

    let dest = if target_parent.is_empty() {
        folder_root.join(&name)
    } else {
        folder_root
            .join(target_parent.replace('/', std::path::MAIN_SEPARATOR_STR))
            .join(&name)
    };

    // Prevent moving into itself
    if dest.starts_with(&source) {
        return Err("Cannot move a folder into itself".to_string());
    }

    if dest.exists() {
        return Err("A folder with that name already exists in the target".to_string());
    }

    // Ensure target parent exists
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }

    // Compute old and new path prefixes for updating IDs
    let old_prefix = format!("{}/", path);
    let new_path = if target_parent.is_empty() {
        name.clone()
    } else {
        format!("{}/{}", target_parent, name)
    };
    let new_prefix = format!("{}/", new_path);

    tokio::fs::rename(&source, &dest)
        .await
        .map_err(|e| e.to_string())?;

    // Update pinned note IDs
    {
        let mut settings = state.settings.write().expect("settings write lock");
        if let Some(ref mut pinned) = settings.pinned_note_ids {
            for pin_id in pinned.iter_mut() {
                if pin_id.starts_with(&old_prefix) {
                    *pin_id = format!("{}{}", new_prefix, &pin_id[old_prefix.len()..]);
                }
            }
        }
        let _ = save_settings(&folder, &settings);
    }

    // Update cache
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        let updates: Vec<(String, String)> = cache
            .keys()
            .filter(|id| id.starts_with(&old_prefix))
            .map(|id| {
                let new_id = format!("{}{}", new_prefix, &id[old_prefix.len()..]);
                (id.clone(), new_id)
            })
            .collect();
        for (old_id, new_id) in updates {
            if let Some(mut meta) = cache.remove(&old_id) {
                meta.id = new_id.clone();
                cache.insert(new_id, meta);
            }
        }
    }

    // Rebuild search index
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let _ = search_index.rebuild_index(&folder_root);
        }
    }

    Ok(())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.read().expect("settings read lock").clone()
}

#[tauri::command]
fn update_settings(
    new_settings: Settings,
    state: State<AppState>,
) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone().ok_or("Notes folder not set")?
    };

    {
        let mut settings = state.settings.write().expect("settings write lock");
        *settings = new_settings;
    }

    let settings = state.settings.read().expect("settings read lock");
    save_settings(&folder, &settings).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn update_git_enabled(
    enabled: Option<bool>,
    expected_folder: String,
    state: State<AppState>,
) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        let folder = app_config.notes_folder.clone().ok_or("Notes folder not set")?;

        if folder != expected_folder {
            return Err("Notes folder changed".to_string());
        }

        folder
    };

    {
        let mut settings = state.settings.write().expect("settings write lock");
        settings.git_enabled = enabled;
    }

    let settings = state.settings.read().expect("settings read lock");
    save_settings(&folder, &settings).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn write_file(path: String, contents: Vec<u8>) -> Result<(), String> {
    fs::write(&path, contents)
        .await
        .map_err(|_| "Failed to write file".to_string())
}

#[tauri::command]
fn preview_note_name(template: String) -> Result<String, String> {
    let expanded = expand_note_name_template(&template);
    let sanitized = sanitize_filename(&expanded);

    // Show first note name (with counter as 1 if present)
    let preview = if template.contains("{counter}") {
        sanitized.replace("{counter}", "1")
    } else {
        sanitized
    };

    Ok(preview)
}

// Preview mode: file content returned by read_file_direct / save_file_direct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub title: String,
    pub modified: i64,
}

/// Validate a file path for preview mode direct file operations.
/// Ensures the path is a markdown file and resolves symlinks.
fn validate_preview_path(path: &str) -> Result<PathBuf, String> {
    let file_path = PathBuf::from(path);

    // Must have a markdown extension
    match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown") => {}
        _ => return Err("Only .md and .markdown files are allowed".to_string()),
    }

    // Resolve symlinks to get the real path
    let canonical = file_path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve file path: {}", e))?;

    Ok(canonical)
}

#[tauri::command]
async fn read_file_direct(path: String) -> Result<FileContent, String> {
    let canonical = validate_preview_path(&path)?;

    if !canonical.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let content = fs::read_to_string(&canonical)
        .await
        .map_err(|_| "Failed to read file".to_string())?;
    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|_| "Failed to read metadata".to_string())?;

    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let title = extract_title(&content);

    Ok(FileContent {
        path,
        content,
        title,
        modified,
    })
}

#[tauri::command]
async fn save_file_direct(path: String, content: String) -> Result<FileContent, String> {
    // For save, the file must already exist (we validate extension + path security)
    let canonical = validate_preview_path(&path)?;

    if !canonical.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    fs::write(&canonical, &content)
        .await
        .map_err(|_| "Failed to write file".to_string())?;

    let metadata = fs::metadata(&canonical)
        .await
        .map_err(|_| "Failed to read metadata".to_string())?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let title = extract_title(&content);

    Ok(FileContent {
        path,
        content,
        title,
        modified,
    })
}

#[tauri::command]
async fn import_file_to_folder(
    app: AppHandle,
    path: String,
    state: State<'_, AppState>,
) -> Result<NoteMetadata, String> {
    let source = validate_preview_path(&path)?;
    if !source.is_file() {
        return Err(format!("Not a file: {}", path));
    }

    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };
    let folder_path = PathBuf::from(&folder);

    // Read the source file content
    let content = fs::read_to_string(&source)
        .await
        .map_err(|_| "Failed to read source file".to_string())?;

    // Derive the note ID from the title (H1 heading), falling back to filename
    let extracted_title = extract_title(&content);
    let base_name = if extracted_title.trim().is_empty() {
        source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    } else {
        extracted_title.trim().to_string()
    };
    let base_id = sanitize_filename(&base_name);

    // Atomically create the file and write content via the handle
    let mut final_id = base_id.clone();
    let mut counter = 1;
    loop {
        let candidate = abs_path_from_id(&folder_path, &final_id)?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
            .await
        {
            Ok(mut file) => {
                if file.write_all(content.as_bytes()).await.is_err() {
                    // Clean up the empty file on write failure
                    let _ = fs::remove_file(&candidate).await;
                    return Err("Failed to write file".to_string());
                }
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                final_id = format!("{}-{}", base_id, counter);
                counter += 1;
            }
            Err(_) => return Err("Failed to create file".to_string()),
        }
    };

    let modified = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // Update search index
    {
        let index = state.search_index.lock().expect("search index mutex");
        if let Some(ref search_index) = *index {
            let _ = search_index.index_note(&final_id, &extracted_title, &content, modified);
        }
    }

    let preview = content
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");

    let metadata = NoteMetadata {
        id: final_id,
        title: extracted_title,
        preview,
        modified,
    };

    // Update notes cache so fallback search sees the imported note immediately
    {
        let mut cache = state.notes_cache.write().expect("cache write lock");
        cache.insert(metadata.id.clone(), metadata.clone());
    }

    // Tell the main window to select the imported note and focus it
    let _ = app.emit_to("main", "select-note", &metadata.id);
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.set_focus();
    }

    Ok(metadata)
}

#[tauri::command]
async fn search_notes(query: String, state: State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let trimmed_query = query.trim().to_string();
    if trimmed_query.is_empty() {
        return Ok(vec![]);
    }

    // Check if search index is available and use it (scoped to drop lock before await)
    let indexed_result = {
        let index = state.search_index.lock().expect("search index mutex");
        (*index).as_ref().map(|search_index| {
            search_index.search(&trimmed_query, 20).map_err(|e| e.to_string())
        })
    };

    match indexed_result {
        Some(Ok(results)) if !results.is_empty() => Ok(results),
        Some(Ok(_)) => {
            // Tantivy can miss partial/fuzzy matches; fall back to substring search.
            fallback_search(&trimmed_query, &state).await
        }
        Some(Err(e)) => {
            eprintln!("Tantivy search error, falling back to substring search: {}", e);
            fallback_search(&trimmed_query, &state).await
        }
        None => {
            // Fallback to simple search if index not available
            fallback_search(&trimmed_query, &state).await
        }
    }
}

// Fallback search when Tantivy index isn't available - searches title and full content
async fn fallback_search(query: &str, state: &State<'_, AppState>) -> Result<Vec<SearchResult>, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    let folder = match folder {
        Some(f) => f,
        None => return Ok(vec![]),
    };

    // Collect cache data upfront to avoid holding lock during async operations
    let cache_data: Vec<(String, String, String, i64)> = {
        let cache = state.notes_cache.read().expect("cache read lock");
        cache
            .values()
            .map(|note| {
                (
                    note.id.clone(),
                    note.title.clone(),
                    note.preview.clone(),
                    note.modified,
                )
            })
            .collect()
    };

    let folder_path = PathBuf::from(&folder);
    let query_lower = query.to_lowercase();
    let mut results: Vec<SearchResult> = Vec::new();

    for (id, title, preview, modified) in cache_data {
        let title_lower = title.to_lowercase();

        let mut score = 0.0f32;
        if title_lower.contains(&query_lower) {
            score += 50.0;
        }

        // Read file content asynchronously and search in it
        let file_path = match abs_path_from_id(&folder_path, &id) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if let Ok(content) = tokio::fs::read_to_string(&file_path).await {
            let content_lower = content.to_lowercase();
            if content_lower.contains(&query_lower) {
                // Higher score if in title, lower if only in content
                if score == 0.0 {
                    score += 10.0;
                } else {
                    score += 5.0;
                }
            }
        }

        if score > 0.0 {
            results.push(SearchResult {
                id,
                title,
                preview,
                modified,
                score,
            });
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(20);

    Ok(results)
}

// File watcher event payload
#[derive(Clone, Serialize)]
struct FileChangeEvent {
    kind: String,
    path: String,
    changed_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct SymlinkWatchAlias {
    target_root: PathBuf,
    alias_relative: PathBuf,
}

fn collect_symlink_watch_aliases(notes_root: &Path) -> Vec<SymlinkWatchAlias> {
    use walkdir::WalkDir;

    let mut aliases = Vec::new();
    let mut seen = HashSet::new();

    for entry in WalkDir::new(notes_root)
        .max_depth(10)
        .follow_links(false)
        .into_iter()
        .filter_entry(is_visible_notes_entry)
        .flatten()
    {
        if !entry.path_is_symlink() {
            continue;
        }

        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let target_root = match entry_path.canonicalize() {
            Ok(path) => path,
            Err(_) => continue,
        };

        let alias_relative = match entry_path.strip_prefix(notes_root) {
            Ok(path) if !path.as_os_str().is_empty() => path.to_path_buf(),
            _ => continue,
        };

        if seen.insert(target_root.clone()) {
            aliases.push(SymlinkWatchAlias {
                target_root,
                alias_relative,
            });
        }
    }

    aliases
}

fn id_from_event_path(
    notes_root: &Path,
    event_path: &Path,
    aliases: &[SymlinkWatchAlias],
) -> Option<String> {
    if let Some(id) = id_from_abs_path(notes_root, event_path) {
        return Some(id);
    }

    let canonical_event = event_path.canonicalize().ok();

    for alias in aliases {
        let rel = if let Ok(r) = event_path.strip_prefix(&alias.target_root) {
            Some(r.to_path_buf())
        } else if let Some(canonical) = canonical_event.as_ref() {
            canonical
                .strip_prefix(&alias.target_root)
                .ok()
                .map(Path::to_path_buf)
        } else {
            None
        };

        let rel = match rel {
            Some(r) => r,
            None => continue,
        };

        let virtual_path = notes_root.join(&alias.alias_relative).join(rel);
        if let Some(id) = id_from_abs_path(notes_root, &virtual_path) {
            return Some(id);
        }
    }

    None
}

fn setup_file_watcher(
    app: AppHandle,
    notes_folder: &str,
    debounce_map: Arc<Mutex<HashMap<PathBuf, Instant>>>,
) -> Result<FileWatcherState, String> {
    let folder_path = PathBuf::from(notes_folder);
    let notes_root = folder_path.clone();
    let app_handle = app.clone();
    let symlink_aliases = collect_symlink_watch_aliases(&notes_root);
    let symlink_aliases_for_cb = symlink_aliases.clone();

    let watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths.iter() {
                    let note_id = match id_from_event_path(&notes_root, path, &symlink_aliases_for_cb) {
                        Some(id) => id,
                        None => continue,
                    };

                    // Debounce with cleanup
                    {
                        let mut map = debounce_map.lock().expect("debounce map mutex");
                        let now = Instant::now();

                        if map.len() > 100 {
                            map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(5));
                        }

                        if let Some(last) = map.get(path) {
                            if now.duration_since(*last) < Duration::from_millis(500) {
                                continue;
                            }
                        }
                        map.insert(path.clone(), now);
                    }

                    let kind = match event.kind {
                        notify::EventKind::Create(_) => "created",
                        notify::EventKind::Modify(_) => "modified",
                        notify::EventKind::Remove(_) => "deleted",
                        // Some backends emit Any for renames or unclassified changes
                        notify::EventKind::Any => "modified",
                        _ => continue,
                    };

                    // Update search index for external file changes
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        let index = state.search_index.lock().expect("search index mutex");
                        if let Some(ref search_index) = *index {
                            match kind {
                                "created" | "modified" => {
                                    match std::fs::read_to_string(path) {
                                        Ok(content) => {
                                            let title = extract_title(&content);
                                            let modified = std::fs::metadata(path)
                                                .ok()
                                                .and_then(|m| m.modified().ok())
                                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                                .map(|d| d.as_secs() as i64)
                                                .unwrap_or(0);
                                            let _ = search_index.index_note(&note_id, &title, &content, modified);
                                        }
                                        Err(_) => {
                                            // File gone between event and read — treat as deletion
                                            if !path.exists() {
                                                let _ = search_index.delete_note(&note_id);
                                            }
                                        }
                                    }
                                }
                                "deleted" => {
                                    let _ = search_index.delete_note(&note_id);
                                }
                                _ => {}
                            }
                        }
                    }

                    // Determine the actual kind for the frontend event
                    // (a "modified" event on a non-existent file is really a delete)
                    let effective_kind = if kind == "modified" && !path.exists() {
                        "deleted"
                    } else {
                        kind
                    };

                    let _ = app_handle.emit(
                        "file-change",
                        FileChangeEvent {
                            kind: effective_kind.to_string(),
                            path: path.to_string_lossy().into_owned(),
                            changed_ids: vec![note_id.clone()],
                        },
                    );
                }
            }
        },
        Config::default(),
    )
    .map_err(|e| e.to_string())?;

    let mut watcher = watcher;

    // Watch the notes folder recursively for .md files in subfolders
    watcher
        .watch(&folder_path, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    // Also watch symlink targets directly so edits/creates in shared linked folders
    // trigger immediate file-change events.
    for alias in symlink_aliases {
        if alias.target_root.starts_with(&folder_path) {
            continue;
        }
        if let Err(err) = watcher.watch(&alias.target_root, RecursiveMode::Recursive) {
            log::warn!(
                "Failed to watch symlink target '{}' (alias '{}'): {}",
                alias.target_root.display(),
                alias.alias_relative.display(),
                err
            );
        }
    }

    Ok(FileWatcherState { watcher })
}

#[tauri::command]
fn start_file_watcher(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    // Clean up debounce map before starting
    cleanup_debounce_map(&state.debounce_map);

    let watcher_state = setup_file_watcher(
        app,
        &folder,
        Arc::clone(&state.debounce_map),
    )?;

    let mut file_watcher = state.file_watcher.lock().expect("file watcher mutex");
    *file_watcher = Some(watcher_state);

    Ok(())
}

#[tauri::command]
fn copy_to_clipboard(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_clipboard_image(
    base64_data: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Guard against empty clipboard payload
    if base64_data.trim().is_empty() {
        return Err("Clipboard data is empty".to_string());
    }

    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    // Decode base64
    let image_data = base64::engine::general_purpose::STANDARD
        .decode(&base64_data)
        .map_err(|_| "Failed to decode base64 image data".to_string())?;

    // Guard against zero-byte files
    if image_data.is_empty() {
        return Err("Decoded image data is empty".to_string());
    }

    // Create assets folder path
    let assets_dir = PathBuf::from(&folder).join("assets");
    fs::create_dir_all(&assets_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Generate unique filename with timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut target_name = format!("screenshot-{}.png", timestamp);
    let mut counter = 1;
    let mut target_path = assets_dir.join(&target_name);

    while target_path.exists() {
        target_name = format!("screenshot-{}-{}.png", timestamp, counter);
        target_path = assets_dir.join(&target_name);
        counter += 1;
    }

    // Write the file
    fs::write(&target_path, &image_data)
        .await
        .map_err(|_| "Failed to write image".to_string())?;

    // Return relative path
    Ok(format!("assets/{}", target_name))
}

#[tauri::command]
async fn copy_image_to_assets(
    source_path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    let source = PathBuf::from(&source_path);
    if !source.exists() {
        return Err("Source image file does not exist".to_string());
    }

    // Get file extension
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .ok_or("Invalid file extension")?;

    const ALLOWED_IMAGE_EXTENSIONS: &[&str] = &[
        "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "tiff", "tif", "ico", "avif",
    ];
    let ext_lower = extension.to_lowercase();
    if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
        return Err("Only image files can be copied to assets".to_string());
    }

    // Get original filename (without extension)
    let original_name = source
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("image");

    // Sanitize the filename
    let sanitized_name = sanitize_filename(original_name);

    // Create assets folder path
    let assets_dir = PathBuf::from(&folder).join("assets");
    fs::create_dir_all(&assets_dir)
        .await
        .map_err(|e| e.to_string())?;

    // Generate unique filename
    let mut target_name = format!("{}.{}", sanitized_name, extension);
    let mut counter = 1;
    let mut target_path = assets_dir.join(&target_name);

    while target_path.exists() {
        target_name = format!("{}-{}.{}", sanitized_name, counter, extension);
        target_path = assets_dir.join(&target_name);
        counter += 1;
    }

    // Copy the file
    fs::copy(&source, &target_path)
        .await
        .map_err(|_| "Failed to copy image".to_string())?;

    // Return both relative path and filename for frontend to construct the URL
    Ok(format!("assets/{}", target_name))
}

#[tauri::command]
fn rebuild_search_index(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    let index_path = get_search_index_path(&app).map_err(|e| e.to_string())?;

    // Create new index
    let search_index = SearchIndex::new(&index_path).map_err(|e| e.to_string())?;
    search_index
        .rebuild_index(&PathBuf::from(&folder))
        .map_err(|e| e.to_string())?;

    let mut index = state.search_index.lock().expect("search index mutex");
    *index = Some(search_index);

    Ok(())
}

// UI helper commands - wrap Tauri plugins for consistent invoke-based API

#[tauri::command]
async fn open_folder_dialog(
    app: AppHandle,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Run blocking dialog on a separate thread to avoid blocking the async runtime
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = app.dialog().file().set_can_create_directories(true);

        if let Some(path) = default_path {
            builder = builder.set_directory(path);
        }

        builder.blocking_pick_folder()
    })
    .await
    .map_err(|e| format!("Dialog task failed: {}", e))?;

    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
async fn open_in_file_manager(path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() || !path_buf.is_dir() {
        return Err("Path does not exist or is not a directory".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        let windows_path = path.replace("/", "\\");
        std::process::Command::new("explorer")
            .arg(&windows_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return Err("Unsupported platform".to_string());
    }

    Ok(())
}

#[tauri::command]
async fn open_url_safe(url: String) -> Result<(), String> {
    // Validate URL scheme - only allow http, https, mailto
    let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {}", e))?;

    match parsed.scheme() {
        "http" | "https" | "mailto" => {}
        scheme => {
            return Err(format!(
                "URL scheme '{}' is not allowed. Only http, https, and mailto are permitted.",
                scheme
            ))
        }
    }

    // Use system opener
    open::that(&url).map_err(|e| format!("Failed to open URL: {}", e))
}

// Git commands - run blocking git operations off the main thread

#[tauri::command]
async fn git_is_available() -> bool {
    tauri::async_runtime::spawn_blocking(git::is_available)
        .await
        .unwrap_or(false)
}

#[tauri::command]
async fn git_get_status(state: State<'_, AppState>) -> Result<git::GitStatus, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                git::get_status(&PathBuf::from(path))
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitStatus::default()),
    }
}

#[tauri::command]
async fn git_init_repo(state: State<'_, AppState>) -> Result<(), String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone().ok_or("Notes folder not set")?
    };

    tauri::async_runtime::spawn_blocking(move || {
        git::git_init(&PathBuf::from(folder))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn git_commit(message: String, state: State<'_, AppState>) -> Result<git::GitResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                git::commit_all(&PathBuf::from(path), &message)
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitResult {
            success: false,
            message: None,
            error: Some("Notes folder not set".to_string()),
        }),
    }
}

#[tauri::command]
async fn git_push(state: State<'_, AppState>) -> Result<git::GitResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                git::push(&PathBuf::from(path))
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitResult {
            success: false,
            message: None,
            error: Some("Notes folder not set".to_string()),
        }),
    }
}

#[tauri::command]
async fn git_fetch(state: State<'_, AppState>) -> Result<git::GitResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                git::fetch(&PathBuf::from(path))
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitResult {
            success: false,
            message: None,
            error: Some("Notes folder not set".to_string()),
        }),
    }
}

#[tauri::command]
async fn git_pull(state: State<'_, AppState>) -> Result<git::GitResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                git::pull(&PathBuf::from(path))
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitResult {
            success: false,
            message: None,
            error: Some("Notes folder not set".to_string()),
        }),
    }
}

#[tauri::command]
async fn git_add_remote(url: String, state: State<'_, AppState>) -> Result<git::GitResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                git::add_remote(&PathBuf::from(path), &url)
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitResult {
            success: false,
            message: None,
            error: Some("Notes folder not set".to_string()),
        }),
    }
}

#[tauri::command]
async fn git_push_with_upstream(state: State<'_, AppState>) -> Result<git::GitResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone()
    };

    match folder {
        Some(path) => {
            tauri::async_runtime::spawn_blocking(move || {
                // Get current branch first
                let status = git::get_status(&PathBuf::from(&path));
                match status.current_branch {
                    Some(branch) => {
                        if !branch
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
                        {
                            return git::GitResult {
                                success: false,
                                message: None,
                                error: Some("Invalid branch name".to_string()),
                            };
                        }
                        git::push_with_upstream(&PathBuf::from(&path), &branch)
                    }
                    None => git::GitResult {
                        success: false,
                        message: None,
                        error: Some("No current branch found".to_string()),
                    },
                }
            })
            .await
            .map_err(|e| e.to_string())
        }
        None => Ok(git::GitResult {
            success: false,
            message: None,
            error: Some("Notes folder not set".to_string()),
        }),
    }
}

// Check if Claude CLI is installed
fn get_expanded_path() -> String {
    let system_path = std::env::var("PATH").unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::new());

    if home.is_empty() {
        return system_path;
    }

    // Common locations for node-installed CLIs (nvm, volta, fnm, mise, homebrew, global npm)
    let candidate_dirs = vec![
        format!("{home}/.nvm/versions/node"),
        format!("{home}/.fnm/node-versions"),
        format!("{home}/.local/share/mise/installs/node"),
    ];
    let static_dirs = vec![
        format!("{home}/.bun/bin"),
        format!("{home}/.volta/bin"),
        format!("{home}/.local/bin"),
        "/usr/local/bin".to_string(),
        "/opt/homebrew/bin".to_string(),
    ];

    let mut expanded = Vec::new();

    // Prefer well-known static locations (e.g. ~/.local/bin for native CLI installs)
    for dir in static_dirs {
        expanded.push(dir);
    }

    // Then scan nvm/fnm node version dirs containing a bin/ folder
    for base in &candidate_dirs {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let bin_path = entry.path().join("bin");
                if bin_path.exists() {
                    expanded.push(bin_path.to_string_lossy().to_string());
                }
            }
        }
    }

    expanded.push(system_path);
    expanded.join(":")
}

/// Create a `Command` that hides the console window on Windows.
fn no_window_cmd(program: &str) -> std::process::Command {
    let cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        cmd
    }
}

fn check_cli_exists(command_name: &str, path: &str) -> Result<bool, String> {
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };

    let check_output = no_window_cmd(which_cmd)
        .arg(command_name)
        .env("PATH", path)
        .output()
        .map_err(|e| format!("Failed to check for {} CLI: {}", command_name, e))?;

    Ok(check_output.status.success())
}

/// Marker comment embedded in CLI wrapper scripts installed by Scratch.
/// Used to identify and validate our own wrapper before modifying or removing it.
#[cfg(target_os = "macos")]
const SCRATCH_CLI_MARKER: &str = "# SCRATCH_CLI_WRAPPER";

/// Returns the path where the CLI script should be installed (macOS only).
/// Checks PATH for Homebrew bin first, then falls back to architecture detection.
/// Apple Silicon: /opt/homebrew/bin/scratch
/// Intel: /usr/local/bin/scratch
#[cfg(target_os = "macos")]
fn cli_target_path() -> PathBuf {
    // Check if the user's PATH contains /opt/homebrew/bin (Homebrew on Apple Silicon)
    if let Ok(path_var) = std::env::var("PATH") {
        if path_var.split(':').any(|p| p == "/opt/homebrew/bin") {
            return PathBuf::from("/opt/homebrew/bin/scratch");
        }
    }
    // Fall back to architecture detection
    if std::env::consts::ARCH == "aarch64" {
        return PathBuf::from("/opt/homebrew/bin/scratch");
    }
    PathBuf::from("/usr/local/bin/scratch")
}

#[tauri::command]
fn get_cli_status() -> Result<CliStatus, String> {
    #[cfg(not(target_os = "macos"))]
    return Ok(CliStatus { supported: false, installed: false, path: None });

    #[cfg(target_os = "macos")]
    {
        let target = cli_target_path();
        if !target.exists() && target.symlink_metadata().is_err() {
            return Ok(CliStatus { supported: true, installed: false, path: None });
        }
        // Verify this is our wrapper (has marker) and points to the current binary
        let content = std::fs::read_to_string(&target).unwrap_or_default();
        if !content.contains(SCRATCH_CLI_MARKER) {
            // Foreign binary at this path — don't claim it as ours
            return Ok(CliStatus { supported: true, installed: false, path: None });
        }
        let current_exe = std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !current_exe.is_empty() && !content.contains(&current_exe) {
            // Our wrapper but points to a moved/deleted binary — needs reinstall
            return Ok(CliStatus { supported: true, installed: false, path: None });
        }
        Ok(CliStatus {
            supported: true,
            installed: true,
            path: Some(target.to_string_lossy().into_owned()),
        })
    }
}

#[tauri::command]
fn install_cli() -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    return Err("CLI install is only supported on macOS".to_string());

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;

        let target = cli_target_path();

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
        }

        if target.exists() || target.symlink_metadata().is_ok() {
            // Only remove if it's our wrapper (contains marker)
            let content = std::fs::read_to_string(&target).unwrap_or_default();
            if !content.contains(SCRATCH_CLI_MARKER) {
                return Err(format!(
                    "A different 'scratch' command already exists at {}. Remove it manually to install the Scratch CLI.",
                    target.display()
                ));
            }
            std::fs::remove_file(&target)
                .map_err(|e| format!("Failed to remove existing file: {}", e))?;
        }

        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Cannot find exe path: {}", e))?;

        // Shell-escape the exe path using single quotes to prevent
        // interpretation of $, `, ", and other metacharacters.
        let exe_str = exe_path.to_string_lossy();
        let escaped_exe = format!("'{}'", exe_str.replace('\'', "'\\''"));

        // Write a wrapper script that launches the binary in the background so
        // the terminal is not blocked waiting for the GUI app to exit.
        let script = format!(
            "#!/bin/sh\n{}\nnohup {} \"$@\" >/dev/null 2>&1 &\n",
            SCRATCH_CLI_MARKER,
            escaped_exe
        );
        std::fs::write(&target, script.as_bytes())
            .map_err(|e| format!("Failed to write CLI script: {}", e))?;

        let mut perms = std::fs::metadata(&target)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;

        Ok(target.to_string_lossy().into_owned())
    }
}

#[tauri::command]
fn uninstall_cli() -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    return Ok(());

    #[cfg(target_os = "macos")]
    {
        let target = cli_target_path();
        if target.exists() || target.symlink_metadata().is_ok() {
            let content = std::fs::read_to_string(&target).unwrap_or_default();
            if !content.contains(SCRATCH_CLI_MARKER) {
                return Err(format!(
                    "File at {} was not installed by Scratch. Refusing to remove.",
                    target.display()
                ));
            }
            std::fs::remove_file(&target)
                .map_err(|e| format!("Failed to remove CLI script: {}", e))?;
        }
        Ok(())
    }
}

#[tauri::command]
async fn ai_check_claude_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("claude", &path)
    })
    .await
    .map_err(|e| format!("Failed to check Claude CLI: {}", e))?
}

#[tauri::command]
async fn ai_check_codex_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("codex", &path)
    })
    .await
    .map_err(|e| format!("Failed to check Codex CLI: {}", e))?
}

#[tauri::command]
async fn ai_check_opencode_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("opencode", &path)
    })
    .await
    .map_err(|e| format!("Failed to check OpenCode CLI: {}", e))?
}

/// Shared AI CLI execution: spawns `command` with `args`, writes `stdin_input` to stdin,
/// and returns the result with a 5-minute timeout.
async fn execute_ai_cli(
    cli_name: &str,
    command: String,
    args: Vec<String>,
    stdin_input: String,
    not_found_msg: String,
    current_dir: Option<String>,
    extra_env: Option<Vec<(String, String)>>,
) -> Result<AiExecutionResult, String> {
    use std::io::Write;
    use std::process::{Child, Stdio};

    let cli_name = cli_name.to_string();
    let timeout_duration = std::time::Duration::from_secs(300);
    let shared_child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    let child_for_task = Arc::clone(&shared_child);
    let cli_name_task = cli_name.clone();

    let mut task = tauri::async_runtime::spawn_blocking(move || {
        // Blocking I/O: expand PATH and check CLI exists
        let path = get_expanded_path();
        match check_cli_exists(&command, &path) {
            Ok(false) => {
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(not_found_msg),
                };
            }
            Err(e) => {
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(e),
                };
            }
            Ok(true) => {}
        }

        let mut cmd = no_window_cmd(&command);
        cmd.env("PATH", &path);
        if let Some(dir) = &current_dir {
            cmd.current_dir(dir);
        }
        if let Some(env_pairs) = &extra_env {
            for (key, value) in env_pairs {
                cmd.env(key, value);
            }
        }
        for arg in &args {
            cmd.arg(arg);
        }
        let process = match cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(p) => p,
            Err(e) => {
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to execute {}: {}", cli_name_task, e)),
                };
            }
        };

        // Store process in shared state so the timeout handler can kill it.
        // We only take individual I/O handles below — the Child stays in the
        // mutex so it remains reachable for kill().
        if let Ok(mut guard) = child_for_task.lock() {
            *guard = Some(process);
        } else {
            return AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to lock {} process handle", cli_name_task)),
            };
        }

        // Take stdin handle (briefly locks then releases)
        let stdin_handle = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.stdin.take()));

        if let Some(mut stdin) = stdin_handle {
            if let Err(e) = stdin.write_all(stdin_input.as_bytes()) {
                if let Ok(mut g) = child_for_task.lock() {
                    if let Some(ref mut p) = *g {
                        let _ = p.kill();
                        let _ = p.wait();
                    }
                }
                return AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to write to {} stdin: {}", cli_name_task, e)),
                };
            }
            // stdin dropped here — closes the pipe
        } else {
            if let Ok(mut g) = child_for_task.lock() {
                if let Some(ref mut p) = *g {
                    let _ = p.kill();
                    let _ = p.wait();
                }
            }
            return AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to open stdin for {}", cli_name_task)),
            };
        }

        // Take stdout/stderr handles so we can read without holding the lock.
        // This allows the timeout handler to lock the mutex and kill the process.
        let stdout_handle = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.stdout.take()));
        let stderr_handle = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.stderr.take()));

        use std::io::Read;

        let mut stdout_str = String::new();
        if let Some(mut out) = stdout_handle {
            let _ = out.read_to_string(&mut stdout_str);
        }

        let mut stderr_str = String::new();
        if let Some(mut err) = stderr_handle {
            let _ = err.read_to_string(&mut stderr_str);
        }

        // Collect exit status — process has exited after stdout/stderr close
        let success = child_for_task
            .lock()
            .ok()
            .and_then(|mut g| g.as_mut().and_then(|p| p.wait().ok()))
            .map(|s| s.success())
            .unwrap_or(false);

        // Strip ANSI escape sequences from output (e.g. Ollama progress spinners)
        let ansi_re = regex::Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\].*?\x07").unwrap();
        let stdout_clean = ansi_re.replace_all(&stdout_str, "").to_string();
        let stderr_clean = ansi_re.replace_all(&stderr_str, "").trim().to_string();

        if success {
            AiExecutionResult {
                success: true,
                output: stdout_clean,
                error: None,
            }
        } else {
            AiExecutionResult {
                success: false,
                output: stdout_clean,
                error: Some(stderr_clean),
            }
        }
    });

    let result = match tokio::time::timeout(timeout_duration, &mut task).await {
        Ok(join_result) => {
            join_result.map_err(|e| format!("Failed to join {} blocking task: {}", cli_name, e))?
        }
        Err(_) => {
            // Kill through the shared handle — the Child is still in the mutex
            // because the blocking task only takes I/O handles, not the Child.
            // This sends SIGKILL, which closes the pipes and unblocks the reads.
            if let Ok(mut guard) = shared_child.lock() {
                if let Some(ref mut process) = *guard {
                    let _ = process.kill();
                }
            }

            match tokio::time::timeout(std::time::Duration::from_secs(5), task).await {
                Ok(join_result) => {
                    if let Err(e) = join_result {
                        return Err(format!(
                            "Failed to join {} blocking task after timeout: {}",
                            cli_name, e
                        ));
                    }
                }
                Err(_) => {
                    return Err(format!(
                        "{} CLI timed out and failed to exit after kill signal",
                        cli_name
                    ));
                }
            }

            AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!("{} CLI timed out after 5 minutes", cli_name)),
            }
        }
    };

    Ok(result)
}

#[tauri::command]
async fn ai_execute_claude(
    file_path: String,
    prompt: String,
    state: State<'_, AppState>,
) -> Result<AiExecutionResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone().ok_or("Notes folder not set")?
    };
    let path = PathBuf::from(&file_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("md") && !ext.eq_ignore_ascii_case("markdown") {
        return Err("AI editing is only supported for markdown files".to_string());
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| "Invalid file path".to_string())?;
    let notes_root = PathBuf::from(&folder)
        .canonicalize()
        .map_err(|_| "Invalid notes folder".to_string())?;
    if !canonical.starts_with(&notes_root) {
        return Err("File must be within notes folder".to_string());
    }

    execute_ai_cli(
        "Claude",
        "claude".to_string(),
        vec![
            canonical.to_string_lossy().to_string(),
            "--dangerously-skip-permissions".to_string(),
            "--print".to_string(),
        ],
        prompt,
        "Claude CLI not found. Please install it from https://claude.ai/code".to_string(),
        None,
        None,
    )
    .await
}

#[tauri::command]
async fn ai_execute_codex(file_path: String, prompt: String) -> Result<AiExecutionResult, String> {
    let stdin_input = format!(
        "Edit only this markdown file: {file_path}\n\
         Apply the user's instructions below directly to that file.\n\
         Do not create, delete, rename, or modify any other files.\n\
         User instructions:\n\
         {prompt}"
    );

    execute_ai_cli(
        "Codex",
        "codex".to_string(),
        vec![
            "exec".to_string(),
            "--skip-git-repo-check".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "-".to_string(),
        ],
        stdin_input,
        "Codex CLI not found. Please install it from https://github.com/openai/codex".to_string(),
        None,
        None,
    )
    .await
}

#[tauri::command]
async fn ai_execute_opencode(
    file_path: String,
    prompt: String,
    state: State<'_, AppState>,
) -> Result<AiExecutionResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone().ok_or("Notes folder not set")?
    };
    let path = PathBuf::from(&file_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("md") && !ext.eq_ignore_ascii_case("markdown") {
        return Err("AI editing is only supported for markdown files".to_string());
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| "Invalid file path".to_string())?;
    let notes_root = PathBuf::from(&folder)
        .canonicalize()
        .map_err(|_| "Invalid notes folder".to_string())?;
    if !canonical.starts_with(&notes_root) {
        return Err("File must be within notes folder".to_string());
    }

    let run_prompt = format!(
        "Edit the attached markdown file in place.\n\
         Do not create, delete, rename, or modify any other files.\n\
         User instructions:\n\
         {}",
        prompt
    );

    execute_ai_cli(
        "OpenCode",
        "opencode".to_string(),
        vec![
            "run".to_string(),
            "--file".to_string(),
            canonical.to_string_lossy().to_string(),
            "--".to_string(),
            run_prompt,
        ],
        String::new(),
        "OpenCode CLI not found. Please install it from https://opencode.ai".to_string(),
        Some(notes_root.to_string_lossy().to_string()),
        Some(vec![
            (
                "OPENCODE_PERMISSION".to_string(),
                r#"{"*":"allow","bash":"deny","task":"deny","webfetch":"deny","websearch":"deny","codesearch":"deny","skill":"deny","external_directory":"deny","doom_loop":"deny"}"#.to_string(),
            ),
        ]),
    )
    .await
}

#[tauri::command]
async fn ai_check_ollama_cli() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let path = get_expanded_path();
        check_cli_exists("ollama", &path)
    })
    .await
    .map_err(|e| format!("Failed to check Ollama CLI: {}", e))?
}

#[tauri::command]
async fn ai_execute_ollama(
    file_path: String,
    prompt: String,
    model: String,
    state: State<'_, AppState>,
) -> Result<AiExecutionResult, String> {
    let folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config.notes_folder.clone().ok_or("Notes folder not set")?
    };
    let path = PathBuf::from(&file_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !ext.eq_ignore_ascii_case("md") && !ext.eq_ignore_ascii_case("markdown") {
        return Err("AI editing is only supported for markdown files".to_string());
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| "Invalid file path".to_string())?;
    let notes_root = PathBuf::from(&folder)
        .canonicalize()
        .map_err(|_| "Invalid notes folder".to_string())?;
    if !canonical.starts_with(&notes_root) {
        return Err("File must be within notes folder".to_string());
    }

    // Read the current file content
    let file_content = tokio::fs::read_to_string(&canonical)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let stdin_input = format!(
        "You are a markdown editor. Edit the markdown content below according to the user's instructions.\n\
         Return ONLY the complete edited markdown content.\n\
         Do NOT include any explanation, commentary, or code fences around the output.\n\
         Do NOT add ```markdown or ``` wrappers.\n\n\
         Current markdown content:\n{file_content}\n\n\
         User instructions:\n{prompt}"
    );

    // Use the model provided (frontend reads from settings, defaults to "qwen3:8b")
    let trimmed = model.trim();
    let model_name = if trimmed.is_empty() {
        "qwen3:8b".to_string()
    } else {
        trimmed.to_string()
    };

    // Check if the model is available locally before running (skip for cloud models)
    if !model_name.contains("cloud") {
        let mn = model_name.clone();
        let available = tauri::async_runtime::spawn_blocking(move || {
            let path = get_expanded_path();
            let mut cmd = no_window_cmd("ollama");
            cmd.env("PATH", &path);
            cmd.args(["show", &mn]);
            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            match cmd.status() {
                Ok(status) => status.success(),
                Err(_) => false,
            }
        })
        .await
        .unwrap_or(false);

        if !available {
            return Ok(AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some(format!(
                    "Model '{}' is not installed. Run: ollama pull {}",
                    model_name, model_name
                )),
            });
        }
    }

    let result = execute_ai_cli(
        "Ollama",
        "ollama".to_string(),
        vec!["run".to_string(), model_name.clone()],
        stdin_input,
        "Ollama CLI not found. Please install it from https://ollama.com".to_string(),
        None,
        None,
    )
    .await?;

    // Improve error messages for common Ollama failures
    if !result.success {
        if let Some(ref err) = result.error {
            let err_lower = err.to_lowercase();
            if err_lower.contains("file does not exist")
                || err_lower.contains("pull model manifest")
                || err_lower.contains("model not found")
                || err_lower.contains("model does not exist")
            {
                return Ok(AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "Model '{}' not found. Run `ollama pull {}` in your terminal to download it.",
                        model_name, model_name
                    )),
                });
            }
            if err.contains("401") || err.contains("Unauthorized") {
                return Ok(AiExecutionResult {
                    success: false,
                    output: String::new(),
                    error: Some("Authentication required. Run `ollama login` in your terminal to sign in.".to_string()),
                });
            }
        }
    }

    // If successful, write the output back to the file
    if result.success {
        let edited_content = result.output.trim().to_string();
        if edited_content.is_empty() {
            return Ok(AiExecutionResult {
                success: false,
                output: String::new(),
                error: Some("Ollama returned empty output. Please try again.".to_string()),
            });
        }
        tokio::fs::write(&canonical, edited_content.as_bytes())
            .await
            .map_err(|e| format!("Failed to write edited file: {}", e))?;

        Ok(AiExecutionResult {
            success: true,
            output: "Note edited successfully with Ollama.".to_string(),
            error: None,
        })
    } else {
        Ok(result)
    }
}

// ============================================================================
// P2P Sharing Commands (Phase 1)
// ============================================================================

/// Start P2P networking
#[tauri::command]
async fn p2p_start(app: AppHandle, state: State<'_, AppState>) -> Result<p2p::P2PStatus, String> {
    let mut network = state.p2p_network.lock().expect("p2p_network lock");
    let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");
    let notes_folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    // Check if already running
    if p2p_state.is_running {
        if let Some(ref net) = *network {
            if let Some(peer_id) = net.peer_id() {
                return Ok(p2p::P2PStatus {
                    is_running: true,
                    peer_id: peer_id.to_string(),
                    connected_peers: net.connected_peers(),
                });
            }
        }
    }

    // Start network
    let net_manager = &mut *network;
    let net_manager = net_manager.get_or_insert_with(p2p::NetworkManager::new);

    let peer_id = net_manager
        .start(app, PathBuf::from(&notes_folder))
        .map_err(|e| format!("Failed to start P2P network: {}", e))?;

    let shares_to_register = p2p_state.shares.clone();
    let notes_root = PathBuf::from(&notes_folder);
    for share in shares_to_register {
        match resolve_share_folder_path(&notes_root, &share.local_path) {
            Ok(full_path) => {
                let remote_peer = if share.is_owner {
                    None
                } else {
                    Some(share.peer_id.clone())
                };
                net_manager.register_share(
                    share.id.clone(),
                    full_path,
                    remote_peer,
                    share.permission.clone(),
                    share.is_owner,
                );
            }
            Err(err) => {
                log::warn!(
                    "Skipping persisted share '{}' due to invalid path '{}': {}",
                    share.id,
                    share.local_path,
                    err
                );
            }
        }
    }

    p2p_state.set_running(true);
    p2p_state.set_peer_id(peer_id.to_string());

    log::info!("P2P started: {}", peer_id);

    Ok(p2p::P2PStatus {
        is_running: true,
        peer_id: peer_id.to_string(),
        connected_peers: net_manager.connected_peers(),
    })
}

/// Stop P2P networking
#[tauri::command]
async fn p2p_stop(state: State<'_, AppState>) -> Result<(), String> {
    let mut network = state.p2p_network.lock().expect("p2p_network lock");
    let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");

    if let Some(ref mut net) = *network {
        net.stop()
            .map_err(|e| format!("Failed to stop P2P network: {}", e))?;
    }

    p2p_state.set_running(false);
    p2p_state.peer_id = None;

    log::info!("P2P stopped");

    Ok(())
}

/// Get P2P status
#[tauri::command]
async fn p2p_get_status(state: State<'_, AppState>) -> Result<p2p::P2PStatus, String> {
    let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
    let network = state.p2p_network.lock().expect("p2p_network lock");

    let peer_id = p2p_state.peer_id.clone().unwrap_or_default();
    let is_running = p2p_state.is_running;

    // Count connected peers (Phase 2: will use actual peer list)
    let connected_peers = if is_running {
        network
            .as_ref()
            .map(|n| n.connected_peers())
            .unwrap_or(0)
    } else {
        0
    };

    Ok(p2p::P2PStatus {
        is_running,
        peer_id,
        connected_peers,
    })
}

fn resolve_share_folder_path(base_path: &Path, input: &str) -> Result<PathBuf, String> {
    let base_canonical = base_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve notes folder: {}", e))?;

    if input.trim().is_empty() {
        return Ok(base_canonical);
    }

    if input.contains('\\') {
        return Err("Invalid folder path: backslashes are not allowed".to_string());
    }

    let rel = Path::new(input);
    if rel.is_absolute() {
        return Err("Invalid folder path: absolute paths are not allowed".to_string());
    }

    let mut resolved = base_canonical.clone();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(name) => {
                resolved.push(name);

                // Block traversal via symlink components that escape the notes folder.
                if resolved.exists() {
                    let metadata = std::fs::symlink_metadata(&resolved)
                        .map_err(|e| format!("Failed to read path metadata: {}", e))?;
                    if metadata.file_type().is_symlink() {
                        let target = resolved
                            .canonicalize()
                            .map_err(|e| format!("Failed to resolve symlink: {}", e))?;
                        if !target.starts_with(&base_canonical) {
                            return Err(
                                "Invalid folder path: symlink escapes notes folder".to_string(),
                            );
                        }
                    }
                }
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(
                    "Invalid folder path: parent directory references are not allowed".to_string(),
                );
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err("Invalid folder path: absolute paths are not allowed".to_string());
            }
        }
    }

    if !resolved.starts_with(&base_canonical) {
        return Err("Invalid folder path: path escapes notes folder".to_string());
    }

    Ok(resolved)
}

fn share_relative_file_path(share_local_path: &str, note_id: &str) -> Option<String> {
    let suffix = if share_local_path.is_empty() {
        note_id.to_string()
    } else if note_id == share_local_path {
        return None;
    } else if let Some(rest) = note_id.strip_prefix(&(share_local_path.to_string() + "/")) {
        rest.to_string()
    } else {
        return None;
    };

    Some(format!("{}.md", suffix))
}

fn persist_p2p_shares(state: &AppState) -> Result<(), String> {
    let notes_folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set".to_string())?
    };

    let shares = {
        let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state.shares.clone()
    };

    save_p2p_shares(&notes_folder, &shares).map_err(|e| format!("Failed to persist shares: {}", e))
}

fn notify_p2p_note_change(state: &State<'_, AppState>, note_id: &str, is_deleted: bool) {
    let shares = {
        let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state.shares.clone()
    };

    if shares.is_empty() {
        return;
    }

    let network = state.p2p_network.lock().expect("p2p_network lock");
    let Some(net) = network.as_ref() else { return; };

    for share in shares {
        let can_send_changes = share.is_owner || matches!(share.permission, p2p::SharePermission::ReadWrite);
        if !can_send_changes {
            continue;
        }

        if let Some(relative_path) = share_relative_file_path(&share.local_path, note_id) {
            net.notify_file_change(share.id.clone(), relative_path, is_deleted);
        }
    }
}

/// Create a share for a folder
#[tauri::command]
async fn p2p_create_share(
    folder_path: String,
    permission: String,
    state: State<'_, AppState>,
) -> Result<p2p::CreateShareResult, String> {
    let notes_folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    // Parse permission
    let share_permission = match permission.as_str() {
        "read_only" => p2p::SharePermission::ReadOnly,
        "read_write" => p2p::SharePermission::ReadWrite,
        _ => return Err("Invalid permission. Use 'read_only' or 'read_write'".to_string()),
    };

    // Get peer ID
    let peer_id = {
        let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state
            .peer_id
            .clone()
            .ok_or("P2P not started. Call p2p_start first.")?
    };

    // Resolve and validate folder path
    let base_path = PathBuf::from(&notes_folder);
    let full_path = resolve_share_folder_path(&base_path, &folder_path)?;

    // Validate path exists
    if !full_path.exists() {
        return Err(format!("Folder not found: {}", folder_path));
    }
    if !full_path.is_dir() {
        return Err(format!("Path is not a folder: {}", folder_path));
    }

    // Get folder name
    let folder_name = full_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Shared")
        .to_string();

    // Create invite payload
    let payload = p2p::create_invite_payload(
        peer_id.clone(),
        folder_name.clone(),
        share_permission.clone(),
        None,
    )
        .map_err(|e| format!("Failed to create invite: {}", e))?;

    // Encode invite code
    let invite_code = p2p::encode_invite_code(&payload)
        .map_err(|e| format!("Failed to encode invite: {}", e))?;

    // Use the invite folder ID as the canonical cross-peer share ID.
    let share_id = payload.folder_id.clone();

    // Create shared folder record
    let shared_folder = p2p::SharedFolder {
        id: share_id.clone(),
        local_path: folder_path.clone(),
        remote_path: folder_name.clone(),
        peer_id: peer_id.clone(),
        peer_name: None,
        permission: share_permission.clone(),
        is_owner: true,
        sync_status: p2p::SyncStatus::Idle,
        last_synced: 0,
        created_at: chrono::Utc::now().timestamp(),
    };

    // Add to state
    {
        let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        if p2p_state.get_share(&share_id).is_some() {
            return Err(format!("Share already exists: {}", share_id));
        }
        p2p_state.add_share(shared_folder);
    }
    persist_p2p_shares(&state)?;

    if let Some(network) = state
        .p2p_network
        .lock()
        .expect("p2p_network lock")
        .as_ref()
    {
        network.register_share(
            share_id.clone(),
            full_path.clone(),
            None,
            share_permission,
            true,
        );
    }

    log::info!("Created share '{}' for folder '{}'", share_id, folder_path);

    Ok(p2p::CreateShareResult {
        invite_code,
        share_id,
    })
}

/// Accept a share invite
#[tauri::command]
async fn p2p_accept_share(
    invite_code: String,
    destination_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<p2p::SharedFolder, String> {
    let notes_folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };

    // Decode invite code
    let payload = p2p::decode_invite_code(&invite_code)
        .map_err(|e| format!("Invalid invite code: {}", e))?;

    // Get local peer ID
    let _local_peer_id = {
        let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state
            .peer_id
            .clone()
            .ok_or("P2P not started. Call p2p_start first.")?
    };

    // Resolve and validate destination path
    let base_path = PathBuf::from(&notes_folder);
    let destination_input = if destination_path.is_empty() {
        payload.folder_name.clone()
    } else {
        destination_path
    };
    let dest_path = resolve_share_folder_path(&base_path, &destination_input)?;

    // Ensure destination directory exists
    fs::create_dir_all(&dest_path)
        .await
        .map_err(|e| format!("Failed to create destination directory: {}", e))?;

    // Get relative path
    let relative_path = dest_path
        .strip_prefix(&base_path)
        .map_err(|e| format!("Invalid destination path: {}", e))?
        .to_str()
        .ok_or("Invalid UTF-8 in path")?
        .to_string();

    // Keep the same share ID across peers using the invite payload folder ID.
    let share_id = payload.folder_id.clone();
    let shared_folder = p2p::SharedFolder {
        id: share_id.clone(),
        local_path: relative_path.clone(),
        remote_path: payload.folder_name.clone(),
        peer_id: payload.sender_peer_id.clone(),
        peer_name: None,
        permission: payload.permissions.clone(),
        is_owner: false,
        sync_status: p2p::SyncStatus::DiscoveringPeer,
        last_synced: 0,
        created_at: chrono::Utc::now().timestamp(),
    };

    // Add to state
    {
        let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        if p2p_state.get_share(&share_id).is_some() {
            return Err(format!("Share already exists: {}", share_id));
        }
        p2p_state.add_share(shared_folder.clone());
    }
    persist_p2p_shares(&state)?;

    if let Some(network) = state
        .p2p_network
        .lock()
        .expect("p2p_network lock")
        .as_ref()
    {
        network.register_share(
            share_id.clone(),
            dest_path.clone(),
            Some(payload.sender_peer_id.clone()),
            payload.permissions.clone(),
            false,
        );
    }

    let _ = app.emit(
        "p2p-share-accepted",
        serde_json::json!({
            "share_id": share_id,
            "local_path": relative_path,
        }),
    );

    log::info!("Accepted share '{}' from peer '{}'", share_id, payload.sender_peer_id);

    Ok(shared_folder)
}

/// List all shares
#[tauri::command]
async fn p2p_list_shares(state: State<'_, AppState>) -> Result<Vec<p2p::SharedFolder>, String> {
    let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
    Ok(p2p_state.shares.clone())
}

/// Revoke a share
#[tauri::command]
async fn p2p_revoke_share(share_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");

    if !p2p_state.remove_share(&share_id) {
        return Err(format!("Share not found: {}", share_id));
    }
    drop(p2p_state);
    persist_p2p_shares(&state)?;
    if let Some(network) = state
        .p2p_network
        .lock()
        .expect("p2p_network lock")
        .as_ref()
    {
        network.remove_share(share_id.clone());
    }

    log::info!("Revoked share '{}'", share_id);

    Ok(())
}

/// Get sync status for a share
#[tauri::command]
async fn p2p_get_sync_status(
    share_id: String,
    state: State<'_, AppState>,
) -> Result<p2p::SyncStatus, String> {
    let p2p_state = state.p2p_state.lock().expect("p2p_state lock");

    let share = p2p_state
        .get_share(&share_id)
        .ok_or(format!("Share not found: {}", share_id))?;

    Ok(share.sync_status.clone())
}

/// Manually trigger sync for a share
#[tauri::command]
async fn p2p_manual_sync(share_id: String, state: State<'_, AppState>) -> Result<(), String> {
    {
        let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        if p2p_state.get_share(&share_id).is_none() {
            return Err(format!("Share not found: {}", share_id));
        }
    }

    if let Some(network) = state
        .p2p_network
        .lock()
        .expect("p2p_network lock")
        .as_ref()
    {
        network.trigger_sync(share_id);
        Ok(())
    } else {
        Err("P2P network is not running".to_string())
    }
}

/// Resolve conflict for a shared file
#[tauri::command]
async fn p2p_resolve_conflict(
    share_id: String,
    file_path: String,
    resolution: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if file_path.contains('\\') {
        return Err("Invalid file path".to_string());
    }
    let rel = Path::new(&file_path);
    if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("Invalid file path".to_string());
    }

    let notes_folder = {
        let app_config = state.app_config.read().expect("app_config read lock");
        app_config
            .notes_folder
            .clone()
            .ok_or("Notes folder not set")?
    };
    let base_path = PathBuf::from(notes_folder);

    let share = {
        let p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state
            .get_share(&share_id)
            .cloned()
            .ok_or(format!("Share not found: {}", share_id))?
    };
    let share_path = resolve_share_folder_path(&base_path, &share.local_path)?;
    let local_path = share_path.join(rel);
    let conflict_path = share_path.join(p2p::sync::conflict_copy_name(&file_path));

    match resolution.as_str() {
        "keep_local" => {
            if conflict_path.exists() {
                fs::remove_file(&conflict_path)
                    .await
                    .map_err(|e| format!("Failed to remove conflict copy: {}", e))?;
            }
        }
        "keep_remote" => {
            if !conflict_path.exists() {
                return Err("Conflict copy not found. Run sync again to fetch remote conflict copy.".to_string());
            }
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| format!("Failed to prepare file path: {}", e))?;
            }
            if local_path.exists() {
                fs::remove_file(&local_path)
                    .await
                    .map_err(|e| format!("Failed to replace local file: {}", e))?;
            }
            fs::rename(&conflict_path, &local_path)
                .await
                .map_err(|e| format!("Failed to promote remote version: {}", e))?;
        }
        "keep_both" => {
            if !conflict_path.exists() {
                if !local_path.exists() {
                    return Err("No local file available for keep_both".to_string());
                }
                if let Some(parent) = conflict_path.parent() {
                    fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("Failed to prepare conflict path: {}", e))?;
                }
                fs::copy(&local_path, &conflict_path)
                    .await
                    .map_err(|e| format!("Failed to create conflict copy: {}", e))?;
            }
        }
        _ => return Err("Invalid resolution. Use keep_local, keep_remote, or keep_both".to_string()),
    }

    {
        let mut p2p_state = state.p2p_state.lock().expect("p2p_state lock");
        p2p_state.update_share_status(&share_id, p2p::SyncStatus::Synced);
    }
    persist_p2p_shares(&state)?;

    Ok(())
}

/// Check if a markdown file is inside the configured notes folder.
/// If so, emit a "select-note" event to the main window and focus it, returning true.
/// Returns false on any failure so callers can fall back to create_preview_window.
fn try_select_in_notes_folder(app: &AppHandle, path: &Path) -> bool {
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return false,
    };

    let notes_folder = state
        .app_config
        .read()
        .expect("app_config read lock")
        .notes_folder
        .clone();

    let folder = match notes_folder {
        Some(f) => f,
        None => return false,
    };

    let folder_path = PathBuf::from(&folder);
    let (canonical_file, canonical_folder) = match (path.canonicalize(), folder_path.canonicalize())
    {
        (Ok(f), Ok(d)) => (f, d),
        _ => return false,
    };

    if !canonical_file.starts_with(&canonical_folder) {
        return false;
    }

    let note_id = match id_from_abs_path(&canonical_folder, &canonical_file) {
        Some(id) => id,
        None => return false,
    };

    let _ = app.emit_to("main", "select-note", note_id);
    if let Some(main_window) = app.get_webview_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }
    true
}

/// Check if a file extension is a supported markdown extension.
fn is_markdown_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            lower == "md" || lower == "markdown"
        })
        .unwrap_or(false)
}

// Preview mode: create a lightweight window for editing a single file
fn create_preview_window(app: &AppHandle, file_path: &str) -> Result<(), String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    file_path.hash(&mut hasher);
    let label = format!("preview-{:x}", hasher.finish());

    // If window already exists for this file, focus it
    if let Some(window) = app.get_webview_window(&label) {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    // Extract filename for the window title
    let filename = PathBuf::from(file_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Preview".to_string());

    let encoded_path = urlencoding::encode(file_path);
    let url = format!("index.html?mode=preview&file={}", encoded_path);

    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(format!("{} — Scratch", filename))
        .inner_size(800.0, 600.0)
        .min_inner_size(400.0, 300.0)
        .resizable(true)
        .decorations(true);

    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);

    let window = builder
        .build()
        .map_err(|e| format!("Failed to create preview window: {}", e))?;

    // Focus the preview window so it appears on top of the main window.
    // Use a short delay because during cold start the main window may steal
    // focus after its WebView finishes loading.
    let win = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = win.set_focus();
    });

    Ok(())
}

#[tauri::command]
fn open_file_preview(app: AppHandle, path: String) -> Result<(), String> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    if !try_select_in_notes_folder(&app, &file_path) {
        create_preview_window(&app, &path)?;
    }
    Ok(())
}

// Handle CLI arguments: open .md files in preview mode.
// Returns true if a standalone preview window was created (file outside notes folder).
fn handle_cli_args(app: &AppHandle, args: &[String], cwd: &str) -> bool {
    let mut opened_file = false;
    let mut opened_preview = false;
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        // Skip flags
        if arg.starts_with('-') {
            if arg == "--profile" {
                i += 1;
            }
            i += 1;
            continue;
        }

        let path = if PathBuf::from(arg).is_absolute() {
            PathBuf::from(arg)
        } else {
            PathBuf::from(cwd).join(arg)
        };

        if is_markdown_extension(&path) && path.is_file() {
            opened_file = true;
            if !try_select_in_notes_folder(app, &path)
                && create_preview_window(app, &path.to_string_lossy()).is_ok()
            {
                opened_preview = true;
            }
        } else if path.is_dir() {
            let canonical = path.canonicalize().unwrap_or(path.clone());
            let state = app.state::<AppState>();
            // Full initialization: directory creation, write-access check,
            // asset-scope update, config/settings persist, and search-index rebuild
            match initialize_notes_folder(app, &canonical, &state) {
                Ok(normalized_path) => {
                    // Emit event for when app is already running (single-instance)
                    let _ = app.emit("set-notes-folder", normalized_path);
                    opened_file = true;
                }
                Err(e) => {
                    eprintln!("Failed to initialize notes folder {:?}: {}", canonical, e);
                }
            }
            if let Some(main_window) = app.get_webview_window("main") {
                let _ = main_window.show();
                let _ = main_window.set_focus();
            }
        }

        i += 1;
    }

    // If no files were opened, show and focus the main window
    if !opened_file {
        if let Some(main_window) = app.get_webview_window("main") {
            let _ = main_window.show();
            let _ = main_window.set_focus();
        }
    }

    opened_preview
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let startup_args: Vec<String> = std::env::args().collect();
    let profile = extract_profile_from_args(&startup_args).or_else(current_profile_id);
    if let Some(ref profile_id) = profile {
        std::env::set_var(PROFILE_ENV_VAR, profile_id);
        eprintln!("Scratch running with profile '{}'", profile_id);
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    let builder = if profile.is_some() {
        // In profile mode we intentionally allow multiple concurrent instances
        // so separate profiles can run side-by-side for local testing.
        builder
    } else {
        // Default mode remains single-instance.
        builder.plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            handle_cli_args(app, &args, &cwd);
        }))
    };

    let app = builder
        .setup(|app| {
            // Load app config on startup (contains notes folder path)
            let mut app_config = load_app_config(app.handle());

            // Normalize legacy/invalid saved paths (e.g. file:// URI from older builds)
            if let Some(saved_path) = app_config.notes_folder.clone() {
                match normalize_notes_folder_path(&saved_path) {
                    Ok(normalized) if normalized.is_dir() => {
                        let normalized_str = normalized.to_string_lossy().into_owned();
                        if normalized_str != saved_path {
                            app_config.notes_folder = Some(normalized_str);
                            let _ = save_app_config(app.handle(), &app_config);
                        }
                    }
                    Ok(normalized) => {
                        // Path is structurally valid but not currently a directory
                        // (e.g., unmounted drive). Preserve the user's preference.
                        eprintln!("Notes folder not found (may be temporarily unavailable): {:?}", normalized);
                    }
                    Err(_) => {
                        app_config.notes_folder = None;
                        let _ = save_app_config(app.handle(), &app_config);
                    }
                }
            }

            // Load per-folder settings if notes folder is set
            let settings = if let Some(ref folder) = app_config.notes_folder {
                load_settings(folder)
            } else {
                Settings::default()
            };

            // Initialize search index if notes folder is set
            let search_index = if let Some(ref folder) = app_config.notes_folder {
                if let Ok(index_path) = get_search_index_path(app.handle()) {
                    SearchIndex::new(&index_path).ok().inspect(|idx| {
                        let _ = idx.rebuild_index(&PathBuf::from(folder));
                    })
                } else {
                    None
                }
            } else {
                None
            };

            let persisted_shares = app_config
                .notes_folder
                .as_ref()
                .map(|folder| load_p2p_shares(folder))
                .unwrap_or_default();

            let state = AppState {
                app_config: RwLock::new(app_config),
                settings: RwLock::new(settings),
                notes_cache: RwLock::new(HashMap::new()),
                file_watcher: Mutex::new(None),
                search_index: Mutex::new(search_index),
                debounce_map: Arc::new(Mutex::new(HashMap::new())),
                p2p_state: Mutex::new({
                    let mut p2p_state = p2p::P2PState::new();
                    p2p_state.shares = persisted_shares;
                    p2p_state
                }),
                p2p_network: Mutex::new(None),
            };
            app.manage(state);

            // Add notes folder to asset protocol scope so images can be served
            if let Some(ref folder) = app.state::<AppState>().app_config.read().expect("app_config read lock").notes_folder.clone() {
                let _ = app.asset_protocol_scope().allow_directory(folder, true);
            }

            // Handle CLI args on first launch; determine whether to show the main window.
            // When a standalone preview is opened (file outside the notes folder) and the
            // notes folder is already configured, the main window is closed so users only
            // see the preview. When no notes folder is configured yet, the main window is
            // always shown so new users can complete onboarding via the FolderPicker.
            let args: Vec<String> = std::env::args().collect();
            let opened_preview = if args.len() > 1 {
                let cwd = std::env::current_dir()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                handle_cli_args(app.handle(), &args, &cwd)
            } else {
                false
            };

            if let Some(main_window) = app.get_webview_window("main") {
                let has_notes_folder = app
                    .state::<AppState>()
                    .app_config
                    .read()
                    .expect("app_config read lock")
                    .notes_folder
                    .is_some();

                if opened_preview && has_notes_folder {
                    // Existing user: notes folder is configured and a standalone preview
                    // was opened. Close the hidden main window so only the preview is visible.
                    let _ = main_window.hide();
                } else {
                    // Show the main window when:
                    // - No standalone preview was opened (normal launch), OR
                    // - No notes folder is configured yet (new user needs FolderPicker
                    //   for onboarding, even if a preview is also showing).
                    let _ = main_window.show();
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            // Handle drag-and-drop of .md files onto any window
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                let app = window.app_handle();
                for path in paths {
                    if is_markdown_extension(path)
                        && path.is_file()
                        && !try_select_in_notes_folder(app, path)
                    {
                        let _ = create_preview_window(app, &path.to_string_lossy());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_notes_folder,
            set_notes_folder,
            list_notes,
            read_note,
            save_note,
            delete_note,
            create_note,
            list_folders,
            create_folder,
            delete_folder,
            rename_folder,
            move_note,
            move_folder,
            get_settings,
            update_settings,
            update_git_enabled,
            preview_note_name,
            write_file,
            search_notes,
            start_file_watcher,
            rebuild_search_index,
            copy_to_clipboard,
            copy_image_to_assets,
            save_clipboard_image,
            open_folder_dialog,
            open_in_file_manager,
            open_url_safe,
            git_is_available,
            git_get_status,
            git_init_repo,
            git_commit,
            git_push,
            git_fetch,
            git_pull,
            git_add_remote,
            git_push_with_upstream,
            ai_check_claude_cli,
            ai_check_codex_cli,
            ai_check_opencode_cli,
            ai_check_ollama_cli,
            ai_execute_claude,
            ai_execute_codex,
            ai_execute_opencode,
            ai_execute_ollama,
            read_file_direct,
            save_file_direct,
            import_file_to_folder,
            open_file_preview,
            install_cli,
            uninstall_cli,
            get_cli_status,
            // P2P commands
            p2p_start,
            p2p_stop,
            p2p_get_status,
            p2p_create_share,
            p2p_accept_share,
            p2p_list_shares,
            p2p_revoke_share,
            p2p_get_sync_status,
            p2p_manual_sync,
            p2p_resolve_conflict,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Use .run() callback to handle macOS "Open With" file events
    // RunEvent::Opened is macOS-only in Tauri v2
    app.run(|_app_handle, _event| {
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = _event {
            for url in urls {
                if let Ok(path) = url.to_file_path() {
                    if is_markdown_extension(&path)
                        && path.is_file()
                        && !try_select_in_notes_folder(_app_handle, &path)
                    {
                        let _ = create_preview_window(_app_handle, &path.to_string_lossy());
                    }
                }
            }
        }
    });
}
