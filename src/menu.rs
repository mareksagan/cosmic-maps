// SPDX-License-Identifier: MIT

use crate::app::Message;
use crate::fl;
use cosmic::{Apply, Element, widget::menu::{self, key_bind::KeyBind, Action, Item, Tree}};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppMenuAction {
    ZoomIn,
    ZoomOut,
    ZoomReset,
    MyLocation,
    AddBookmark,
    ToggleBookmarks,
    About,
}

impl Action for AppMenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            AppMenuAction::ZoomIn => Message::ZoomIn,
            AppMenuAction::ZoomOut => Message::ZoomOut,
            AppMenuAction::ZoomReset => Message::ResetView,
            AppMenuAction::MyLocation => Message::LocateUser,
            AppMenuAction::AddBookmark => Message::ShowAddBookmark,
            AppMenuAction::ToggleBookmarks => Message::ToggleBookmarksPage,
            AppMenuAction::About => Message::ToggleAboutPage,
        }
    }
}

pub fn menu_bar(key_binds: &HashMap<KeyBind, AppMenuAction>) -> cosmic::Element<'static, Message> {
    let view_menu = Tree::with_children(
        menu::root(fl!("menu-view")).apply(Element::from),
        menu::items(
            key_binds,
            vec![
                Item::Button(fl!("menu-zoom-in"), None, AppMenuAction::ZoomIn),
                Item::Button(fl!("menu-zoom-out"), None, AppMenuAction::ZoomOut),
                Item::Button(fl!("menu-zoom-reset"), None, AppMenuAction::ZoomReset),
                Item::Divider,
                Item::Button(fl!("menu-my-location"), None, AppMenuAction::MyLocation),
            ],
        ),
    );

    let bookmarks_menu = Tree::with_children(
        menu::root(fl!("menu-bookmarks")).apply(Element::from),
        menu::items(
            key_binds,
            vec![
                Item::Button(fl!("menu-add-bookmark"), None, AppMenuAction::AddBookmark),
                Item::Divider,
                Item::Button(fl!("menu-manage-bookmarks"), None, AppMenuAction::ToggleBookmarks),
            ],
        ),
    );

    let help_menu = Tree::with_children(
        menu::root(fl!("menu-help")).apply(Element::from),
        menu::items(
            key_binds,
            vec![Item::Button(fl!("menu-about"), None, AppMenuAction::About)],
        ),
    );

    menu::bar(vec![view_menu, bookmarks_menu, help_menu]).into()
}

pub fn init_key_binds() -> HashMap<KeyBind, AppMenuAction> {
    use cosmic::iced::keyboard::key::Named;
    use menu::key_bind::Modifier;

    let mut key_binds = HashMap::new();

    key_binds.insert(
        KeyBind {
            key: cosmic::iced::keyboard::Key::Character("+".into()),
            modifiers: vec![Modifier::Ctrl],
        },
        AppMenuAction::ZoomIn,
    );

    key_binds.insert(
        KeyBind {
            key: cosmic::iced::keyboard::Key::Character("-".into()),
            modifiers: vec![Modifier::Ctrl],
        },
        AppMenuAction::ZoomOut,
    );

    key_binds.insert(
        KeyBind {
            key: cosmic::iced::keyboard::Key::Character("0".into()),
            modifiers: vec![Modifier::Ctrl],
        },
        AppMenuAction::ZoomReset,
    );

    key_binds.insert(
        KeyBind {
            key: cosmic::iced::keyboard::Key::Character("d".into()),
            modifiers: vec![Modifier::Ctrl],
        },
        AppMenuAction::AddBookmark,
    );

    key_binds.insert(
        KeyBind {
            key: cosmic::iced::keyboard::Key::Named(Named::F1),
            modifiers: vec![],
        },
        AppMenuAction::About,
    );

    key_binds
}
