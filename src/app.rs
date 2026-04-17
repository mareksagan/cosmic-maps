// SPDX-License-Identifier: MIT

use crate::{
    bookmarks::Bookmark,
    config::Config,
    fl,
    map::{MapCanvas, MapState, TileCache, TileId},
    menu::{self, AppMenuAction},
    search::SearchResult,
};
use cosmic::{
    app::context_drawer,
    cosmic_config::{self, CosmicConfigEntry},
    iced::{
        keyboard::{self, Key},
        window,
        Alignment, Length, Subscription,
    },
    widget::{self, about::About, icon, text_input},
    widget::menu::Action,
    Application, Core, Element, Task,
};
use std::collections::HashMap;
use std::time::Duration;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/com.system76.CosmicMaps.svg");

#[derive(Clone, Debug, Default)]
pub struct AppFlags;

impl cosmic::app::CosmicFlags for AppFlags {
    type SubCommand = String;
    type Args = Vec<String>;
}

pub struct AppModel {
    core: Core,
    context_page: ContextPage,
    about: About,
    key_binds: HashMap<cosmic::widget::menu::key_bind::KeyBind, AppMenuAction>,
    config: Config,
    config_handler: Option<cosmic::cosmic_config::Config>,

    map_canvas: MapCanvas,
    current_location: Option<(f64, f64)>,
    error_message: Option<String>,
    pois: Vec<crate::poi::Poi>,
    poi_fetch_pending: bool,
    last_poi_fetch: Option<std::time::Instant>,
    selected_poi_id: Option<u64>,

    search_query: String,
    search_results: Vec<SearchResult>,
    bookmark_input: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    LaunchUrl(String),
    ToggleAboutPage,
    ToggleBookmarksPage,
    ToggleSearchResults,
    ShowAddBookmark,
    ResetView,
    UpdateConfig(Config),
    KeyEvent(keyboard::Event),

    MapPan(f64, f64),
    MapZoom(i8, f64, f64, f32, f32),
    CheckTiles,
    TileFetched(TileId, Result<cosmic::iced::widget::image::Handle, String>),

    FetchPois,
    PoisFetched(Result<Vec<crate::poi::Poi>, String>),
    SelectPoi(u64),

    SearchInput(String),
    SearchSubmit,
    SearchResults(Result<Vec<SearchResult>, String>),
    SelectSearchResult(SearchResult),

    BookmarkInput(String),
    SaveBookmark,
    DeleteBookmark(usize),
    GoToBookmark(usize),
    DismissBookmarkDialog,

    LocateUser,
    LocationResult(Result<crate::location::Location, crate::location::LocationError>),
    DismissError,

    ZoomIn,
    ZoomOut,

    DismissPoiInfo,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
    Bookmarks,
    SearchResults,
    AddBookmark,
    PoiInfo,
}

impl Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = AppFlags;
    type Message = Message;

    const APP_ID: &'static str = "com.system76.CosmicMaps";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        tracing::info!("AppModel::init");
        let about = About::default()
            .name(fl!("app-title"))
            .icon(icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .links([(fl!("repository"), REPOSITORY)])
            .license(env!("CARGO_PKG_LICENSE"));

        let (config, config_handler) =
            match cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                Ok(handler) => {
                    let config = match Config::get_entry(&handler) {
                        Ok(config) => config,
                        Err((_errors, config)) => config,
                    };
                    (config, Some(handler))
                }
                Err(e) => {
                    tracing::warn!("failed to create cosmic_config: {e}");
                    (Config::default(), None)
                }
            };

        let map_state = if config.remember_last_view {
            config
                .last_view()
                .map(|(lat, lon, zoom)| MapState::new(lat, lon, zoom))
                .unwrap_or_default()
        } else {
            MapState::default()
        };
        tracing::info!("initial map state: lat={} lon={} zoom={}", map_state.center_lat, map_state.center_lon, map_state.zoom);

        let app = Self {
            core,
            context_page: ContextPage::default(),
            about,
            key_binds: menu::init_key_binds(),
            config: config.clone(),
            config_handler,
            map_canvas: MapCanvas::new(map_state, TileCache::new(256)),
            current_location: None,
            error_message: None,
            pois: Vec::new(),
            poi_fetch_pending: false,
            last_poi_fetch: None,
            selected_poi_id: None,
            search_query: String::new(),
            search_results: Vec::new(),
            bookmark_input: String::new(),
        };

        // Automatically locate user on startup
        let locate_task = Task::perform(
            crate::location::locate_user(),
            |res| cosmic::Action::App(Message::LocationResult(res)),
        );

        (app, locate_task)
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        vec![menu::menu_bar(&self.key_binds)]
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        let space_s = cosmic::theme::spacing().space_s;

        let search = text_input(fl!("search-placeholder"), &self.search_query)
            .width(Length::Fixed(280.0))
            .on_input(Message::SearchInput)
            .on_submit(|_| Message::SearchSubmit);

        let zoom_out = widget::button::icon(icon::from_name("zoom-out-symbolic"))
            .padding(space_s)
            .on_press(Message::ZoomOut);

        let zoom_in = widget::button::icon(icon::from_name("zoom-in-symbolic"))
            .padding(space_s)
            .on_press(Message::ZoomIn);

        let locate = widget::button::icon(icon::from_name("mark-location-symbolic"))
            .padding(space_s)
            .on_press(Message::LocateUser);

        let bookmark = widget::button::icon(icon::from_name("bookmark-new-symbolic"))
            .padding(space_s)
            .on_press(Message::ShowAddBookmark);

        let row = widget::row::with_capacity(6)
            .push(search)
            .push(zoom_out)
            .push(zoom_in)
            .push(locate)
            .push(bookmark)
            .align_y(Alignment::Center)
            .spacing(space_s);

        vec![row.into()]
    }

    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match self.context_page {
            ContextPage::About => context_drawer::about(
                &self.about,
                |url| Message::LaunchUrl(url.to_string()),
                Message::ToggleAboutPage,
            ),
            ContextPage::Bookmarks => {
                let space_s = cosmic::theme::spacing().space_s;
                let mut list = widget::column::with_capacity(self.config.bookmarks.len() + 1)
                    .spacing(space_s)
                    .width(Length::Fill);

                for (i, bm) in self.config.bookmarks.iter().enumerate() {
                    let goto = widget::button::text(&bm.name)
                        .on_press(Message::GoToBookmark(i))
                        .width(Length::Fill);
                    let del = widget::button::icon(icon::from_name("user-trash-symbolic"))
                        .on_press(Message::DeleteBookmark(i));
                    let row = widget::row::with_capacity(2)
                        .push(goto)
                        .push(del)
                        .spacing(space_s)
                        .width(Length::Fill);
                    list = list.push(row);
                }

                if self.config.bookmarks.is_empty() {
                    list = list.push(widget::text::body("No bookmarks yet"));
                }

                context_drawer::context_drawer(list, Message::ToggleBookmarksPage)
                    .title(fl!("menu-manage-bookmarks"))
            }
            ContextPage::SearchResults => {
                let space_xxs = cosmic::theme::spacing().space_xxs;
                let mut list = widget::column::with_capacity(self.search_results.len() + 1)
                    .spacing(space_xxs)
                    .width(Length::Fill);

                if self.search_results.is_empty() {
                    list = list.push(widget::text::body(fl!("search-no-results")));
                } else {
                    for result in &self.search_results {
                        let btn = widget::button::text(&result.display_name)
                            .on_press(Message::SelectSearchResult(result.clone()))
                            .width(Length::Fill);
                        list = list.push(btn);
                    }
                }

                context_drawer::context_drawer(list, Message::ToggleSearchResults)
                    .title(fl!("search-results"))
            }
            ContextPage::AddBookmark => {
                let space_s = cosmic::theme::spacing().space_s;
                let input = text_input(fl!("bookmark-name-placeholder"), &self.bookmark_input)
                    .on_input(Message::BookmarkInput)
                    .on_submit(|_| Message::SaveBookmark)
                    .width(Length::Fill);

                let save = widget::button::suggested(fl!("bookmark-save"))
                    .on_press(Message::SaveBookmark);
                let cancel = widget::button::standard(fl!("dismiss"))
                    .on_press(Message::DismissBookmarkDialog);

                let content = widget::column::with_capacity(3)
                    .push(input)
                    .push(
                        widget::row::with_capacity(2)
                            .push(save)
                            .push(cancel)
                            .spacing(space_s),
                    )
                    .spacing(space_s)
                    .width(Length::Fill);

                context_drawer::context_drawer(content, Message::DismissBookmarkDialog)
                    .title(fl!("menu-add-bookmark"))
            }
            ContextPage::PoiInfo => {
                let space_s = cosmic::theme::spacing().space_s;
                let mut list = widget::column::with_capacity(4)
                    .spacing(space_s)
                    .width(Length::Fill);

                if let Some(id) = self.selected_poi_id {
                    if let Some(poi) = self.pois.iter().find(|p| p.id == id) {
                        list = list.push(widget::text::heading(&poi.name));
                        list = list.push(widget::text::body(format!("Category: {}", poi.category)));
                        list = list.push(widget::text::body(format!("Latitude: {:.5}", poi.lat)));
                        list = list.push(widget::text::body(format!("Longitude: {:.5}", poi.lon)));
                    } else {
                        list = list.push(widget::text::body("POI not found"));
                    }
                } else {
                    list = list.push(widget::text::body("No POI selected"));
                }

                context_drawer::context_drawer(list, Message::DismissPoiInfo)
                    .title("Point of Interest")
            }
        })
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let space_s = cosmic::theme::spacing().space_s;

        let canvas = cosmic::iced::widget::canvas(&self.map_canvas)
            .width(Length::Fill)
            .height(Length::Fill);

        if let Some(ref error) = self.error_message {
            let banner = widget::container(
                widget::row::with_capacity(2)
                    .push(widget::text::body(error.clone()))
                    .push(
                        widget::button::icon(icon::from_name("window-close-symbolic"))
                            .on_press(Message::DismissError),
                    )
                    .spacing(space_s)
                    .align_y(Alignment::Center),
            )
            .class(cosmic::theme::Container::Primary)
            .padding(space_s)
            .width(Length::Fill);

            widget::column::with_capacity(2)
                .push(banner)
                .push(canvas)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            canvas.into()
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = vec![
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
            cosmic::iced::time::every(Duration::from_millis(100))
                .map(|_| Message::CheckTiles),
        ];

        subscriptions.push(
            cosmic::iced::event::listen_with(|event, _status, _id| match event {
                cosmic::iced::Event::Keyboard(key_event) => {
                    Some(Message::KeyEvent(key_event))
                }
                _ => None,
            }),
        );

        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        tracing::debug!("update: message={message:?}");
        match message {
            Message::LaunchUrl(url) => {
                if let Err(err) = open::that_detached(&url) {
                    tracing::error!("failed to open {url:?}: {err}");
                }
            }

            Message::ToggleAboutPage => {
                if self.context_page == ContextPage::About {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = ContextPage::About;
                    self.core.window.show_context = true;
                }
            }

            Message::ToggleBookmarksPage => {
                if self.context_page == ContextPage::Bookmarks {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = ContextPage::Bookmarks;
                    self.core.window.show_context = true;
                }
            }

            Message::ToggleSearchResults => {
                if self.context_page == ContextPage::SearchResults {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = ContextPage::SearchResults;
                    self.core.window.show_context = true;
                }
            }

            Message::ShowAddBookmark => {
                self.bookmark_input = String::new();
                self.context_page = ContextPage::AddBookmark;
                self.core.window.show_context = true;
            }

            Message::ResetView => {
                tracing::info!("resetting view to default");
                *self.map_canvas.state.lock().unwrap() = MapState::default();
                self.save_view();
            }

            Message::UpdateConfig(config) => {
                tracing::debug!("config updated: bookmarks={}", config.bookmarks.len());
                self.config = config;
            }

            Message::MapPan(dx, dy) => {
                tracing::trace!("pan dx={dx} dy={dy}");
                self.map_canvas.state.lock().unwrap().pan_pixels(dx, dy);
                self.save_view();
            }
            Message::MapZoom(delta, cx, cy, vw, vh) => {
                tracing::trace!("zoom delta={delta} at ({cx},{cy}) viewport={vw}x{vh}");
                self.map_canvas
                    .state
                    .lock()
                    .unwrap()
                    .zoom_at_point(delta, cx, cy, vw, vh);
                self.save_view();
            }

            Message::CheckTiles => {
                let mut tasks = vec![self.request_missing_tiles()];

                // Throttled POI fetch
                let should_fetch_pois = {
                    let state = self.map_canvas.state.lock().unwrap();
                    let zoom_ok = state.zoom >= 14;
                    let not_pending = !self.poi_fetch_pending;
                    let cooldown_ok = self
                        .last_poi_fetch
                        .map(|t| t.elapsed().as_secs() >= 5)
                        .unwrap_or(true);
                    zoom_ok && not_pending && cooldown_ok
                };
                if should_fetch_pois {
                    tasks.push(Task::perform(
                        async { Message::FetchPois },
                        cosmic::Action::App,
                    ));
                }

                return Task::batch(tasks);
            }

            Message::FetchPois => {
                self.poi_fetch_pending = true;
                let (min_lat, min_lon, max_lat, max_lon) = {
                    let state = self.map_canvas.state.lock().unwrap();
                    let (cx, cy) = state.center_tile();
                    let dx = (state.viewport_width as f64 / 2.0) / 256.0;
                    let dy = (state.viewport_height as f64 / 2.0) / 256.0;
                    let (max_lat, min_lon) = state.tile_to_lat_lon(cx - dx, cy - dy);
                    let (min_lat, max_lon) = state.tile_to_lat_lon(cx + dx, cy + dy);
                    (min_lat, min_lon, max_lat, max_lon)
                };
                return Task::perform(
                    async move { crate::poi::fetch_pois(min_lat, min_lon, max_lat, max_lon).await },
                    |res| cosmic::Action::App(Message::PoisFetched(res)),
                );
            }

            Message::PoisFetched(result) => {
                self.poi_fetch_pending = false;
                self.last_poi_fetch = Some(std::time::Instant::now());
                match result {
                    Ok(pois) => {
                        tracing::info!("PoisFetched: {} POIs", pois.len());
                        self.pois = pois;
                        self.map_canvas.set_pois(&self.pois);
                    }
                    Err(e) => {
                        tracing::warn!("PoisFetched failed: {e}");
                    }
                }
            }

            Message::SelectPoi(id) => {
                tracing::info!("SelectPoi: {id}");
                self.selected_poi_id = Some(id);
                self.map_canvas.set_selected_poi(Some(id));
                if let Some(poi) = self.pois.iter().find(|p| p.id == id) {
                    let mut state = self.map_canvas.state.lock().unwrap();
                    state.center_lat = poi.lat;
                    state.center_lon = poi.lon;
                    drop(state);
                    self.save_view();
                }
                self.context_page = ContextPage::PoiInfo;
                self.core.window.show_context = true;
            }

            Message::DismissPoiInfo => {
                self.core.window.show_context = false;
                self.selected_poi_id = None;
                self.map_canvas.set_selected_poi(None);
            }

            Message::TileFetched(id, result) => {
                match result {
                    Ok(handle) => {
                        tracing::trace!("tile fetched {id:?}");
                        self.map_canvas.tiles.insert(id, handle);
                    }
                    Err(e) => {
                        tracing::warn!("failed to fetch tile {id:?}: {e}");
                        self.map_canvas.tiles.remove_pending(&id);
                    }
                }
            }

            Message::ZoomIn => {
                let state = self.map_canvas.state.lock().unwrap();
                let (cx, cy) = (state.viewport_width as f64 / 2.0, state.viewport_height as f64 / 2.0);
                let (vw, vh) = (state.viewport_width, state.viewport_height);
                drop(state);
                self.map_canvas.state.lock().unwrap().zoom_at_point(1, cx, cy, vw, vh);
                self.save_view();
            }

            Message::ZoomOut => {
                let state = self.map_canvas.state.lock().unwrap();
                let (cx, cy) = (state.viewport_width as f64 / 2.0, state.viewport_height as f64 / 2.0);
                let (vw, vh) = (state.viewport_width, state.viewport_height);
                drop(state);
                self.map_canvas.state.lock().unwrap().zoom_at_point(-1, cx, cy, vw, vh);
                self.save_view();
            }

            Message::SearchInput(value) => {
                self.search_query = value;
            }

            Message::SearchSubmit => {
                let query = self.search_query.clone();
                tracing::info!("search submitted: {query}");
                if !query.is_empty() {
                    return Task::perform(
                        async move { crate::search::search(&query).await },
                        |res| cosmic::Action::App(Message::SearchResults(res)),
                    );
                }
            }

            Message::SearchResults(result) => {
                match result {
                    Ok(results) => {
                        tracing::info!("search returned {} results", results.len());
                        self.search_results = results;
                        self.context_page = ContextPage::SearchResults;
                        self.core.window.show_context = true;
                    }
                    Err(e) => {
                        tracing::warn!("search failed: {e}");
                        self.search_results = Vec::new();
                        self.core.window.show_context = false;
                    }
                }
            }

            Message::SelectSearchResult(result) => {
                tracing::info!("selected search result: {} lat={} lon={}", result.display_name, result.lat, result.lon);
                let mut state = self.map_canvas.state.lock().unwrap();
                state.center_lat = result.lat;
                state.center_lon = result.lon;
                let lat_span = (result.bounding_box.1 - result.bounding_box.0).abs();
                let lon_span = (result.bounding_box.3 - result.bounding_box.2).abs();
                if lat_span > 0.0 && lon_span > 0.0 {
                    let zoom_lat = ((85.05112878 * 2.0 / lat_span).log2()).floor() as u8;
                    let lon_zoom = ((360.0 / lon_span).log2()).floor() as u8;
                    state.zoom = zoom_lat.min(lon_zoom).min(19);
                }
                drop(state);
                self.core.window.show_context = false;
                self.save_view();
            }

            Message::BookmarkInput(value) => {
                self.bookmark_input = value;
            }

            Message::SaveBookmark => {
                let name = self.bookmark_input.trim().to_string();
                if !name.is_empty() {
                    let state = self.map_canvas.state.lock().unwrap();
                    let bm = Bookmark::new(
                        name,
                        state.center_lat,
                        state.center_lon,
                        state.zoom,
                    );
                    drop(state);
                    self.config.bookmarks.push(bm);
                    let _ = self.write_config();
                }
                self.core.window.show_context = false;
                self.bookmark_input.clear();
            }

            Message::DismissBookmarkDialog => {
                self.core.window.show_context = false;
                self.bookmark_input.clear();
            }

            Message::DeleteBookmark(index) => {
                if index < self.config.bookmarks.len() {
                    self.config.bookmarks.remove(index);
                    let _ = self.write_config();
                }
            }

            Message::GoToBookmark(index) => {
                if let Some(bm) = self.config.bookmarks.get(index) {
                    let mut state = self.map_canvas.state.lock().unwrap();
                    state.center_lat = bm.lat();
                    state.center_lon = bm.lon();
                    state.zoom = bm.zoom;
                    drop(state);
                    self.save_view();
                }
            }

            Message::LocateUser => {
                tracing::info!("LocateUser requested");
                self.error_message = None;
                return Task::perform(
                    crate::location::locate_user(),
                    |res| cosmic::Action::App(Message::LocationResult(res)),
                );
            }

            Message::LocationResult(result) => {
                match result {
                    Ok(loc) => {
                        tracing::info!("LocationResult: lat={} lon={}", loc.lat, loc.lon);
                        let mut state = self.map_canvas.state.lock().unwrap();
                        state.center_lat = loc.lat;
                        state.center_lon = loc.lon;
                        state.zoom = 15;
                        drop(state);
                        self.current_location = Some((loc.lat, loc.lon));
                        self.map_canvas.set_current_location(self.current_location);
                        self.error_message = None;
                        self.save_view();
                    }
                    Err(e) => {
                        let msg = format!("{e:?}");
                        tracing::warn!("location failed: {msg}");
                        self.error_message = Some(msg);
                    }
                }
            }

            Message::DismissError => {
                self.error_message = None;
            }

            Message::KeyEvent(event) => {
                if let keyboard::Event::KeyPressed { key, modifiers, .. } = event {
                    let action = self.key_binds.iter().find(|(bind, _)| {
                        bind.key == key && modifier_match(&bind.modifiers, modifiers)
                    });

                    if let Some((_, action)) = action {
                        return self.update(action.message());
                    }

                    if modifiers.is_empty() {
                        let pan_amount = 64.0;
                        let mut state = self.map_canvas.state.lock().unwrap();
                        match key {
                            Key::Named(keyboard::key::Named::ArrowUp) => {
                                state.pan_pixels(0.0, pan_amount);
                            }
                            Key::Named(keyboard::key::Named::ArrowDown) => {
                                state.pan_pixels(0.0, -pan_amount);
                            }
                            Key::Named(keyboard::key::Named::ArrowLeft) => {
                                state.pan_pixels(pan_amount, 0.0);
                            }
                            Key::Named(keyboard::key::Named::ArrowRight) => {
                                state.pan_pixels(-pan_amount, 0.0);
                            }
                            _ => {}
                        }
                        drop(state);
                        self.save_view();
                    }
                }
            }
        }

        Task::none()
    }

    fn on_window_resize(&mut self, _id: window::Id, width: f32, height: f32) {
        tracing::trace!("window resized: {width}x{height}");
        // Viewport is updated from actual canvas draw bounds for accuracy,
        // but we store the window size here as a fallback until draw runs.
        let mut state = self.map_canvas.state.lock().unwrap();
        state.viewport_width = width;
        state.viewport_height = height;
    }
}

impl AppModel {
    fn request_missing_tiles(&self) -> Task<cosmic::Action<Message>> {
        let state = self.map_canvas.state.lock().unwrap();
        let visible: Vec<TileId> = state
            .visible_tiles(state.viewport_width, state.viewport_height)
            .into_iter()
            .map(|(z, x, y)| TileId { z, x, y })
            .collect();
        drop(state);

        let missing = self.map_canvas.tiles.missing(&visible);
        if missing.is_empty() {
            return Task::none();
        }
        tracing::trace!("requesting {} missing tiles", missing.len());

        // Limit concurrent fetches
        let to_fetch: Vec<TileId> = missing.into_iter().take(8).collect();
        for id in &to_fetch {
            self.map_canvas.tiles.mark_pending(*id);
        }

        let tasks: Vec<Task<cosmic::Action<Message>>> = to_fetch
            .into_iter()
            .map(|id| {
                Task::perform(
                    async move { crate::map::tiles::fetch_tile(id).await },
                    move |result| cosmic::Action::App(Message::TileFetched(id, result)),
                )
            })
            .collect();

        Task::batch(tasks)
    }

    fn save_view(&mut self) {
        if self.config.remember_last_view {
            let state = self.map_canvas.state.lock().unwrap();
            self.config.set_last_view(
                state.center_lat,
                state.center_lon,
                state.zoom,
            );
            drop(state);
            let _ = self.write_config();
        }
    }

    fn write_config(&self) -> Result<(), cosmic::cosmic_config::Error> {
        if let Some(ref handler) = self.config_handler {
            self.config.write_entry(handler)?;
        }
        Ok(())
    }
}

fn modifier_match(
    expected: &[cosmic::widget::menu::key_bind::Modifier],
    actual: cosmic::iced::keyboard::Modifiers,
) -> bool {
    use cosmic::widget::menu::key_bind::Modifier;
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut has_super = false;

    for m in expected {
        match m {
            Modifier::Ctrl => has_ctrl = true,
            Modifier::Alt => has_alt = true,
            Modifier::Shift => has_shift = true,
            Modifier::Super => has_super = true,
        }
    }

    actual.control() == has_ctrl
        && actual.alt() == has_alt
        && actual.shift() == has_shift
        && actual.command() == has_super
}
