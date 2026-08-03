use dioxus::prelude::*;

pub static IS_LOGGED_IN: GlobalSignal<bool> = Signal::global(|| false);
