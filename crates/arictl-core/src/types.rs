use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CleanAction {
    Delete,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanItem {
    pub id: String,
    pub category: String,
    pub label: String,
    pub path: String,
    pub size: u64,
    pub count: u64,
    pub action: CleanAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySummary {
    pub label: String,
    pub size: u64,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub items: Vec<CleanItem>,
    pub total_size: u64,
    pub categories: std::collections::HashMap<String, CategorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewItem {
    pub id: String,
    pub path: String,
    pub size: u64,
    pub will_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewResult {
    pub dry_run: bool,
    pub items: Vec<PreviewItem>,
    pub total_size: u64,
    pub protected_skipped: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeleteStatus {
    Deleted,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResultItem {
    pub id: String,
    pub path: String,
    pub size: u64,
    pub status: DeleteStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub completed: Vec<DeleteResultItem>,
    pub failed: Vec<DeleteResultItem>,
    pub total_freed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub version: String,
    pub pid: u32,
    pub uptime_secs: u64,
    pub operations_count: u64,
    pub addr: String,
}
