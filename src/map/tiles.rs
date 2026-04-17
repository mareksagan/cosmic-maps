// SPDX-License-Identifier: MIT

use cosmic::iced::widget::image::Handle;
use image::RgbaImage;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

const USER_AGENT: &str = "COSMIC Maps/0.1.0 (https://github.com/example/cosmic-maps)";
const MAX_RETRIES: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TileId {
    pub z: u8,
    pub x: u64,
    pub y: u64,
}

impl TileId {
    pub fn url(&self) -> String {
        format!(
            "https://tile.openstreetmap.org/{}/{}/{}.png",
            self.z, self.x, self.y
        )
    }
}

#[derive(Clone, Debug)]
pub struct TileCache {
    inner: Arc<Mutex<LruCache<TileId, Handle>>>,
    pending: Arc<Mutex<std::collections::HashMap<TileId, u8>>>,
}

impl Default for TileCache {
    fn default() -> Self {
        Self::new(256)
    }
}

impl TileCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(capacity).unwrap(),
            ))),
            pending: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn get(&self, id: &TileId) -> Option<Handle> {
        self.inner.lock().unwrap().get(id).cloned()
    }

    pub fn insert(&self, id: TileId, handle: Handle) {
        tracing::trace!("TileCache::insert {id:?}");
        self.inner.lock().unwrap().put(id, handle);
        self.pending.lock().unwrap().remove(&id);
    }

    /// Mark a tile as pending for fetch. Returns true if it should be fetched.
    pub fn mark_pending(&self, id: TileId) -> bool {
        let mut pending = self.pending.lock().unwrap();
        let retries = pending.entry(id).or_insert(0);
        if *retries >= MAX_RETRIES {
            tracing::trace!("TileCache::mark_pending {id:?} max retries reached");
            return false;
        }
        *retries += 1;
        tracing::trace!("TileCache::mark_pending {id:?} retry={}", *retries);
        true
    }

    pub fn remove_pending(&self, id: &TileId) {
        self.pending.lock().unwrap().remove(id);
    }

    #[allow(dead_code)]
    pub fn is_pending(&self, id: &TileId) -> bool {
        self.pending.lock().unwrap().contains_key(id)
    }

    pub fn missing(&self, tiles: &[TileId]) -> Vec<TileId> {
        let cache = self.inner.lock().unwrap();
        let pending = self.pending.lock().unwrap();
        tiles
            .iter()
            .filter(|id| !cache.contains(id) && !pending.contains_key(id))
            .cloned()
            .collect()
    }
}

pub async fn fetch_tile(id: TileId) -> Result<Handle, String> {
    tracing::trace!("fetch_tile: {id:?} url={}", id.url());
    let client = reqwest::Client::new();
    let bytes = client
        .get(id.url())
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("fetch_tile {id:?}: request failed: {e}");
            e.to_string()
        })?
        .bytes()
        .await
        .map_err(|e| {
            tracing::warn!("fetch_tile {id:?}: read body failed: {e}");
            e.to_string()
        })?;

    tracing::trace!("fetch_tile {id:?}: received {} bytes", bytes.len());

    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|e| {
            tracing::warn!("fetch_tile {id:?}: decode png failed: {e}");
            e.to_string()
        })?
        .to_rgba8();

    Ok(image_handle_from_rgba(img))
}

pub fn image_handle_from_rgba(img: RgbaImage) -> Handle {
    let (width, height) = img.dimensions();
    let pixels: Vec<u8> = img.into_raw();
    Handle::from_rgba(width, height, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_cache_mark_pending_limits_retries() {
        let cache = TileCache::new(4);
        let id = TileId { z: 1, x: 0, y: 0 };
        assert!(cache.mark_pending(id));
        assert!(cache.mark_pending(id));
        assert!(cache.mark_pending(id));
        // 4th attempt should be blocked (MAX_RETRIES = 3)
        assert!(!cache.mark_pending(id));
    }

    #[test]
    fn tile_cache_missing_ignores_pending_and_cached() {
        let cache = TileCache::new(4);
        let id1 = TileId { z: 1, x: 0, y: 0 };
        let id2 = TileId { z: 1, x: 1, y: 0 };
        let id3 = TileId { z: 1, x: 2, y: 0 };

        cache.mark_pending(id1);
        cache.insert(id2, Handle::from_rgba(1, 1, vec![0]));

        let missing = cache.missing(&[id1, id2, id3]);
        assert_eq!(missing, vec![id3]);
    }

    #[test]
    fn tile_cache_remove_pending_allows_retry() {
        let cache = TileCache::new(4);
        let id = TileId { z: 1, x: 0, y: 0 };
        cache.mark_pending(id);
        cache.remove_pending(&id);
        assert!(cache.mark_pending(id));
    }
}
