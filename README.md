# arictl

**arictl** (蟻) — lightweight macOS system cleanup tool. Scan, preview, and move cache files, project build artifacts, and other cleanable disk space to Trash.

```bash
# Quick start
arictl scan                          # scan all (table output)
arictl scan --json                   # machine-readable JSON
arictl scan --categories purge       # only project build artifacts

arictl clean --all                   # scan → confirm → clean in one step
arictl clean --interactive            # interactive checklist (Space to toggle)
arictl clean --categories purge -y    # skip confirmation (for scripts)

arictl whitelist list                # see protected paths
arictl whitelist add "~/important"   # protect a path
```

## Features

- **🛡️ Trash-first, not rm**: filesystem deletions go to macOS Trash (`~/.Trash`) — recoverable via Finder, never permanently lost
- **👤 Human-friendly default**: `scan` outputs a colored table; add `--json` for machine-readable output
- **⚡ One-step cleanup**: `arictl clean --all` chains scan → confirm → run; `-y` skips prompts
- **9 categories**, 240+ cleanup rules covering system caches, browser caches, developer tools, app caches, project build artifacts, cloud storage, code editors, and more
- **Two modes**: direct CLI execution and `--rpc` stdio pipe mode (JSON-RPC 2.0)
- **Native commands preferred**: uses `brew cleanup`, `npm cache clean --force`, `pip3 cache purge`, `go clean -cache`, `docker system prune`, `conda clean` etc. where possible, instead of raw filesystem deletion
- **Safety first**: Trash recovery, glob-based whitelist, project indicator verification, min-age guards, exclude indicators
- **User-extensible rules**: drop `.toml` files in `~/.config/arictl/rules/` to override any built-in category
- **Fast**: size calculation via `du -sb` (kernel-level), parallel discovery with rayon
- **Single static binary**: no daemon, no socket file, no background process

## Install

```bash
# From source
cargo install --git https://github.com/agent-rt/arictl

# Or build locally
git clone https://github.com/agent-rt/arictl && cd arictl
cargo build --release
cp target/release/arictl ~/local/bin/
```

## Usage

### Scan

```bash
# Human-friendly table output (default)
arictl scan

# Filter by category
arictl scan --categories purge
arictl scan --categories brew dev_tools

# Machine-readable JSON (for scripts / jq)
arictl scan --json
arictl scan --categories brew --json | jq '.items[] | {label, size}'
```

### Preview & Run

```bash
# Preview specific items by ID (from scan output)
arictl preview brew npm --json   # JSON for machines
arictl preview brew npm          # human-friendly table

# Execute deletion (moves to Trash)
arictl run brew npm pip
```

Item IDs are shown in scan output (`"id": "brew"`).

### One-Step Clean

```bash
# Scan all → show preview → prompt → clean
arictl clean --all

# Interactive checklist (scan table → Space to select → Enter to run)
arictl clean --interactive
arictl clean -i --categories brew

# Scan specific categories, skip confirmation (for scripts)
arictl clean --categories purge -y

# Full automation
arictl clean --all -y
```

`clean` chains `scan` → `preview` → confirmation prompt → `run`. Add `-y` to skip all prompts for scripting/agent use.

With `--interactive` / `-i`, the scan results are displayed as a table, then a terminal checklist lets you pick items with `Space`. `Enter` runs the selected items directly — no additional text prompts.

### Whitelist

```bash
arictl whitelist list
arictl whitelist add "~/Documents/important"
arictl whitelist remove "~/Documents/important"
```

### RPC Mode (stdio)

```bash
echo '{"jsonrpc":"2.0","method":"clean.scan","params":{"categories":["purge"]},"id":1}' \
  | arictl --rpc
```

Streaming progress notifications (`$scan.progress`) are emitted during long scans.

## Categories

| Category ID | Label | Items |
|-------------|-------|-------|
| `app_cache` | Application Caches | 60 | macOS system caches, communication apps (Slack, Discord, WeChat, Teams, Telegram, Zoom…), design tools (Sketch, Figma, Adobe), media players (Spotify, Apple Music, NetEase…), gaming (Steam, Epic, Battle.net, Minecraft), video tools, download managers, remote desktop, more |
| `brew` | Homebrew | 1 | `brew cleanup` |
| `browser` | Browser Cache | 13 | Safari, Chrome, Chromium, Firefox, Brave, Edge, Opera, Vivaldi, Yandex, Arc, Zen Browser, QQ Browser, Safari Safe Browsing |
| `cloud_office` | Cloud Storage & Office | 12 | Dropbox, Google Drive, OneDrive, Baidu Netdisk, Aliyun Drive, Box, Word, Excel, PowerPoint, Outlook, iWork, WPS |
| `dev_tools` | Developer Tools | 97 | npm, pnpm, yarn, bun, corepack, pip, uv, conda, poetry, ruff, mypy, pytest, pyenv, jupyter, huggingface, torch, tensorflow, wandb, cargo, rustup, go, gradle, maven, sbt, ivy, Xcode (DerivedData, simulators, device support, IB, logs), Android Studio, CocoaPods, Expo, Docker, Nix, JetBrains, Terraform, kube, AWS, gcloud, Azure, Playwright, CI/CD caches, database tool caches, API tool caches, shell/VCS caches, more |
| `editor` | Code Editor Caches | 16 | VS Code (7 sub-items), Cursor (5), Zed, Sublime Text, Copilot |
| `purge` | Project Build Artifacts | 23 | `target/`, `node_modules/`, `build/`, `Pods/`, `.next/`, `.nuxt/`, `.svelte-kit/`, `.astro/`, `.turbo/`, `.dart_tool/`, `.zig-cache/`, `.angular/`, `.build/`, `.expo/`, `.venv/`, `venv/`, `.tox/`, `.nox/`, `vendor/`, `vendor/bundle/`, `zig-out/`, `.cxx/`, `.output/` — all with project indicator + min_age validation |
| `system` | System Caches & Logs | 13 | `/Library/Caches`, `/private/tmp`, `/private/var/tmp`, `/private/var/log`, `/Library/Logs/DiagnosticReports`, `/Library/Updates`, `/macOS Install Data`, GPU caches, system diagnostics, power logs |
| `user` | User Essentials | 8 | Trash, Recent Items, Mail Downloads, Messages Sticker Cache, Incomplete Downloads, Darwin runtime dirs, iOS firmware (IPSW), `.DS_Store` |

### User Rules

Override or extend built-in rules by creating TOML files in `~/.config/arictl/rules/`:

```toml
category_id = "purge"
category_label = "Project Build Artifacts"

[[items]]
id = "my_python_venvs"
label = "Python .venv directories (my stricter rules)"
paths = ["~"]
strategy = "remove_subdirs"
strategy_args = { subdir = ".venv", indicators = ["pyproject.toml"], min_age_days = 14 }
min_size = 104857600
```

If a user category has the same `category_id` as a built-in, it completely replaces the built-in.

## RPC Protocol

Newline-delimited JSON-RPC 2.0 on stdin/stdout.

**Request:**
```json
{"jsonrpc":"2.0","method":"clean.scan","params":{"categories":["brew"]},"id":1}
```

**Response:**
```json
{"jsonrpc":"2.0","result":{...},"id":1}
```

**Progress notifications (no `id` field):**
```json
{"jsonrpc":"2.0","method":"$scan.progress","params":{"category":"purge","status":"start"}}
{"jsonrpc":"2.0","method":"$scan.progress","params":{"category":"purge","status":"item","item":"Rust target/ directories"}}
{"jsonrpc":"2.0","method":"$scan.progress","params":{"category":"purge","status":"done","size":123456789}}
```

### Methods

| Method | Params | Returns |
|--------|--------|---------|
| `clean.scan` | `{ categories?: string[] }` | `ScanResult` |
| `clean.preview` | `string[]` (item IDs) | `PreviewResult` |
| `clean.run` | `string[]` (item IDs) | `DeleteResult` |
| `clean.whitelist` | `{ action: "list" \| "add" \| "remove", pattern?: string }` | varies |
| `server.status` | none | `ServerStatus` |

## Safety

1. **🗑️ Trash recovery**: every filesystem deletion moves items to macOS Trash (`~/.Trash`) via the native `trash` crate (NSFileManager). Items appear in Finder Trash and can be put back. Native command execution (brew, npm, conda, etc.) uses the tool's own safe cleanup routines and is unaffected.
2. **Glob whitelist**: paths matching whitelist patterns are protected from deletion
3. **Project indicators**: `node_modules/` is only cleaned if adjacent `package-lock.json`/`yarn.lock`/`pnpm-lock.yaml` exists
4. **Exclude indicators**: `vendor/` is skipped if `Gemfile` or `go.mod` exists (Rails/Go vendor)
5. **Min age**: build artifacts younger than 7 days (configurable) are skipped
6. **Dry run**: `preview` shows what would be deleted without executing
7. **Hidden dir exclusion**: purge scan skips hidden directories at depth 1-2

## Performance

| Scan | Time |
|------|------|
| `app_cache` | <0.1s |
| `brew` | <0.1s |
| `browser editor cloud_office` | ~0.2s |
| `dev_tools` | ~9s |
| `purge` (full home scan) | ~29s |
| `system` | varies (I/O bound) |

Size calculation uses `du -sb` (kernel-level `getattrlist` on APFS). Multi-directory discovery uses parallel `du` via rayon.

## Development

```bash
git clone https://github.com/agent-rt/arictl
cd arictl
cargo build
cargo test
```

Project structure:

```
arictl/
├── Cargo.toml              # workspace root
├── apps/
│   └── arictl-cli/         # binary entry point (CLI + RPC)
│       └── src/main.rs
├── crates/
│   └── arictl-core/        # library: engine, rules, scanner, deleter
│       ├── rules/           # built-in TOML rule files (9 categories)
│       └── src/
│           ├── clean/
│           │   ├── engine.rs    # scan/preview/run orchestrator
│           │   ├── scanner.rs   # du-based size calculation + subdir discovery
│           │   ├── deleter.rs   # Trash + native command execution
│           │   ├── rules.rs     # TOML rule loading (user + embedded)
│           │   ├── model.rs     # CleanStrategy, CleanItemDef, CleanCategory
│           │   ├── whitelist.rs # glob-based path protection
│           │   └── mod.rs
│           ├── rpc.rs       # JSON-RPC 2.0 types
│           ├── types.rs     # shared types (ScanResult, DeleteResult, etc.)
│           └── lib.rs
```

## License

MIT
