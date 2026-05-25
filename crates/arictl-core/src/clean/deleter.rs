use std::path::Path;

use crate::types::{DeleteResult, DeleteResultItem, DeleteStatus};

pub fn delete_paths(ids_and_paths: &[(&str, &str, u64)]) -> DeleteResult {
    let mut completed = Vec::new();
    let mut failed = Vec::new();

    for (id, path, size) in ids_and_paths {
        let p = Path::new(path);
        match delete_single(p) {
            Ok(()) => completed.push(DeleteResultItem {
                id: id.to_string(),
                path: path.to_string(),
                size: *size,
                status: DeleteStatus::Deleted,
                error: None,
            }),
            Err(e) => failed.push(DeleteResultItem {
                id: id.to_string(),
                path: path.to_string(),
                size: *size,
                status: DeleteStatus::Skipped,
                error: Some(e.to_string()),
            }),
        }
    }

    let total_freed = completed.iter().map(|i| i.size).sum();
    DeleteResult { completed, failed, total_freed }
}

pub fn delete_subdirs(id: &str, dirs: &[(String, u64)]) -> (Vec<DeleteResultItem>, Vec<DeleteResultItem>) {
    let mut completed = Vec::new();
    let mut failed = Vec::new();

    for (path, size) in dirs {
        let p = Path::new(path);
        match delete_single(p) {
            Ok(()) => completed.push(DeleteResultItem {
                id: id.to_string(),
                path: path.clone(),
                size: *size,
                status: DeleteStatus::Deleted,
                error: None,
            }),
            Err(e) => failed.push(DeleteResultItem {
                id: id.to_string(),
                path: path.clone(),
                size: *size,
                status: DeleteStatus::Skipped,
                error: Some(e.to_string()),
            }),
        }
    }

    (completed, failed)
}

pub fn run_command(id: &str, cmd: &str, size: u64) -> DeleteResultItem {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                DeleteResultItem {
                    id: id.to_string(),
                    path: format!("command: {cmd}"),
                    size,
                    status: DeleteStatus::Deleted,
                    error: None,
                }
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                DeleteResultItem {
                    id: id.to_string(),
                    path: format!("command: {cmd}"),
                    size,
                    status: DeleteStatus::Skipped,
                    error: Some(if stderr.is_empty() {
                        format!("exit code: {}", out.status.code().unwrap_or(-1))
                    } else {
                        stderr
                    }),
                }
            }
        }
        Err(e) => DeleteResultItem {
            id: id.to_string(),
            path: format!("command: {cmd}"),
            size,
            status: DeleteStatus::Skipped,
            error: Some(e.to_string()),
        },
    }
}

fn delete_single(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(());
    }

    trash::delete(path)?;
    Ok(())
}
