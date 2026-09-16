use dioxus::prelude::*;

use crate::api::jwt_token;

pub static IS_LOGGED_IN: GlobalSignal<bool> = Signal::global(|| jwt_token().is_some());
