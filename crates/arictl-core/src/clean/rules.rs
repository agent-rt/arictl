use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use super::model::{CleanCategory, CleanItemDef, CleanStrategy};

#[derive(Debug, Deserialize)]
struct RuleItemDef {
    id: String,
    label: String,
    paths: Vec<String>,
    strategy: String,
    strategy_args: Option<HashMap<String, toml::Value>>,
    min_size: Option<u64>,
    command: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RuleFile {
    category_id: String,
    category_label: String,
    items: Vec<RuleItemDef>,
}

fn parse_strategy(kind: &str, args: &Option<HashMap<String, toml::Value>>) -> Result<CleanStrategy, String> {
    match kind {
        "remove_contents" => Ok(CleanStrategy::RemoveContents),
        "remove_dir" => Ok(CleanStrategy::RemoveDir),
        "remove_if_older" => {
            let days = args
                .as_ref()
                .and_then(|m| m.get("days"))
                .and_then(|v| v.as_integer())
                .unwrap_or(7) as u32;
            Ok(CleanStrategy::RemoveFilesOlderThan(days))
        }
        "remove_subdirs" => {
            let args_map = args.as_ref().ok_or_else(|| "remove_subdirs requires strategy_args".to_string())?;
            let subdir = args_map.get("subdir")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "remove_subdirs requires strategy_args.subdir".to_string())?
                .to_string();
            let indicators = args_map.get("indicators")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .or_else(|| args_map.get("indicator").and_then(|v| v.as_str()).map(|s| vec![s.to_string()]))
                .unwrap_or_default();
            let exclude_indicators = args_map.get("exclude_indicators")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let min_age_days = args_map.get("min_age_days")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u64;
            Ok(CleanStrategy::RemoveSubdirs { subdir, indicators, exclude_indicators, min_age_days })
        }
        other => Err(format!("unknown strategy: {other}")),
    }
}

fn parse_toml(content: &str) -> Option<CleanCategory> {
    let file: RuleFile = toml::from_str(content).ok()?;
    let items: Vec<CleanItemDef> = file
        .items
        .into_iter()
        .filter_map(|r| {
            let strategy = parse_strategy(&r.strategy, &r.strategy_args).ok()?;
            Some(CleanItemDef {
                id: r.id,
                label: r.label,
                paths: r.paths,
                strategy,
                min_size: r.min_size.unwrap_or(0),
                command: r.command,
            })
        })
        .collect();
    Some(CleanCategory {
        id: file.category_id,
        label: file.category_label,
        items,
    })
}

fn user_rules_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "~".into());
    PathBuf::from(home).join(".config/arictl/rules")
}

fn load_from(dir: &PathBuf) -> Vec<CleanCategory> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|content| parse_toml(&content))
        .collect()
}

fn load_embedded() -> Vec<CleanCategory> {
    const BUILTINS: &[(&str, &str)] = &[
        ("app_cache", include_str!("../../rules/app_cache.toml")),
        ("system", include_str!("../../rules/system.toml")),
        ("browser", include_str!("../../rules/browser.toml")),
        ("dev", include_str!("../../rules/dev.toml")),
        ("editor", include_str!("../../rules/editor.toml")),
        ("user", include_str!("../../rules/user.toml")),
        ("cloud_office", include_str!("../../rules/cloud_office.toml")),
        ("brew", include_str!("../../rules/brew.toml")),
        ("purge", include_str!("../../rules/purge.toml")),
    ];

    BUILTINS
        .iter()
        .filter_map(|(_name, content)| parse_toml(content))
        .collect()
}

/// Load rules: user files override builtins by category_id.
pub fn all_categories() -> Vec<CleanCategory> {
    let user = load_from(&user_rules_dir());
    let builtin = load_embedded();

    let user_ids: std::collections::HashSet<String> = user.iter().map(|c| c.id.clone()).collect();
    let mut result: Vec<CleanCategory> = user;

    for b in builtin {
        if !user_ids.contains(&b.id) {
            result.push(b);
        }
    }

    result
}
