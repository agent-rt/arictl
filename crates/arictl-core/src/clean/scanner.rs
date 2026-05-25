use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use rayon::prelude::*;

/// Calculate directory/file size using `du -sb`.
/// `du` uses kernel-level getattrlist/fts, much faster than Rust read_dir + stat.
pub fn calculate_size(path: &str) -> Option<u64> {
    let p = Path::new(path);
    if !p.exists() {
        return Some(0);
    }
    if p.is_symlink() {
        return Some(0);
    }
    if p.is_file() {
        return p.metadata().ok().map(|m| m.len());
    }

    let output = Command::new("du")
        .arg("-sb")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return Some(0);
    }
    let out_str = std::str::from_utf8(&output.stdout).ok()?;
    let size_str = out_str.split_whitespace().next()?;
    size_str.parse::<u64>().ok()
}

/// Estimate directory size by stat'ing first-level entries only (fast, rough).
/// Returns 0 for files / empty dirs.
pub fn estimate_size(path: &str) -> Option<u64> {
    let p = Path::new(path);
    if !p.exists() || p.is_symlink() {
        return Some(0);
    }
    if p.is_file() {
        return p.metadata().ok().map(|m| m.len());
    }
    if p.is_dir() {
        let total: u64 = match p.read_dir() {
            Ok(e) => e
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.is_symlink() {
                        return Some(0);
                    }
                    if path.is_file() {
                        e.metadata().ok().map(|m| m.len())
                    } else if path.is_dir() {
                        e.metadata().ok().map(|m| m.len())
                    } else {
                        Some(0)
                    }
                })
                .sum(),
            Err(_) => 0,
        };
        Some(total)
    } else {
        Some(0)
    }
}

/// Search project roots for subdirectories matching `subdir`, optionally filtered
/// by indicator files, exclude indicators, and minimum age in days.
/// Checks depth 1 and 2 from each root.
/// First pass: discover matching subdirs (fast stat only).
/// Second pass: calculate all sizes in parallel via du.
pub fn find_subdirs(
    roots: &[String],
    subdir: &str,
    indicators: &[String],
    exclude_indicators: &[String],
    min_age_days: u64,
) -> Vec<(String, u64)> {
    let mut candidates = Vec::new();

    for root in roots {
        let root_path = Path::new(root);
        if !root_path.exists() {
            continue;
        }

        let top_entries: Vec<_> = match root_path.read_dir() {
            Ok(e) => e
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .collect(),
            Err(_) => continue,
        };

        for entry in &top_entries {
            collect_candidate(&entry.path(), subdir, indicators, exclude_indicators, min_age_days, &mut candidates);
        }

        for entry in &top_entries {
            let sub_entries: Vec<_> = match entry.path().read_dir() {
                Ok(e) => e
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                    .collect(),
                Err(_) => continue,
            };
            for sub in &sub_entries {
                collect_candidate(&sub.path(), subdir, indicators, exclude_indicators, min_age_days, &mut candidates);
            }
        }
    }

    // Second pass: calculate all sizes in parallel
    let sizes: Vec<u64> = candidates
        .par_iter()
        .map(|p| calculate_size(&p.to_string_lossy()).unwrap_or(0))
        .collect();

    candidates
        .into_iter()
        .zip(sizes)
        .map(|(p, s)| (p.to_string_lossy().to_string(), s))
        .collect()
}

/// Fast stat-only check: is this a valid candidate? Returns path if yes.
fn collect_candidate(
    project_dir: &Path,
    subdir: &str,
    indicators: &[String],
    exclude_indicators: &[String],
    min_age_days: u64,
    candidates: &mut Vec<PathBuf>,
) {
    if !indicators.is_empty() {
        let has = indicators.iter().any(|ind| project_dir.join(ind).exists());
        if !has {
            return;
        }
    }

    for ind in exclude_indicators {
        if project_dir.join(ind).exists() {
            return;
        }
    }

    let target = project_dir.join(subdir);
    if !target.exists() || !target.is_dir() {
        return;
    }

    if min_age_days > 0 {
        if let Ok(meta) = target.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    let age_days = age.as_secs() / 86400;
                    if age_days < min_age_days {
                        return;
                    }
                }
            }
        }
    }

    candidates.push(target);
}
