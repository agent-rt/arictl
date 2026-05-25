use std::collections::HashMap;

use rayon::prelude::*;

use crate::types::{
    CleanAction, CleanItem, DeleteResult, DeleteResultItem, DeleteStatus, PreviewItem, PreviewResult,
    ScanResult, CategorySummary,
};

use super::deleter;
use super::model::{CleanCategory, CleanItemDef, CleanStrategy};
use super::rules;
use super::scanner;
use super::whitelist::Whitelist;

pub type ScanProgress = dyn Fn(&str, &str, Option<&str>, Option<u64>) + Send + Sync;

pub struct Engine {
    categories: Vec<CleanCategory>,
    whitelist: Whitelist,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            categories: rules::all_categories(),
            whitelist: Whitelist::new(),
        }
    }

    fn expand_paths_par(&self, paths: &[String]) -> (Vec<String>, Vec<u64>, u64) {
        let expanded: Vec<String> = paths.iter()
            .map(|p| shellexpand::tilde(p).to_string())
            .collect();

        let sizes: Vec<u64> = expanded
            .par_iter()
            .map(|p| scanner::calculate_size(p).unwrap_or(0))
            .collect();

        let total: u64 = sizes.iter().sum();
        (expanded, sizes, total)
    }

    /// Fast estimate using only first-level entries (avoids deep recursion).
    fn estimate_paths(&self, paths: &[String]) -> (Vec<String>, u64) {
        let expanded: Vec<String> = paths.iter()
            .map(|p| shellexpand::tilde(p).to_string())
            .collect();

        let total: u64 = expanded
            .par_iter()
            .map(|p| scanner::estimate_size(p).unwrap_or(0))
            .sum();

        (expanded, total)
    }

    fn find_subdirs_for(&self, def: &CleanItemDef) -> Vec<(String, u64)> {
        if let CleanStrategy::RemoveSubdirs { subdir, indicators, exclude_indicators, min_age_days } = &def.strategy {
            let expanded: Vec<String> = def.paths.iter()
                .map(|p| shellexpand::tilde(p).to_string())
                .collect();
            scanner::find_subdirs(&expanded, subdir, indicators, exclude_indicators, *min_age_days)
        } else {
            Vec::new()
        }
    }

    pub fn scan(&self, on_progress: Option<&ScanProgress>) -> ScanResult {
        self.scan_filtered(&[], on_progress)
    }

    pub fn scan_filtered(
        &self,
        category_ids: &[String],
        on_progress: Option<&ScanProgress>,
    ) -> ScanResult {
        let mut all_items = Vec::new();
        let mut all_categories = HashMap::new();
        let mut total_size = 0u64;

        for cat in &self.categories {
            if !category_ids.is_empty() && !category_ids.contains(&cat.id) {
                continue;
            }

            let mut cat_size = 0u64;
            let mut cat_count = 0usize;

            if let Some(cb) = on_progress {
                cb(&cat.id, "start", None, None);
            }

            for def in &cat.items {
                if let Some(cb) = on_progress {
                    cb(&cat.id, "item", Some(&def.label), Some(0));
                }

                let item = if def.is_subdirs() {
                    let found = self.find_subdirs_for(def);
                    if found.is_empty() {
                        continue;
                    }
                    let total: u64 = found.iter().map(|(_, s)| s).sum();
                    if total < def.min_size {
                        continue;
                    }
                    CleanItem {
                        id: def.id.clone(),
                        category: cat.id.clone(),
                        label: def.label.clone(),
                        path: found.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", "),
                        size: total,
                        count: found.len() as u64,
                        action: CleanAction::Delete,
                    }
                } else {
                    // Fast pass: estimate size from first-level entries
                    let (_expanded, estimated) = self.estimate_paths(&def.paths);
                    if estimated < def.min_size {
                        continue;
                    }
                    // Full pass: walk all files in parallel to get exact size
                    let (_expanded, sizes, total) = self.expand_paths_par(&def.paths);
                    if total < def.min_size {
                        continue;
                    }
                    CleanItem {
                        id: def.id.clone(),
                        category: cat.id.clone(),
                        label: def.label.clone(),
                        path: _expanded.join(", "),
                        size: total,
                        count: sizes.iter().filter(|&&s| s > 0).count() as u64,
                        action: CleanAction::Delete,
                    }
                };

                cat_size += item.size;
                cat_count += 1;
                all_items.push(item);
            }

            if let Some(cb) = on_progress {
                cb(&cat.id, "done", None, Some(cat_size));
            }

            total_size += cat_size;
            all_categories.insert(cat.id.clone(), CategorySummary {
                label: cat.label.clone(),
                size: cat_size,
                count: cat_count,
            });
        }

        ScanResult {
            items: all_items,
            total_size,
            categories: all_categories,
        }
    }

    pub fn preview(&self, item_ids: &[String]) -> PreviewResult {
        let mut items = Vec::new();
        let mut total_size = 0u64;
        let mut protected_skipped = Vec::new();

        for id in item_ids {
            let def = match self.find_item(id) {
                Some(d) => d,
                None => continue,
            };

            let (path_str, item_total) = if def.is_subdirs() {
                let found = self.find_subdirs_for(def);
                if found.is_empty() {
                    continue;
                }
                let total: u64 = found.iter().map(|(_, s)| s).sum();
                (found.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", "), total)
            } else {
                let (expanded, _sizes, total) = self.expand_paths_par(&def.paths);
                (expanded.join(", "), total)
            };

            let protected = def.strategy.uses_filesystem()
                && path_str.split(", ").any(|p| self.whitelist.is_protected(p));
            if protected {
                protected_skipped.push(id.clone());
                items.push(PreviewItem {
                    id: id.clone(),
                    path: path_str,
                    size: item_total,
                    will_delete: false,
                });
            } else {
                total_size += item_total;
                items.push(PreviewItem {
                    id: id.clone(),
                    path: path_str,
                    size: item_total,
                    will_delete: true,
                });
            }
        }

        PreviewResult {
            dry_run: true,
            items,
            total_size,
            protected_skipped,
        }
    }

    pub fn run(&self, item_ids: &[String]) -> DeleteResult {
        let mut completed = Vec::new();
        let mut failed = Vec::new();

        for id in item_ids {
            let def = match self.find_item(id) {
                Some(d) => d,
                None => {
                    failed.push(DeleteResultItem {
                        id: id.clone(),
                        path: String::new(),
                        size: 0,
                        status: DeleteStatus::Skipped,
                        error: Some(format!("item not found: {id}")),
                    });
                    continue;
                }
            };

            if def.is_subdirs() {
                let found = self.find_subdirs_for(def);
                if found.is_empty() {
                    failed.push(DeleteResultItem {
                        id: id.clone(),
                        path: String::new(),
                        size: 0,
                        status: DeleteStatus::Skipped,
                        error: Some("no matching subdirectories found".to_string()),
                    });
                    continue;
                }

                let protected = found.iter().any(|(p, _)| self.whitelist.is_protected(p));
                if protected {
                    let total: u64 = found.iter().map(|(_, s)| s).sum();
                    failed.push(DeleteResultItem {
                        id: id.clone(),
                        path: found.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", "),
                        size: total,
                        status: DeleteStatus::Skipped,
                        error: Some("path is protected by whitelist".to_string()),
                    });
                    continue;
                }

                let (c, f) = deleter::delete_subdirs(id, &found);
                completed.extend(c);
                failed.extend(f);
            } else if let Some(cmd) = &def.command {
                let (_expanded, _sizes, total) = self.expand_paths_par(&def.paths);
                let result = deleter::run_command(id, cmd, total);
                if result.status == DeleteStatus::Deleted {
                    completed.push(result);
                } else {
                    failed.push(result);
                }
            } else {
                let (expanded, _sizes, total) = self.expand_paths_par(&def.paths);
                let path_str = expanded.join(", ");
                let protected = def.strategy.uses_filesystem()
                    && path_str.split(", ").any(|p| self.whitelist.is_protected(p));
                if protected {
                    failed.push(DeleteResultItem {
                        id: id.clone(),
                        path: path_str,
                        size: total,
                        status: DeleteStatus::Skipped,
                        error: Some("path is protected by whitelist".to_string()),
                    });
                    continue;
                }
                let pair: Vec<(&str, &str, u64)> = vec![(id.as_str(), &path_str, total)];
                let res = deleter::delete_paths(&pair);
                completed.extend(res.completed);
                failed.extend(res.failed);
            }
        }

        let total_freed = completed.iter().map(|i| i.size).sum();
        DeleteResult { completed, failed, total_freed }
    }

    pub fn whitelist(&self) -> &Whitelist {
        &self.whitelist
    }

    pub fn whitelist_mut(&mut self) -> &mut Whitelist {
        &mut self.whitelist
    }

    fn find_item(&self, id: &str) -> Option<&CleanItemDef> {
        for cat in &self.categories {
            for item in &cat.items {
                if item.id == id {
                    return Some(item);
                }
            }
        }
        None
    }
}
