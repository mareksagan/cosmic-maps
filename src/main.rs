// SPDX-License-Identifier: MIT

mod app;
mod bookmarks;
mod config;
mod i18n;
mod location;
mod map;
mod menu;
mod poi;
mod search;

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt::init();

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(640.0)
            .min_height(480.0),
    );

    cosmic::app::run_single_instance::<app::AppModel>(settings, app::AppFlags::default())
}
