// SPDX-License-Identifier: MIT

use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
use serde::Deserialize;

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Location {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum LocationError {
    DbusError(String),
    NetworkError(String),
    NotAvailable,
}

pub async fn locate_user() -> Result<Location, LocationError> {
    tracing::info!("locate_user: starting geolocation");

    // Try multiple IP geolocation services first (fast, reliable)
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    tracing::info!("locate_user: trying ip-api.com");
    match locate_via_ip_api(&client).await {
        Ok(loc) => {
            tracing::info!("locate_user: ip-api.com succeeded lat={} lon={}", loc.lat, loc.lon);
            return Ok(loc);
        }
        Err(e) => tracing::warn!("locate_user: ip-api.com failed: {e:?}"),
    }

    tracing::info!("locate_user: trying ipwho.is");
    match locate_via_ipwho(&client).await {
        Ok(loc) => {
            tracing::info!("locate_user: ipwho.is succeeded lat={} lon={}", loc.lat, loc.lon);
            return Ok(loc);
        }
        Err(e) => tracing::warn!("locate_user: ipwho.is failed: {e:?}"),
    }

    tracing::info!("locate_user: trying ipapi.co");
    match locate_via_ipapi(&client).await {
        Ok(loc) => {
            tracing::info!("locate_user: ipapi.co succeeded lat={} lon={}", loc.lat, loc.lon);
            return Ok(loc);
        }
        Err(e) => tracing::warn!("locate_user: ipapi.co failed: {e:?}"),
    }

    // Fall back to GeoClue2 if IP geolocation fails entirely
    tracing::info!("locate_user: falling back to GeoClue2");
    match tokio::task::spawn_blocking(locate_via_geoclue)
        .await
        .map_err(|e| LocationError::DbusError(e.to_string()))?
    {
        Ok(loc) => {
            tracing::info!("locate_user: GeoClue succeeded lat={} lon={}", loc.lat, loc.lon);
            Ok(loc)
        }
        Err(e) => {
            tracing::error!("locate_user: GeoClue failed: {e:?}");
            Err(e)
        }
    }
}

fn locate_via_geoclue() -> Result<Location, LocationError> {
    tracing::debug!("locate_via_geoclue: opening system D-Bus connection");
    let conn = dbus::blocking::Connection::new_system()
        .map_err(|e| LocationError::DbusError(e.to_string()))?;

    let proxy = conn.with_proxy(
        "org.freedesktop.GeoClue2",
        "/org/freedesktop/GeoClue2/Manager",
        std::time::Duration::from_secs(5),
    );

    tracing::debug!("locate_via_geoclue: calling GetClient");
    let (client_path,): (dbus::Path,) = proxy
        .method_call("org.freedesktop.GeoClue2.Manager", "GetClient", ())
        .map_err(|e| {
            tracing::error!("locate_via_geoclue: GetClient failed: {e}");
            LocationError::DbusError(format!("GetClient failed: {e}"))
        })?;
    tracing::debug!("locate_via_geoclue: client_path={}", &client_path);

    let client_proxy = conn.with_proxy(
        "org.freedesktop.GeoClue2",
        &client_path,
        std::time::Duration::from_secs(5),
    );

    // GeoClue2 requires DesktopId to be set before Start
    tracing::debug!("locate_via_geoclue: setting DesktopId");
    if let Err(e) = client_proxy.set(
        "org.freedesktop.GeoClue2.Client",
        "DesktopId",
        "com.system76.CosmicMaps".to_string(),
    ) {
        tracing::warn!("locate_via_geoclue: failed to set DesktopId: {e}");
    }

    tracing::debug!("locate_via_geoclue: calling Start");
    client_proxy
        .method_call::<(), _, _, _>("org.freedesktop.GeoClue2.Client", "Start", ())
        .map_err(|e| {
            tracing::error!("locate_via_geoclue: Start failed: {e}");
            LocationError::DbusError(format!("Start failed: {e}"))
        })?;

    // Poll the Location property until it becomes non-empty
    tracing::debug!("locate_via_geoclue: waiting for location fix");
    let location_path: dbus::Path = {
        let mut loc_path: dbus::Path = "/".into();
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            match client_proxy.get::<dbus::Path>("org.freedesktop.GeoClue2.Client", "Location") {
                Ok(p) if p != "/" => {
                    loc_path = p;
                    break;
                }
                Ok(_) => continue,
                Err(e) => {
                    tracing::warn!("locate_via_geoclue: getting Location property failed: {e}");
                    break;
                }
            }
        }
        if loc_path == "/" {
            let _ = client_proxy.method_call::<(), _, _, _>("org.freedesktop.GeoClue2.Client", "Stop", ());
            return Err(LocationError::DbusError("GeoClue did not return a location".into()));
        }
        loc_path
    };
    tracing::debug!("locate_via_geoclue: location_path={}", &location_path);

    let location_proxy = conn.with_proxy(
        "org.freedesktop.GeoClue2",
        &location_path,
        std::time::Duration::from_secs(5),
    );

    let lat: f64 = location_proxy
        .get("org.freedesktop.GeoClue2.Location", "Latitude")
        .map_err(|e| {
            tracing::error!("locate_via_geoclue: Latitude failed: {e}");
            LocationError::DbusError(format!("Latitude failed: {e}"))
        })?;
    let lon: f64 = location_proxy
        .get("org.freedesktop.GeoClue2.Location", "Longitude")
        .map_err(|e| {
            tracing::error!("locate_via_geoclue: Longitude failed: {e}");
            LocationError::DbusError(format!("Longitude failed: {e}"))
        })?;

    tracing::debug!("locate_via_geoclue: lat={lat} lon={lon}");

    let _ = client_proxy.method_call::<(), _, _, _>("org.freedesktop.GeoClue2.Client", "Stop", ());

    Ok(Location { lat, lon })
}

// --- ipapi.co ---
#[derive(Deserialize, Debug)]
struct IpApiResponse {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

async fn locate_via_ipapi(client: &reqwest::Client) -> Result<Location, LocationError> {
    let resp = client
        .get("https://ipapi.co/json/")
        .send()
        .await
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    let data: IpApiResponse = resp
        .json()
        .await
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    match (data.latitude, data.longitude) {
        (Some(lat), Some(lon)) => Ok(Location { lat, lon }),
        _ => Err(LocationError::NotAvailable),
    }
}

// --- ip-api.com ---
#[derive(Deserialize, Debug)]
struct IpApiComResponse {
    status: String,
    lat: Option<f64>,
    lon: Option<f64>,
}

async fn locate_via_ip_api(client: &reqwest::Client) -> Result<Location, LocationError> {
    let resp = client
        .get("http://ip-api.com/json/?fields=status,lat,lon")
        .send()
        .await
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    let data: IpApiComResponse = resp
        .json()
        .await
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    if data.status != "success" {
        return Err(LocationError::NotAvailable);
    }
    match (data.lat, data.lon) {
        (Some(lat), Some(lon)) => Ok(Location { lat, lon }),
        _ => Err(LocationError::NotAvailable),
    }
}

// --- ipwho.is ---
#[derive(Deserialize, Debug)]
struct IpWhoResponse {
    success: bool,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

async fn locate_via_ipwho(client: &reqwest::Client) -> Result<Location, LocationError> {
    let resp = client
        .get("https://ipwho.is/")
        .send()
        .await
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    let data: IpWhoResponse = resp
        .json()
        .await
        .map_err(|e| LocationError::NetworkError(e.to_string()))?;

    if !data.success {
        return Err(LocationError::NotAvailable);
    }
    match (data.latitude, data.longitude) {
        (Some(lat), Some(lon)) => Ok(Location { lat, lon }),
        _ => Err(LocationError::NotAvailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ipapi_response() {
        let json = r#"{"ip":"1.2.3.4","network":"1.2.3.0/24","version":"IPv4","city":"Warsaw","region":"Mazovia","region_code":"14","country":"PL","country_name":"Poland","country_code":"PL","country_code_iso3":"POL","country_capital":"Warsaw","country_tld":".pl","continent_code":"EU","in_eu":true,"postal":"00-001","latitude":52.2297,"longitude":21.0122,"timezone":"Europe/Warsaw","utc_offset":"+01:00","country_calling_code":"+48","currency":"PLN","currency_name":"Zloty","languages":"pl","country_area":312696.0,"country_population":37958138,"asn":"AS5617","org":"Orange Polska Spolka Akcyjna"}"#;
        let data: IpApiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.latitude, Some(52.2297));
        assert_eq!(data.longitude, Some(21.0122));
    }

    #[test]
    fn parse_ipapi_rate_limit_response() {
        // When rate limited, ipapi returns missing latitude/longitude fields
        let json = r#"{"reason":"RateLimited","message":"Please contact us for a trial account"}"#;
        let data: IpApiResponse = serde_json::from_str(json).unwrap();
        assert!(data.latitude.is_none());
        assert!(data.longitude.is_none());
    }

    #[test]
    fn parse_ip_api_com_response() {
        let json = r#"{"status":"success","lat":52.2299,"lon":21.0093}"#;
        let data: IpApiComResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.status, "success");
        assert_eq!(data.lat, Some(52.2299));
        assert_eq!(data.lon, Some(21.0093));
    }

    #[test]
    fn parse_ip_api_com_fail_response() {
        let json = r#"{"status":"fail","message":"invalid query"}"#;
        let data: IpApiComResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.status, "fail");
        assert!(data.lat.is_none());
        assert!(data.lon.is_none());
    }

    #[test]
    fn parse_ipwho_response() {
        let json = r#"{"ip":"2a02:a311:4399:6480::2f41","success":true,"type":"IPv6","continent":"Europe","latitude":52.2103359,"longitude":20.9712206}"#;
        let data: IpWhoResponse = serde_json::from_str(json).unwrap();
        assert!(data.success);
        assert_eq!(data.latitude, Some(52.2103359));
        assert_eq!(data.longitude, Some(20.9712206));
    }

    #[test]
    fn parse_ipwho_fail_response() {
        let json = r#"{"success":false,"message":"Invalid IP address"}"#;
        let data: IpWhoResponse = serde_json::from_str(json).unwrap();
        assert!(!data.success);
    }

    #[test]
    fn location_error_display() {
        let e = LocationError::NotAvailable;
        assert_eq!(format!("{e:?}"), "NotAvailable");
    }
}
