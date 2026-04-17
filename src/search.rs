// SPDX-License-Identifier: MIT

use serde::Deserialize;

const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
const USER_AGENT: &str = "COSMIC Maps/0.1.0 (https://github.com/example/cosmic-maps)";

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub display_name: String,
    pub lat: f64,
    pub lon: f64,
    pub bounding_box: (f64, f64, f64, f64), // min_lat, max_lat, min_lon, max_lon
}

#[derive(Deserialize)]
struct NominatimResult {
    display_name: String,
    lat: String,
    lon: String,
    boundingbox: Vec<String>,
}

pub async fn search(query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(NOMINATIM_URL)
        .query(&[
            ("q", query),
            ("format", "json"),
            ("limit", "5"),
            ("addressdetails", "0"),
        ])
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let raw: Vec<NominatimResult> = resp.json().await.map_err(|e| e.to_string())?;

    let mut results = Vec::with_capacity(raw.len());
    for r in raw {
        let lat = r.lat.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
        let lon = r.lon.parse().map_err(|e: std::num::ParseFloatError| e.to_string())?;
        let bounding_box = if r.boundingbox.len() == 4 {
            (
                r.boundingbox[0].parse().unwrap_or(lat),
                r.boundingbox[1].parse().unwrap_or(lat),
                r.boundingbox[2].parse().unwrap_or(lon),
                r.boundingbox[3].parse().unwrap_or(lon),
            )
        } else {
            (lat, lat, lon, lon)
        };
        results.push(SearchResult {
            display_name: r.display_name,
            lat,
            lon,
            bounding_box,
        });
    }

    Ok(results)
}
