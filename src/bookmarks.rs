// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Bookmark {
    pub name: String,
    pub lat_micro: i64,
    pub lon_micro: i64,
    pub zoom: u8,
}

impl Bookmark {
    pub fn new(name: String, lat: f64, lon: f64, zoom: u8) -> Self {
        Self {
            name,
            lat_micro: (lat * 1_000_000.0) as i64,
            lon_micro: (lon * 1_000_000.0) as i64,
            zoom,
        }
    }

    pub fn lat(&self) -> f64 {
        self.lat_micro as f64 / 1_000_000.0
    }

    pub fn lon(&self) -> f64 {
        self.lon_micro as f64 / 1_000_000.0
    }
}
