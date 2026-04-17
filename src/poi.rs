// SPDX-License-Identifier: MIT

use serde::Deserialize;

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";
const USER_AGENT: &str = "COSMIC Maps/0.1.0 (https://github.com/example/cosmic-maps)";
const FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Poi {
    pub id: u64,
    pub lat: f64,
    pub lon: f64,
    pub name: String,
    pub category: String,
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Element {
    id: u64,
    lat: Option<f64>,
    lon: Option<f64>,
    tags: Option<Tags>,
}

#[derive(Debug, Deserialize)]
struct Tags {
    name: Option<String>,
    amenity: Option<String>,
    shop: Option<String>,
    tourism: Option<String>,
    historic: Option<String>,
    leisure: Option<String>,
    #[serde(flatten)]
    _other: std::collections::HashMap<String, String>,
}

pub async fn fetch_pois(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
) -> Result<Vec<Poi>, String> {
    let query = format!(
        "[out:json][timeout:10];
(
  node[\"amenity\"]({min_lat},{min_lon},{max_lat},{max_lon});
  node[\"shop\"]({min_lat},{min_lon},{max_lat},{max_lon});
  node[\"tourism\"]({min_lat},{min_lon},{max_lat},{max_lon});
  node[\"historic\"]({min_lat},{min_lon},{max_lat},{max_lon});
  node[\"leisure\"]({min_lat},{min_lon},{max_lat},{max_lon});
);
out center 50;"
    );

    tracing::info!(
        "fetch_pois: bbox=({min_lat},{min_lon},{max_lat},{max_lon})"
    );

    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .post(OVERPASS_URL)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("data={}", urlencoding::encode(&query)))
        .send()
        .await
        .map_err(|e| {
            tracing::warn!("fetch_pois: request failed: {e}");
            e.to_string()
        })?;

    let text = resp.text().await.map_err(|e| {
        tracing::warn!("fetch_pois: read body failed: {e}");
        e.to_string()
    })?;

    let data: OverpassResponse = serde_json::from_str(&text).map_err(|e| {
        tracing::warn!("fetch_pois: parse json failed: {e}");
        e.to_string()
    })?;

    let mut pois = Vec::with_capacity(data.elements.len());
    for el in data.elements {
        let (lat, lon) = match (el.lat, el.lon) {
            (Some(lat), Some(lon)) => (lat, lon),
            _ => continue,
        };

        let tags = match el.tags {
            Some(t) => t,
            None => continue,
        };

        let name = tags.name.unwrap_or_else(|| "Unnamed".to_string());
        let category = tags
            .amenity
            .or(tags.shop)
            .or(tags.tourism)
            .or(tags.historic)
            .or(tags.leisure)
            .unwrap_or_else(|| "poi".to_string());

        pois.push(Poi {
            id: el.id,
            lat,
            lon,
            name,
            category,
        });
    }

    tracing::info!("fetch_pois: found {} POIs", pois.len());
    Ok(pois)
}
