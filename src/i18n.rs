// SPDX-License-Identifier: MIT

use i18n_embed::{
    fluent::fluent_language_loader,
    DefaultLocalizer, Localizer,
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<i18n_embed::fluent::FluentLanguageLoader> =
    LazyLock::new(|| fluent_language_loader!());

#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($args),*)
    }};
}

pub fn init(requested_languages: &[i18n_embed::unic_langid::LanguageIdentifier]) {
    let localizer = DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations);

    if let Err(error) = localizer.select(requested_languages) {
        eprintln!("error while loading fluent localizations: {error}");
    }
}
