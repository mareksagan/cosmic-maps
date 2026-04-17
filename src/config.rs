// SPDX-License-Identifier: MIT

use crate::bookmarks::Bookmark;
use cosmic::cosmic_config::{cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};


#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub remember_last_view: bool,
    pub last_lat: Option<i64>,   // stored as microdegrees for precision
    pub last_lon: Option<i64>,
    pub last_zoom: Option<u8>,
    pub bookmarks: Vec<Bookmark>,
}

impl Config {
    pub fn last_view(&self) -> Option<(f64, f64, u8)> {
        if let (Some(lat), Some(lon), Some(zoom)) = (self.last_lat, self.last_lon, self.last_zoom) {
            Some((lat as f64 / 1_000_000.0, lon as f64 / 1_000_000.0, zoom))
        } else {
            None
        }
    }

    pub fn set_last_view(&mut self, lat: f64, lon: f64, zoom: u8) {
        self.last_lat = Some((lat * 1_000_000.0) as i64);
        self.last_lon = Some((lon * 1_000_000.0) as i64);
        self.last_zoom = Some(zoom);
    }
}
