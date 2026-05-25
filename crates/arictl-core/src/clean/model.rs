#[derive(Debug, Clone)]
pub enum CleanStrategy {
    RemoveContents,
    RemoveFilesOlderThan(u32),
    RemoveDir,
    RemoveSubdirs {
        subdir: String,
        indicators: Vec<String>,
        exclude_indicators: Vec<String>,
        min_age_days: u64,
    },
}

impl CleanStrategy {
    pub fn uses_filesystem(&self) -> bool {
        matches!(self, CleanStrategy::RemoveContents | CleanStrategy::RemoveFilesOlderThan(_) | CleanStrategy::RemoveDir)
    }
}

#[derive(Debug, Clone)]
pub struct CleanItemDef {
    pub id: String,
    pub label: String,
    pub paths: Vec<String>,
    pub strategy: CleanStrategy,
    pub min_size: u64,
    pub command: Option<String>,
}

impl CleanItemDef {
    pub fn is_subdirs(&self) -> bool {
        matches!(self.strategy, CleanStrategy::RemoveSubdirs { .. })
    }
}

#[derive(Debug, Clone)]
pub struct CleanCategory {
    pub id: String,
    pub label: String,
    pub items: Vec<CleanItemDef>,
}
