// SPDX-License-Identifier: MIT

use super::{MapState, TileCache, TileId};
use cosmic::iced::{
    mouse,
    widget::canvas::{self, Event, Frame, Geometry, Path, Program, Stroke, Text},
    Color, Point, Rectangle,
};
use std::sync::Mutex;

pub struct MapCanvas {
    pub state: Mutex<MapState>,
    pub tiles: TileCache,
    current_location: Mutex<Option<(f64, f64)>>,
}

impl MapCanvas {
    pub fn new(state: MapState, tiles: TileCache) -> Self {
        Self {
            state: Mutex::new(state),
            tiles,
            current_location: Mutex::new(None),
        }
    }

    pub fn set_current_location(&self, loc: Option<(f64, f64)>) {
        *self.current_location.lock().unwrap() = loc;
    }
}

pub struct CanvasState {
    dragging: bool,
    last_cursor: Option<Point>,
    zoom_accumulator: f64,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            dragging: false,
            last_cursor: None,
            zoom_accumulator: 0.0,
        }
    }
}

impl Program<crate::app::Message, cosmic::Theme, cosmic::Renderer> for MapCanvas {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<crate::app::Message>> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    state.dragging = true;
                    state.last_cursor = Some(pos);
                    return Some(canvas::Action::capture());
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
                state.last_cursor = None;
                return Some(canvas::Action::capture());
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.dragging {
                    if let Some(last) = state.last_cursor {
                        let dx = position.x - last.x;
                        let dy = position.y - last.y;
                        state.last_cursor = Some(*position);
                        return Some(canvas::Action::publish(crate::app::Message::MapPan(
                            dx as f64, dy as f64,
                        )));
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if let Some(pos) = cursor.position() {
                    let raw = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => *y as f64,
                        mouse::ScrollDelta::Pixels { y, .. } => *y as f64 / 40.0,
                    };
                    state.zoom_accumulator += raw;
                    if state.zoom_accumulator.abs() >= 1.0 {
                        let delta_zoom = state.zoom_accumulator.signum() as i8;
                        state.zoom_accumulator -= delta_zoom as f64;
                        tracing::trace!(
                            "WheelScrolled: raw={raw} delta_zoom={delta_zoom} cursor=({},{}) bounds={}x{}",
                            pos.x, pos.y, bounds.width, bounds.height
                        );
                        return Some(canvas::Action::publish(crate::app::Message::MapZoom(
                            delta_zoom,
                            pos.x as f64,
                            pos.y as f64,
                            bounds.width,
                            bounds.height,
                        )));
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut map_state = self.state.lock().unwrap();
        if (map_state.viewport_width - bounds.width).abs() > 0.5
            || (map_state.viewport_height - bounds.height).abs() > 0.5
        {
            tracing::debug!(
                "canvas draw: updating viewport from {}x{} to {}x{}",
                map_state.viewport_width,
                map_state.viewport_height,
                bounds.width,
                bounds.height
            );
            map_state.viewport_width = bounds.width;
            map_state.viewport_height = bounds.height;
        }
        let vw = bounds.width;
        let vh = bounds.height;

        let mut frame = Frame::new(renderer, bounds.size());

        // Background
        frame.fill_rectangle(
            Point::ORIGIN,
            bounds.size(),
            Color::from_rgb8(0xEE, 0xEE, 0xEE),
        );

        let visible = map_state.visible_tiles(vw, vh);
        for (z, x, y) in visible {
            let id = TileId { z, x, y };
            let (offset_x, offset_y) = map_state.tile_offset(x, y, vw, vh);
            let tile_size = 256.0_f32;

            if let Some(handle) = self.tiles.get(&id) {
                frame.draw_image(
                    Rectangle::new(
                        Point::new(offset_x as f32, offset_y as f32),
                        cosmic::iced::Size::new(tile_size, tile_size),
                    ),
                    canvas::Image::new(handle),
                );
            } else {
                // Placeholder grid
                frame.stroke_rectangle(
                    Point::new(offset_x as f32, offset_y as f32),
                    cosmic::iced::Size::new(tile_size, tile_size),
                    Stroke::default().with_color(Color::from_rgb8(0xCC, 0xCC, 0xCC)),
                );
            }
        }

        // Draw current location marker if known
        if let Some((lat, lon)) = *self.current_location.lock().unwrap() {
            let (tx, ty) = map_state.lat_lon_to_tile(lat, lon);
            let (cx, cy) = map_state.center_tile();
            let screen_x = (tx - cx) * 256.0 + vw as f64 / 2.0;
            let screen_y = (ty - cy) * 256.0 + vh as f64 / 2.0;

            // Only draw if inside viewport
            if screen_x >= -20.0
                && screen_x <= bounds.width as f64 + 20.0
                && screen_y >= -20.0
                && screen_y <= bounds.height as f64 + 20.0
            {
                let center = Point::new(screen_x as f32, screen_y as f32);
                // Outer white ring
                frame.fill(
                    &Path::circle(center, 10.0),
                    Color::from_rgb8(0xFF, 0xFF, 0xFF),
                );
                // Inner red dot
                frame.fill(
                    &Path::circle(center, 6.0),
                    Color::from_rgb8(0xFF, 0x33, 0x33),
                );
            }
        }
        drop(map_state);

        // OSM attribution
        let mut text = Text::from("© OpenStreetMap contributors");
        text.position = Point::new(8.0, bounds.height - 8.0);
        text.color = Color::from_rgb8(0x33, 0x33, 0x33);
        text.size = cosmic::iced::Pixels(10.0);
        frame.fill_text(text);

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.dragging {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::Grab
        }
    }
}
