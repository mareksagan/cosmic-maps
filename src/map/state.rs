// SPDX-License-Identifier: MIT

use std::f64::consts::PI;

const TILE_SIZE: f64 = 256.0;

#[derive(Clone, Debug)]
pub struct MapState {
    pub center_lat: f64,
    pub center_lon: f64,
    pub zoom: u8,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

impl Default for MapState {
    fn default() -> Self {
        Self {
            center_lat: 51.505,
            center_lon: -0.09,
            zoom: 13,
            viewport_width: 800.0,
            viewport_height: 600.0,
        }
    }
}

impl MapState {
    pub fn new(lat: f64, lon: f64, zoom: u8) -> Self {
        Self {
            center_lat: lat,
            center_lon: lon,
            zoom,
            ..Default::default()
        }
    }

    pub fn tile_count(&self) -> f64 {
        (1u64 << self.zoom) as f64
    }

    pub fn lat_lon_to_tile(&self, lat: f64, lon: f64) -> (f64, f64) {
        let n = self.tile_count();
        let x = (lon + 180.0) / 360.0 * n;
        let lat_rad = lat.to_radians();
        let y = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0 * n;
        (x, y)
    }

    pub fn tile_to_lat_lon(&self, x: f64, y: f64) -> (f64, f64) {
        let n = self.tile_count();
        let lon = x / n * 360.0 - 180.0;
        let lat_rad = ((1.0 - 2.0 * y / n) * PI).sinh().atan();
        let lat = lat_rad.to_degrees();
        (lat, lon)
    }

    pub fn center_tile(&self) -> (f64, f64) {
        self.lat_lon_to_tile(self.center_lat, self.center_lon)
    }

    pub fn visible_tiles(&self, vw: f32, vh: f32) -> Vec<(u8, u64, u64)> {
        let n = self.tile_count() as u64;
        let (cx, cy) = self.center_tile();
        let cx_px = cx * TILE_SIZE;
        let cy_px = cy * TILE_SIZE;

        let vw = vw as f64;
        let vh = vh as f64;

        let min_x = ((cx_px - vw / 2.0) / TILE_SIZE).floor().max(0.0) as u64;
        let max_x = ((cx_px + vw / 2.0) / TILE_SIZE).ceil().min(n as f64 - 1.0) as u64;
        let min_y = ((cy_px - vh / 2.0) / TILE_SIZE).floor().max(0.0) as u64;
        let max_y = ((cy_px + vh / 2.0) / TILE_SIZE).ceil().min(n as f64 - 1.0) as u64;

        let mut tiles = Vec::with_capacity(((max_x - min_x + 1) * (max_y - min_y + 1)) as usize);
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                tiles.push((self.zoom, x, y));
            }
        }
        tiles
    }

    /// Screen offset of the top-left corner of a given tile relative to viewport top-left.
    pub fn tile_offset(&self, tile_x: u64, tile_y: u64, vw: f32, vh: f32) -> (f64, f64) {
        let (cx, cy) = self.center_tile();
        let x_px = tile_x as f64 * TILE_SIZE;
        let y_px = tile_y as f64 * TILE_SIZE;
        let cx_px = cx * TILE_SIZE;
        let cy_px = cy * TILE_SIZE;
        (
            x_px - cx_px + vw as f64 / 2.0,
            y_px - cy_px + vh as f64 / 2.0,
        )
    }

    pub fn pan_pixels(&mut self, delta_x: f64, delta_y: f64) {
        let n = self.tile_count();
        let (cx, cy) = self.center_tile();
        let new_cx = cx - delta_x / TILE_SIZE;
        let new_cy = cy - delta_y / TILE_SIZE;
        // Clamp y to valid Mercator range
        let new_cy = new_cy.clamp(0.0, n - 1e-9);
        // Wrap x around the world
        let new_cx = new_cx.rem_euclid(n);
        let (lat, lon) = self.tile_to_lat_lon(new_cx, new_cy);
        self.center_lat = lat.clamp(-85.05112878, 85.05112878);
        self.center_lon = lon;
        tracing::trace!("pan_pixels: new center lat={} lon={} zoom={}", self.center_lat, self.center_lon, self.zoom);
    }

    pub fn zoom_at_point(&mut self, delta_zoom: i8, cursor_x: f64, cursor_y: f64, vw: f32, vh: f32) {
        let new_zoom = (self.zoom as i16 + delta_zoom as i16).clamp(0, 19) as u8;
        if new_zoom == self.zoom {
            return;
        }
        let scale_factor = (1u64 << new_zoom.abs_diff(self.zoom)) as f64;
        let factor = if delta_zoom > 0 { scale_factor } else { 1.0 / scale_factor };

        let (cx, cy) = self.center_tile();
        // cursor relative to viewport center, in tile units
        let dx = (cursor_x - vw as f64 / 2.0) / TILE_SIZE;
        let dy = (cursor_y - vh as f64 / 2.0) / TILE_SIZE;

        let new_cx = (cx + dx) * factor - dx;
        let new_cy = (cy + dy) * factor - dy;

        let n = (1u64 << new_zoom) as f64;
        let new_cy = new_cy.clamp(0.0, n - 1e-9);
        let new_cx = new_cx.rem_euclid(n);

        self.zoom = new_zoom;
        let (lat, lon) = self.tile_to_lat_lon(new_cx, new_cy);
        self.center_lat = lat.clamp(-85.05112878, 85.05112878);
        self.center_lon = lon;
        tracing::trace!(
            "zoom_at_point: delta={delta_zoom} cursor=({cursor_x},{cursor_y}) viewport={vw}x{vh} -> lat={} lon={} zoom={}",
            self.center_lat, self.center_lon, self.zoom
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn lat_lon_to_tile_roundtrip() {
        let state = MapState::new(51.505, -0.09, 13);
        let (x, y) = state.lat_lon_to_tile(state.center_lat, state.center_lon);
        let (lat, lon) = state.tile_to_lat_lon(x, y);
        assert!(approx_eq(lat, state.center_lat, 1e-9), "lat mismatch: {lat} vs {}", state.center_lat);
        assert!(approx_eq(lon, state.center_lon, 1e-9), "lon mismatch: {lon} vs {}", state.center_lon);
    }

    #[test]
    fn center_tile_matches_lat_lon() {
        let state = MapState::new(40.7128, -74.0060, 10);
        let (x1, y1) = state.lat_lon_to_tile(state.center_lat, state.center_lon);
        let (x2, y2) = state.center_tile();
        assert!(approx_eq(x1, x2, 1e-9));
        assert!(approx_eq(y1, y2, 1e-9));
    }

    #[test]
    fn tile_offset_center_tile_is_viewport_center() {
        let state = MapState::new(0.0, 0.0, 10);
        let vw = 800.0;
        let vh = 600.0;
        let (cx, cy) = state.center_tile();
        let tile_x = cx.floor() as u64;
        let tile_y = cy.floor() as u64;
        let (ox, oy) = state.tile_offset(tile_x, tile_y, vw, vh);
        // The top-left of the tile that contains the center should be offset
        // by the fractional part from the viewport center.
        let expected_ox = vw as f64 / 2.0 - (cx - tile_x as f64) * TILE_SIZE;
        let expected_oy = vh as f64 / 2.0 - (cy - tile_y as f64) * TILE_SIZE;
        assert!(
            approx_eq(ox, expected_ox, 1e-6),
            "ox mismatch: {ox} vs {expected_ox}"
        );
        assert!(
            approx_eq(oy, expected_oy, 1e-6),
            "oy mismatch: {oy} vs {expected_oy}"
        );
    }

    #[test]
    fn visible_tiles_includes_center_tile() {
        let state = MapState::new(51.505, -0.09, 13);
        let vw = 800.0;
        let vh = 600.0;
        let tiles = state.visible_tiles(vw, vh);
        let (cx, cy) = state.center_tile();
        let center_tile = (state.zoom, cx.floor() as u64, cy.floor() as u64);
        assert!(tiles.contains(&center_tile), "visible tiles should include center tile {center_tile:?}");
    }

    #[test]
    fn pan_pixels_does_not_change_zoom() {
        let mut state = MapState::new(10.0, 20.0, 7);
        state.pan_pixels(100.0, -50.0);
        assert_eq!(state.zoom, 7);
    }

    #[test]
    fn pan_pixels_roundtrip() {
        let mut state = MapState::new(10.0, 20.0, 7);
        let original = (state.center_lat, state.center_lon);
        state.pan_pixels(100.0, 50.0);
        state.pan_pixels(-100.0, -50.0);
        assert!(
            approx_eq(state.center_lat, original.0, 1e-6),
            "lat drift: {} vs {}", state.center_lat, original.0
        );
        assert!(
            approx_eq(state.center_lon, original.1, 1e-6),
            "lon drift: {} vs {}", state.center_lon, original.1
        );
    }

    #[test]
    fn zoom_at_center_preserves_location() {
        let mut state = MapState::new(48.8566, 2.3522, 10);
        let vw = 1024.0;
        let vh = 768.0;
        let original = (state.center_lat, state.center_lon);
        state.zoom_at_point(1, vw as f64 / 2.0, vh as f64 / 2.0, vw, vh);
        assert_eq!(state.zoom, 11);
        assert!(
            approx_eq(state.center_lat, original.0, 1e-6),
            "lat changed on center zoom: {} vs {}", state.center_lat, original.0
        );
        assert!(
            approx_eq(state.center_lon, original.1, 1e-6),
            "lon changed on center zoom: {} vs {}", state.center_lon, original.1
        );
    }

    #[test]
    fn zoom_at_corner_preserves_corner() {
        let mut state = MapState::new(40.0, -74.0, 10);
        let vw = 1024.0;
        let vh = 768.0;
        // Pick top-left corner as cursor
        let cursor_x = 0.0;
        let cursor_y = 0.0;

        // Compute the lat/lon at the top-left corner before zoom
        let (cx_old, cy_old) = state.center_tile();
        let dx = (cursor_x - vw as f64 / 2.0) / TILE_SIZE;
        let dy = (cursor_y - vh as f64 / 2.0) / TILE_SIZE;
        let tile_x_old = cx_old + dx;
        let tile_y_old = cy_old + dy;
        let (lat_old, lon_old) = state.tile_to_lat_lon(tile_x_old, tile_y_old);

        state.zoom_at_point(1, cursor_x, cursor_y, vw, vh);
        assert_eq!(state.zoom, 11);

        let (cx_new, cy_new) = state.center_tile();
        let dx_new = (cursor_x - vw as f64 / 2.0) / TILE_SIZE;
        let dy_new = (cursor_y - vh as f64 / 2.0) / TILE_SIZE;
        let tile_x_new = cx_new + dx_new;
        let tile_y_new = cy_new + dy_new;
        let (lat_new, lon_new) = state.tile_to_lat_lon(tile_x_new, tile_y_new);

        assert!(
            approx_eq(lat_new, lat_old, 1e-6),
            "corner lat changed: {lat_new} vs {lat_old}"
        );
        assert!(
            approx_eq(lon_new, lon_old, 1e-6),
            "corner lon changed: {lon_new} vs {lon_old}"
        );
    }

    #[test]
    fn zoom_in_then_out_returns_to_original() {
        let mut state = MapState::new(35.0, 139.0, 8);
        let vw = 800.0;
        let vh = 600.0;
        let cursor_x = vw as f64 / 2.0 + 123.0;
        let cursor_y = vh as f64 / 2.0 - 45.0;
        let original = (state.center_lat, state.center_lon, state.zoom);

        state.zoom_at_point(1, cursor_x, cursor_y, vw, vh);
        assert_eq!(state.zoom, 9);
        state.zoom_at_point(-1, cursor_x, cursor_y, vw, vh);
        assert_eq!(state.zoom, 8);

        assert!(
            approx_eq(state.center_lat, original.0, 1e-6),
            "lat drift after zoom in/out: {} vs {}", state.center_lat, original.0
        );
        assert!(
            approx_eq(state.center_lon, original.1, 1e-6),
            "lon drift after zoom in/out: {} vs {}", state.center_lon, original.1
        );
    }

    #[test]
    fn tile_offset_after_pan_is_consistent() {
        let mut state = MapState::new(0.0, 0.0, 5);
        let vw = 800.0;
        let vh = 600.0;
        let tiles_before = state.visible_tiles(vw, vh);
        let first_before = tiles_before[0];
        let offset_before = state.tile_offset(first_before.1, first_before.2, vw, vh);

        state.pan_pixels(100.0, 50.0);
        let tiles_after = state.visible_tiles(vw, vh);
        // The same tile should still be in visible set if it was near center
        // Instead, check that the offset changed by exactly the pan amount
        if let Some(tile) = tiles_after.iter().find(|t| t.1 == first_before.1 && t.2 == first_before.2) {
            let offset_after = state.tile_offset(tile.1, tile.2, vw, vh);
            let dx = offset_after.0 - offset_before.0;
            let dy = offset_after.1 - offset_before.1;
            assert!(
                approx_eq(dx, 100.0, 1e-3) && approx_eq(dy, 50.0, 1e-3),
                "offset didn't shift by pan amount: dx={dx} dy={dy}"
            );
        }
    }

    #[test]
    fn zoom_at_point_multiple_levels() {
        let mut state = MapState::new(50.0, 10.0, 5);
        let vw = 800.0;
        let vh = 600.0;
        let cursor_x = vw as f64 / 2.0;
        let cursor_y = vh as f64 / 2.0;
        let original = (state.center_lat, state.center_lon);

        state.zoom_at_point(3, cursor_x, cursor_y, vw, vh);
        assert_eq!(state.zoom, 8);
        assert!(approx_eq(state.center_lat, original.0, 1e-6), "lat drift after +3 zoom");
        assert!(approx_eq(state.center_lon, original.1, 1e-6), "lon drift after +3 zoom");

        state.zoom_at_point(-3, cursor_x, cursor_y, vw, vh);
        assert_eq!(state.zoom, 5);
        assert!(approx_eq(state.center_lat, original.0, 1e-6), "lat drift after -3 zoom");
        assert!(approx_eq(state.center_lon, original.1, 1e-6), "lon drift after -3 zoom");
    }
}
