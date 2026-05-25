use std::io::Write;
use std::sync::Mutex;

use clap::{Parser, Subcommand};
use colored::Colorize;

use arictl_core::clean::engine::Engine;
use arictl_core::rpc::{Request, Response};
use arictl_core::types::ServerStatus;

mod display;

#[derive(Parser)]
#[command(version, about = "macOS system cleanup tool")]
struct Cli {
    /// RPC mode: read JSON-RPC 2.0 from stdin, write responses to stdout
    #[arg(long, global = true)]
    rpc: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Scan for cleanable items
    Scan {
        /// Filter by category (e.g. purge, dev_tools)
        #[arg(long)]
        categories: Vec<String>,

        /// Output raw JSON instead of human-readable table
        #[arg(long)]
        json: bool,
    },
    /// Preview deletion of specified items
    Preview {
        ids: Vec<String>,
        /// Output raw JSON
        #[arg(long)]
        json: bool,
    },
    /// Execute deletion of specified items
    Run {
        ids: Vec<String>,
    },
    /// Scan and clean in one step
    Clean {
        /// Clean all items across all categories
        #[arg(long)]
        all: bool,

        /// Filter by category (e.g. purge, dev_tools)
        #[arg(long)]
        categories: Vec<String>,

        /// Skip confirmation prompt
        #[arg(long, short)]
        yes: bool,

        /// Interactive checklist (select items with Space)
        #[arg(long, short)]
        interactive: bool,
    },
    /// Manage whitelist
    Whitelist {
        #[command(subcommand)]
        action: WhitelistAction,
    },
    /// Print JSON-RPC schema (for agent/LLM consumption)
    Schema,
}

#[derive(Subcommand)]
enum WhitelistAction {
    /// List protected patterns
    List,
    /// Add a protected pattern
    Add { pattern: String },
    /// Remove a protected pattern
    Remove { pattern: String },
}

fn main() {
    let cli = Cli::parse();

    if cli.rpc {
        run_rpc_mode();
    } else if let Some(cmd) = cli.command {
        run_direct(cmd);
    } else {
        clap::Command::new("arictl")
            .print_help()
            .ok();
    }
}

fn run_direct(cmd: Command) {
    let mut engine = Engine::new();

    match cmd {
        Command::Scan { categories, json } => {
            let result = if categories.is_empty() {
                engine.scan(None)
            } else {
                engine.scan_filtered(&categories, None)
            };
            if json {
                print_json(&result);
            } else {
                display::print_scan(&result);
            }
        }
        Command::Preview { ids, json } => {
            let result = engine.preview(&ids);
            if json {
                print_json(&result);
            } else {
                display::print_preview(&result);
            }
        }
        Command::Run { ids } => {
            let result = engine.run(&ids);
            display::print_delete(&result);
        }
        Command::Clean { all, categories, yes, interactive } => {
            eprint!("  Scanning");
            if !categories.is_empty() {
                let _ = writeln!(std::io::stderr(), " [{}]...", categories.join(", "));
            } else {
                let _ = writeln!(std::io::stderr(), " all categories...");
            }

            let result = if all || !categories.is_empty() {
                engine.scan_filtered(&categories, None)
            } else {
                engine.scan(None)
            };

            if result.items.is_empty() {
                println!("\n  {} Nothing to clean.\n", "✓".green());
                return;
            }

            if interactive {
                display::print_scan(&result);
                let ids = interactive_select(&result);
                if ids.is_empty() {
                    println!("  {}\n", "Cancelled.".dimmed());
                    return;
                }
                let del_result = engine.run(&ids);
                display::print_delete(&del_result);
                return;
            }

            display::print_scan(&result);

            if !yes {
                print!("  {}  ", "Proceed with cleanup?".white().bold());
                std::io::stdout().flush().ok();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    println!("  {}\n", "Cancelled.".dimmed());
                    return;
                }
            }

            let ids: Vec<String> = result.items.iter().map(|i| i.id.clone()).collect();
            let preview = engine.preview(&ids);
            display::print_preview(&preview);

            if !yes {
                print!("  {}  ", "Confirm deletion?".white().bold());
                std::io::stdout().flush().ok();
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).ok();
                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    println!("  {}\n", "Cancelled.".dimmed());
                    return;
                }
            }

            let del_result = engine.run(&ids);
            display::print_delete(&del_result);
        }
        Command::Schema => {
            print_json(&rpc_schema());
        }
        Command::Whitelist { action } => match action {
            WhitelistAction::List => {
                let patterns = engine.whitelist().patterns().to_vec();
                for p in &patterns {
                    println!("  {}", p);
                }
                if patterns.is_empty() {
                    println!("  (no patterns)");
                }
            }
            WhitelistAction::Add { pattern } => {
                engine.whitelist_mut().add_pattern(pattern.clone());
                println!("  {} added: {}", "✓".green(), pattern);
            }
            WhitelistAction::Remove { pattern } => {
                engine.whitelist_mut().remove_pattern(&pattern);
                println!("  {} removed: {}", "✓".green(), pattern);
            }
        },
    }
}

fn interactive_select(result: &arictl_core::types::ScanResult) -> Vec<String> {
    use dialoguer::MultiSelect;

    if result.items.is_empty() {
        return Vec::new();
    }

    let choices: Vec<String> = result
        .items
        .iter()
        .map(|item| {
            format!(
                "[{}] {} ({})",
                item.category,
                item.label,
                display::format_size(item.size),
            )
        })
        .collect();

    let defaults: Vec<bool> = vec![true; choices.len()];

    let selections = MultiSelect::new()
        .with_prompt("Select items to clean (Space to toggle, Enter to confirm)")
        .items(&choices)
        .defaults(&defaults)
        .interact()
        .unwrap_or_default();

    selections
        .iter()
        .map(|&i| result.items[i].id.clone())
        .collect()
}

fn rpc_schema() -> serde_json::Value {
    serde_json::json!({
        "openrpc": "1.0.0-rc1",
        "info": {
            "title": "arictl RPC",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "macOS system cleanup tool — JSON-RPC 2.0 over stdio. Send one JSON-RPC request per line, read one response per line. Notifications have no id field."
        },
        "methods": [
            {
                "name": "server.status",
                "summary": "Get server status and version info",
                "params": [],
                "result": {
                    "description": "ServerStatus: version, pid, uptime_secs, operations_count, addr"
                }
            },
            {
                "name": "rpc.discover",
                "summary": "Get this schema",
                "params": [],
                "result": {
                    "description": "OpenRPC schema describing all available methods"
                }
            },
            {
                "name": "clean.scan",
                "summary": "Scan for cleanable items across categories",
                "params": [
                    {
                        "name": "categories",
                        "schema": {"type": "array", "items": {"type": "string"}},
                        "required": false,
                        "summary": "Filter by category IDs (omit for all)"
                    }
                ],
                "result": {
                    "description": "ScanResult: items[], total_size, categories map"
                }
            },
            {
                "name": "clean.preview",
                "summary": "Preview deletion of specific items",
                "params": [
                    {
                        "name": "item_ids",
                        "schema": {"type": "array", "items": {"type": "string"}},
                        "required": true,
                        "summary": "Item IDs to preview (from scan result)"
                    }
                ],
                "result": {
                    "description": "PreviewResult: items[] (will_delete, size, path), protected_skipped[]"
                }
            },
            {
                "name": "clean.run",
                "summary": "Execute deletion of specific items (moves to Trash)",
                "params": [
                    {
                        "name": "item_ids",
                        "schema": {"type": "array", "items": {"type": "string"}},
                        "required": true,
                        "summary": "Item IDs to delete (from scan result)"
                    }
                ],
                "result": {
                    "description": "DeleteResult: completed[], failed[], total_freed"
                }
            },
            {
                "name": "clean.whitelist",
                "summary": "Manage whitelist patterns",
                "params": [
                    {
                        "name": "action",
                        "schema": {"type": "string", "enum": ["list", "add", "remove"]},
                        "required": true,
                        "summary": "list → show patterns, add → protect a path, remove → unprotect"
                    },
                    {
                        "name": "pattern",
                        "schema": {"type": "string"},
                        "required": false,
                        "summary": "Glob pattern (required for add/remove)"
                    }
                ],
                "result": {
                    "description": "list → {\"patterns\": [...]}, add/remove → {\"status\": \"added|removed\", \"pattern\": \"...\"}"
                }
            }
        ],
        "notifications": [
            {
                "name": "$scan.progress",
                "summary": "Progress notification emitted during clean.scan for long-running scans",
                "params": [
                    {"name": "category", "schema": {"type": "string"}, "summary": "Category ID being scanned"},
                    {"name": "status", "schema": {"type": "string", "enum": ["start", "item", "done"]}, "summary": "Scan phase"},
                    {"name": "item", "schema": {"type": "string"}, "required": false, "summary": "Item label being evaluated (when status=item)"},
                    {"name": "size", "schema": {"type": "integer"}, "required": false, "summary": "Cumulative category size (when status=done)"}
                ]
            }
        ]
    })
}

fn print_json(value: &impl serde::Serialize) {
    if let Ok(json) = serde_json::to_string_pretty(value) {
        println!("{json}");
    }
}

// ── RPC mode (stdio) ──────────────────────────────────

fn run_rpc_mode() {
    let engine = Mutex::new(Engine::new());
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut line = String::new();

    loop {
        line.clear();
        let n = stdin.read_line(&mut line).unwrap_or(0);
        if n == 0 {
            break;
        }

        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::err(None, -32700, format!("parse error: {e}"));
                writeln_json(&stdout, &resp);
                continue;
            }
        };

        let id = req.id.clone();
        let result = dispatch_rpc(&req, &engine);
        let resp = match result {
            Ok(value) => Response::ok(id, value),
            Err(msg) => Response::err(id, -32000, msg),
        };

        writeln_json(&stdout, &resp);
    }
}

fn dispatch_rpc(req: &Request, engine: &Mutex<Engine>) -> Result<serde_json::Value, String> {
    match req.method.as_str() {
        "rpc.discover" => {
            Ok(rpc_schema())
        }
        "server.status" => {
            let status = ServerStatus {
                version: env!("CARGO_PKG_VERSION").into(),
                pid: std::process::id(),
                uptime_secs: 0,
                operations_count: 0,
                addr: "stdio".into(),
            };
            serde_json::to_value(status).map_err(|e| e.to_string())
        }
        "clean.scan" => {
            #[derive(serde::Deserialize)]
            struct ScanParams {
                categories: Option<Vec<String>>,
            }
            let params: ScanParams = req.params.as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(ScanParams { categories: None });

            let e = engine.lock().map_err(|e| e.to_string())?;
            let categories = params.categories.unwrap_or_default();
            let on_progress = |category: &str, status: &str, item: Option<&str>, size: Option<u64>| {
                let mut params = serde_json::json!({
                    "category": category,
                    "status": status,
                });
                if let Some(item) = item {
                    params["item"] = serde_json::json!(item);
                }
                if let Some(size) = size {
                    params["size"] = serde_json::json!(size);
                }
                let notif = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "$scan.progress",
                    "params": params,
                });
                let mut out = std::io::stdout().lock();
                let _ = writeln!(out, "{}", serde_json::to_string(&notif).unwrap_or_default());
                let _ = out.flush();
            };
            serde_json::to_value(e.scan_filtered(&categories, Some(&on_progress))).map_err(|e| e.to_string())
        }
        "clean.preview" => {
            let ids: Vec<String> = req.params.as_ref()
                .and_then(|p| serde_json::from_value(p.clone()).ok())
                .ok_or_else(|| "expected params to be an array of item IDs".to_string())?;
            let e = engine.lock().map_err(|e| e.to_string())?;
            serde_json::to_value(e.preview(&ids)).map_err(|e| e.to_string())
        }
        "clean.run" => {
            let ids: Vec<String> = req.params.as_ref()
                .and_then(|p| serde_json::from_value(p.clone()).ok())
                .ok_or_else(|| "expected params to be an array of item IDs".to_string())?;
            let e = engine.lock().map_err(|e| e.to_string())?;
            serde_json::to_value(e.run(&ids)).map_err(|e| e.to_string())
        }
        "clean.whitelist" => {
            #[derive(serde::Deserialize)]
            struct Params { action: String, pattern: Option<String> }
            let p: Params = req.params.as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .ok_or_else(|| "invalid whitelist params".to_string())?;
            match p.action.as_str() {
                "list" => {
                    let e = engine.lock().map_err(|e| e.to_string())?;
                    let patterns = e.whitelist().patterns().to_vec();
                    Ok(serde_json::json!({ "patterns": patterns }))
                }
                "add" => {
                    let pat = p.pattern.ok_or_else(|| "missing pattern".to_string())?;
                    let mut e = engine.lock().map_err(|e| e.to_string())?;
                    e.whitelist_mut().add_pattern(pat.clone());
                    Ok(serde_json::json!({ "status": "added", "pattern": pat }))
                }
                "remove" => {
                    let pat = p.pattern.ok_or_else(|| "missing pattern".to_string())?;
                    let mut e = engine.lock().map_err(|e| e.to_string())?;
                    e.whitelist_mut().remove_pattern(&pat);
                    Ok(serde_json::json!({ "status": "removed", "pattern": pat }))
                }
                _ => Err(format!("unknown whitelist action: {}", p.action)),
            }
        }
        _ => Err(format!("unknown method: {}", req.method)),
    }
}

fn writeln_json(stdout: &std::io::Stdout, value: &impl serde::Serialize) {
    let mut out = stdout.lock();
    if let Ok(json) = serde_json::to_string(value) {
        let _ = writeln!(out, "{json}");
        let _ = out.flush();
    }
}
