use std::path::Path;

#[derive(Debug, Clone)]
pub struct Whitelist {
    patterns: Vec<String>,
}

impl Whitelist {
    pub fn new() -> Self {
        Self {
            patterns: vec![
                "~/Library/Caches/com.apple.*".to_string(),
            ],
        }
    }

    pub fn is_protected(&self, path: &str) -> bool {
        let expanded = shellexpand::tilde(path);
        let p = Path::new(expanded.as_ref());
        self.patterns.iter().any(|pattern| {
            let expanded_pattern = shellexpand::tilde(pattern);
            let glob = globset::Glob::new(expanded_pattern.as_ref())
                .map(|g| g.compile_matcher())
                .ok();
            match glob {
                Some(m) => m.is_match(p),
                None => false,
            }
        })
    }

    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    pub fn add_pattern(&mut self, pattern: String) {
        if !self.patterns.contains(&pattern) {
            self.patterns.push(pattern);
        }
    }

    pub fn remove_pattern(&mut self, pattern: &str) {
        self.patterns.retain(|p| p != pattern);
    }
}
