use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct FileCache {
    cache: Arc<RwLock<HashMap<PathBuf, Vec<u8>>>>,
}

impl FileCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_or_load(&self, path: &PathBuf) -> Option<Vec<u8>> {
        {
            if let Ok(cache_lock) = self.cache.read() {
                if let Some(content) = cache_lock.get(path) {
                    return Some(content.clone());
                }
            }
        }

        let content = fs::read(path).ok()?;

        if let Ok(mut cache_lock) = self.cache.write() {
            cache_lock.insert(path.clone(), content.clone());
        }

        Some(content)
    }
}
