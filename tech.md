# Scratch - Tech Stack Documentation

> **Last Updated:** 2026-04-01
> **Purpose:** Complete technical reference for the Scratch note-taking application.

---

## Table of Contents

1. [Overview](#overview)
2. [Frontend Stack](#frontend-stack)
3. [Backend Stack (Rust/Tauri)](#backend-stack-rusttauri)
4. [DevOps & Tooling](#devops--tooling)
5. [Architecture](#architecture)
6. [File Structure](#file-structure)
7. [Key Technologies Deep Dive](#key-technologies-deep-dive)
8. [Commands & Scripts](#commands--scripts)
9. [CI/CD](#cicd)
10. [Build & Release](#build--release)

---

## Overview

**Scratch** is a cross-platform desktop note-taking application built with:

- **Frontend:** React 19 + TypeScript + Tailwind CSS v4
- **Backend:** Tauri v2 + Rust
- **Editor:** TipTap (WYSIWYG with markdown support)
- **Search:** Tantivy (full-text search engine)
- **Platforms:** macOS, Windows, Linux

**Philosophy:** Offline-first, native performance, clean architecture, minimal technical debt.

---

## Frontend Stack

### Core Framework

| Technology | Version | Purpose |
|------------|---------|---------|
| **React** | 19.1.0 | UI framework |
| **TypeScript** | 5.8.3 | Type safety and developer experience |
| **Vite** | 7.0.4 | Fast build tool and dev server |

### UI Components & Styling

| Technology | Version | Purpose |
|------------|---------|---------|
| **Tailwind CSS** | 4.1.18 | Utility-first CSS framework |
| **@tailwindcss/vite** | 4.1.18 | Vite integration for Tailwind v4 |
| **@tailwindcss/typography** | 0.5.19 | Beautiful prose styling |
| **Radix UI** | Various | Headless, accessible UI primitives |
| **@dnd-kit** | 6.x | Modern drag-and-drop for folders |
| **Sonner** | 2.0.7 | Toast notifications |
| **Tippy.js** | 6.3.7 | Tooltip/popover engine |
| **clsx** | 2.1.1 | Conditional className utility |
| **tailwind-merge** | 3.4.0 | Intelligent Tailwind class merging |

**Radix UI Packages Used:**
- `@radix-ui/react-alert-dialog` - Confirm dialogs
- `@radix-ui/react-context-menu` - Right-click menus
- `@radix-ui/react-dropdown-menu` - Dropdown menus
- `@radix-ui/react-tooltip` - Tooltips

### Editor - TipTap

| Package | Version | Purpose |
|---------|---------|---------|
| **@tiptap/core** | 3.19.0 | Core editor engine |
| **@tiptap/react** | 3.18.0 | React integration |
| **@tiptap/starter-kit** | 3.18.0 | Basic extensions bundle |
| **@tiptap/markdown** | 3.18.0 | Markdown import/export |
| **@tiptap/extension-link** | 3.18.0 | Link support |
| **@tiptap/extension-image** | 3.18.0 | Image support |
| **@tiptap/extension-table** | 3.19.0 | Table support |
| **@tiptap/extension-table-row** | 3.19.0 | Table rows |
| **@tiptap/extension-table-cell** | 3.19.0 | Table cells |
| **@tiptap/extension-table-header** | 3.19.0 | Table headers |
| **@tiptap/extension-task-list** | 3.18.0 | Task lists |
| **@tiptap/extension-task-item** | 3.18.0 | Task items |
| **@tiptap/extension-placeholder** | 3.18.0 | Placeholder text |
| **@tiptap/extension-code-block-lowlight** | 3.20.0 | Code blocks with syntax highlighting |
| **@tiptap/extension-mathematics** | 3.18.0 | Math equations (KaTeX) |
| **@tiptap/pm** | 3.18.0 | ProseMirror utilities |
| **@tiptap/suggestion** | 3.18.0 | Suggestion utilities |

### Code Highlighting

| Package | Version | Purpose |
|---------|---------|---------|
| **highlight.js** | 11.11.1 | Syntax highlighting library |
| **lowlight** | 3.3.0 | lowlight for TipTap integration |

**Languages Supported (20 total):**
JavaScript, TypeScript, Python, Rust, Go, Java, C, C++, HTML, CSS, JSON, YAML, SQL, Bash, Markdown, PHP, Ruby, Swift, Kotlin, Dart

### Diagrams & Math

| Package | Version | Purpose |
|---------|---------|---------|
| **beautiful-mermaid** | 1.1.3 | Mermaid diagram rendering (sync SVG with CSS variables) |
| **KaTeX** | 0.16.33 | Math equation rendering |

### Tauri APIs

| Package | Version | Purpose |
|---------|---------|---------|
| **@tauri-apps/api** | 2.x | Core Tauri APIs |
| **@tauri-apps/plugin-clipboard-manager** | 2.3.2 | Clipboard access |
| **@tauri-apps/plugin-dialog** | 2.6.0 | Native file dialogs |
| **@tauri-apps/plugin-opener** | 2.x | Open URLs in default browser |
| **@tauri-apps/plugin-updater** | 2.10.0 | Auto-update functionality |

---

## Backend Stack (Rust/Tauri)

### Core Framework

| Crate | Version | Purpose |
|-------|---------|---------|
| **tauri** | 2.x | Desktop app framework |
| **tauri-build** | 2.x | Build script support |

### Tauri Plugins

| Crate | Version | Purpose |
|-------|---------|---------|
| **tauri-plugin-opener** | 2.x | Open URLs |
| **tauri-plugin-fs** | 2.x | File system access |
| **tauri-plugin-dialog** | 2.x | Native dialogs |
| **tauri-plugin-clipboard-manager** | 2.x | Clipboard operations |
| **tauri-plugin-updater** | 2.x | Auto-updater |
| **tauri-plugin-single-instance** | 2.x | Ensure only one instance running |

### Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| **tokio** | 1.x | Async runtime (with fs and sync features) |
| **serde** | 1.x | Serialization framework |
| **serde_json** | 1.x | JSON serialization |
| **anyhow** | 1.x | Error handling |
| **chrono** | 0.4.x | Date/time handling |

### File System & Watching

| Crate | Version | Purpose |
|-------|---------|---------|
| **notify** | 6.x | File system watching with debouncing |
| **walkdir** | 2.x | Recursive directory traversal |

### Search

| Crate | Version | Purpose |
|-------|---------|---------|
| **tantivy** | 0.22.x | Full-text search engine (Lucene-like for Rust) |

### Networking & Utilities

| Crate | Version | Purpose |
|-------|---------|---------|
| **url** | 2.x | URL parsing |
| **urlencoding** | 2.x | URL encoding/decoding |
| **base64** | 0.22.x | Base64 encoding/decoding |
| **open** | 5.x | Open URLs/files in default applications |
| **regex** | 1.x | Regular expressions |

---

## DevOps & Tooling

### Build Tools

| Tool | Version | Purpose |
|------|---------|---------|
| **Vite** | 7.0.4 | Frontend build tool |
| **@vitejs/plugin-react** | 4.6.0 | React support for Vite |
| **Tauri CLI** | 2.x | Desktop app building |
| **TypeScript** | 5.8.3 | Type checking |
| **Rust** | (latest stable) | Backend compilation |

### Type Definitions

| Package | Version | Purpose |
|---------|---------|---------|
| **@types/react** | 19.1.8 | React type definitions |
| **@types/react-dom** | 19.1.6 | React DOM type definitions |

### Package Manager

| Tool | Purpose |
|------|---------|
| **npm** | Dependency management (can use pnpm/yarn) |

---

## Architecture

### Application Structure

```
┌─────────────────────────────────────────────────────────────┐
│                     Scratch Desktop App                      │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                 Frontend (React 19)                   │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Components                                     │  │  │
│  │  │  • Editor (TipTap)                              │  │  │
│  │  │  • Sidebar, NoteList, FolderTreeView           │  │  │
│  │  │  • Settings, CommandPalette                     │  │  │
│  │  │  • AI Edit Modal, Git Status                   │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Context (State Management)                     │  │  │
│  │  │  • NotesContext (notes, CRUD, search)           │  │  │
│  │  │  • GitContext (git operations)                  │  │  │
│  │  │  • ThemeContext (theme, typography)             │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Services (Tauri Command Wrappers)              │  │  │
│  │  │  • notes.ts, git.ts, ai.ts                      │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           │ Tauri IPC Bridge                 │
│                           │ (invoke / events)                │
│                           ▼                                  │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                 Backend (Rust)                        │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Tauri Commands                                 │  │  │
│  │  │  • Note management (CRUD, search)               │  │  │
│  │  │  • File operations, folder management           │  │  │
│  │  │  • Git operations, AI CLI wrappers              │  │  │
│  │  │  • Clipboard, dialogs, updater                  │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  Core Services                                  │  │  │
│  │  │  • File watcher (notify with debouncing)        │  │  │
│  │  │  • Search index (Tantivy)                       │  │  │
│  │  │  • State management                             │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

```
User Action → React Component → Context/Service → Tauri.invoke()
                                                        │
                                                        ▼
                                               Rust Command Handler
                                                        │
                                                        ▼
                                               File System / Git / CLI
                                                        │
                                                        ▼
                                               Result → Frontend
```

### State Management Pattern

**Dual Context Pattern (for performance):**
- Data context: Holds state (notes, selectedNote, etc.)
- Actions context: Holds functions (createNote, saveNote, etc.)

This prevents unnecessary re-renders when actions change.

---

## File Structure

```
scratch/
├── src/                              # React Frontend
│   ├── components/
│   │   ├── editor/                   # TipTap editor components
│   │   │   ├── Editor.tsx            # Main editor (auto-save, copy-as, focus mode)
│   │   │   ├── LinkEditor.tsx        # Inline link add/edit popup
│   │   │   ├── CodeBlockView.tsx     # Code block NodeView with language selector
│   │   │   ├── MermaidRenderer.tsx   # Mermaid SVG rendering
│   │   │   ├── lowlight.ts           # Lowlight instance with 20 languages
│   │   │   ├── SlashCommand.tsx      # Slash command extension
│   │   │   └── SlashCommandList.tsx  # Slash command popup
│   │   ├── layout/                   # Layout components
│   │   │   ├── Sidebar.tsx           # Note list, search, git status, DnD
│   │   │   └── FolderPicker.tsx      # Initial folder selection dialog
│   │   ├── notes/                    # Note management UI
│   │   │   ├── NoteList.tsx          # Scrollable note list with context menu
│   │   │   ├── FolderTreeView.tsx    # Collapsible folder tree with drag-and-drop
│   │   │   └── FolderNameDialog.tsx  # Create/rename folder dialog
│   │   ├── command-palette/
│   │   │   └── CommandPalette.tsx    # Cmd+P command palette
│   │   ├── settings/                 # Settings page
│   │   │   ├── SettingsPage.tsx      # Tabbed settings interface
│   │   │   ├── GeneralSettingsSection.tsx
│   │   │   ├── AppearanceSettingsSection.tsx
│   │   │   ├── ShortcutsSettingsSection.tsx
│   │   │   └── AboutSettingsSection.tsx
│   │   ├── ai/                       # AI features
│   │   │   ├── AiEditModal.tsx       # AI prompt input modal
│   │   │   └── AiResponseToast.tsx   # AI response with undo
│   │   ├── git/
│   │   │   └── GitStatus.tsx         # Floating git status with commit UI
│   │   ├── ui/                       # Shared UI components
│   │   │   ├── Button.tsx
│   │   │   ├── Input.tsx
│   │   │   ├── Tooltip.tsx
│   │   │   └── index.tsx             # ListItem, CommandItem, ToolbarButton
│   │   └── icons/                    # SVG icon components
│   │       └── index.tsx             # 30+ icon exports
│   ├── context/                      # React Context providers
│   │   ├── NotesContext.tsx          # Note CRUD, search, file watching
│   │   ├── GitContext.tsx            # Git operations wrapper
│   │   └── ThemeContext.tsx          # Theme, typography, settings
│   ├── lib/                          # Utilities
│   │   ├── utils.ts                  # cn() className merging
│   │   └── folderTree.ts             # Build folder tree from flat list
│   ├── services/                     # Tauri command wrappers
│   │   ├── notes.ts                  # Note management commands
│   │   ├── git.ts                    # Git commands
│   │   └── ai.ts                     # AI CLI wrappers
│   ├── types/
│   │   └── note.ts                   # TypeScript types
│   ├── App.tsx                       # Main app component
│   ├── App.css                       # Global styles
│   └── main.tsx                      # React root & providers
│
├── src-tauri/                        # Rust Backend
│   ├── src/
│   │   ├── lib.rs                    # Main Tauri commands, state, file watcher, search
│   │   └── git.rs                    # Git CLI wrapper
│   ├── capabilities/
│   │   └── default.json              # Tauri v2 permissions config
│   ├── Cargo.toml                    # Rust dependencies
│   ├── tauri.conf.json               # Tauri app configuration
│   └── build.rs                      # Build script
│
├── .github/workflows/
│   ├── ci.yml                        # CI: build validation on push/PR
│   └── release.yml                   # Release: multi-platform build + publish
│
├── public/                           # Static assets
├── package.json                      # Node dependencies & scripts
├── tsconfig.json                     # TypeScript config
├── vite.config.ts                    # Vite config
├── tailwind.config.js                # Tailwind config
├── .gitignore                        # Git ignore rules
└── README.md                         # Project documentation
```

---

## Key Technologies Deep Dive

### TipTap Editor

**Why TipTap?**
- Headless editor built on ProseMirror
- Excellent markdown support
- Rich extension ecosystem
- Framework-agnostic (great React integration)
- Active community and maintenance

**Key Features Implemented:**
- Markdown bidirectional conversion
- Tables with context menu (insert/delete rows, columns, merge cells)
- Task lists with checkboxes
- Code blocks with syntax highlighting (20 languages)
- Math equations via KaTeX
- Mermaid diagrams with Edit/Preview toggle
- Inline link editor (`Cmd+K`)
- Slash commands for quick block insertion
- Find in note (`Cmd+F`) with highlighting
- Markdown source mode (`Cmd+Shift+M`)
- Focus mode (`Cmd+Shift+Enter`)

### Tantivy Search

**Why Tantivy?**
- Rust port of Lucene (battle-tested search engine)
- Fast full-text search
- Prefix queries for autocomplete
- No external dependencies
- Runs entirely offline

**Schema:**
```rust
Schema {
    id: STRING(indexed),
    title: TEXT(tokenized),
    content: TEXT(tokenized),
    modified: INTEGER(indexed),
}
```

**Query Strategy:**
1. Try full-text search with phrase queries
2. Fall back to prefix search (query*) for partial matches
3. Final fallback to cache-based title/content matching

### File Watching (notify crate)

**Custom Debouncing Implementation:**
- 500ms debounce per file
- Prevents multiple events for rapid changes
- Recent-save tracking to ignore own file watcher events
- 5-second retention for debounce map cleanup

### Git Integration

**Approach:** CLI wrapper (not libgit2)
- Simpler for this use case
- Leverages user's git installation
- 8 commands: status, init, add, commit, push, add_remote, push_with_upstream, is_available

**Features:**
- Optional integration (off by default)
- Inline commit UI in sidebar
- Auto-refresh on file changes (1000ms debounce)

### Tauri v2

**Why Tauri over Electron?**
- Rust backend instead of Node.js
- System webview instead of bundled Chromium
- ~10x smaller bundle size
- Better performance and lower memory usage
- Capability-based security model

**Key v2 Features Used:**
- Plugin system for extended functionality
- Enhanced security with capabilities
- Improved window management
- Better updater integration

---

## Commands & Scripts

### NPM Scripts

```json
{
  "dev": "vite",                    // Start Vite dev server only
  "build": "tsc && vite build",     // Build frontend (type check + bundle)
  "preview": "vite preview",        // Preview production build
  "tauri": "tauri"                  // Tauri CLI proxy
}
```

### Common Commands

```bash
# Development
npm run dev                    # Vite dev server only
npm run tauri dev              # Full app in development mode

# Building
npm run build                  # Build frontend only
npm run tauri build            # Build production app bundle

# Type checking
tsc --noEmit                  // Type check without emitting files
```

### Keyboard Shortcuts (macOS)

| Shortcut | Action |
|----------|--------|
| `Cmd+N` | New note |
| `Cmd+P` | Command palette |
| `Cmd+K` | Add/edit link (when in editor) |
| `Cmd+F` | Find in current note |
| `Cmd+Shift+C` | Copy & Export menu |
| `Cmd+Shift+M` | Toggle markdown source mode |
| `Cmd+Shift+Enter` | Toggle focus mode |
| `Cmd+Shift+F` | Search notes |
| `Cmd+R` | Reload current note |
| `Cmd+,` | Open settings |
| `Cmd+1/2/3/4` | Switch settings tabs |
| `Cmd+\` | Toggle sidebar |
| `Cmd+B/I` | Bold/Italic |
| `Cmd+=/-/0` | Zoom in/out/reset |

*(On Windows/Linux, use `Ctrl` instead of `Cmd`)*

---

## CI/CD

### CI Workflow (`.github/workflows/ci.yml`)

**Triggers:** Push to `main`, pull requests

**Jobs:**
- Frontend validation: `tsc` type check + Vite build
- Backend validation: `cargo check` + `cargo clippy`
- Runs on Ubuntu runner
- Does NOT build Tauri app bundle

### Release Workflow (`.github/workflows/release.yml`)

**Triggers:** `v*` tag push, manual `workflow_dispatch`

**Platforms:** Builds in parallel
- macOS: Universal binary (arm64 + x86_64), code-signed and notarized
- Windows: NSIS installer (x64)
- Linux: AppImage and .deb

**Creates:**
- Draft GitHub release with all artifacts
- `latest.json` for auto-updater
- Manual review and publish required

---

## Build & Release

### Local Build (Manual)

**macOS Universal Binary:**
```bash
source .env.build  # Load signing environment variables
npm run tauri build -- --target universal-apple-darwin
```

**Windows:**
```bash
npm run tauri build
```

### Release Process

1. Bump version in `package.json` and `src-tauri/tauri.conf.json`
2. Commit version bump to `main`
3. Tag and push: `git tag v0.5.0 && git push origin v0.5.0`
4. CI builds all platforms (~20-30 min)
5. Review draft release on GitHub
6. Edit release notes and publish

### GitHub Secrets Required

| Secret | Purpose |
|--------|---------|
| `APPLE_CERTIFICATE` | Base64-encoded .p12 export of Developer ID certificate |
| `APPLE_CERTIFICATE_PASSWORD` | Password for the .p12 file |
| `APPLE_SIGNING_IDENTITY` | Developer ID Application identity |
| `APPLE_ID` | Apple Developer account email |
| `APPLE_PASSWORD` | App-specific password for notarization |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `TAURI_SIGNING_PRIVATE_KEY` | Tauri updater signing key contents |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for Tauri signing key |

### Auto-Updater

- Checks for updates via Tauri updater plugin
- Endpoint: `https://github.com/erictli/scratch/releases/latest/download/latest.json`
- Triggers: On startup (after 3s delay), manually via Settings
- Toast notification with "Update Now" button if update available
- Configured in `src-tauri/tauri.conf.json` under `plugins.updater`

---

## Performance Optimizations

| Area | Optimization |
|------|--------------|
| Auto-save | 300ms debounce |
| Search | 150ms debounce (sidebar) |
| File watcher | 500ms debounce per file |
| Git status | 1000ms debounce |
| Components | React.memo for expensive renders |
| Functions | useCallback/useMemo for critical paths |
| State | Dual context pattern (data/actions separated) |
| Search engine | Tantivy (Rust, faster than JS solutions) |

---

## Security Model

### Tauri v2 Capabilities

Permissions defined in `src-tauri/capabilities/default.json`:
- Core: `core:path:default`, `core:event:default`, etc.
- File system read/write for notes folder
- Dialog (folder picker)
- Clipboard access
- Shell (for git commands)
- Window management
- App scope (open URLs)

### Data Privacy

- 100% offline by default
- No telemetry or analytics
- No cloud dependencies
- All data stored locally on user's machine
- Optional features (git push, updates) require explicit user action

---

## Future Considerations

### Potential Upgrades
- Tauri Mobile support (when stable)
- Native AI integration (vs CLI wrappers)
- CRDT-based sync for collaborative editing
- Database alternative for large note sets (SQLite)

### Technology Debt
- None significant - codebase is clean and modern
- Regular dependency updates recommended
- Monitor for TipTap v4 updates
- Track Tauri mobile progress

---

**Document Status:** Current - Reflects codebase as of 2026-04-01
