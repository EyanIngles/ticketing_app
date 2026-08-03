#![allow(non_snake_case)]

use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let mut text = use_signal(|| {
        // Load saved text or default
        LocalStorage::get("saved_text").unwrap_or_else(|_| "Start typing here...".to_string())
    });

    rsx! {
        div {
            style: "max-width: 600px; margin: 40px auto; padding: 20px; font-family: system-ui;",

            h1 { "Lrya Ticket Manager" }
            p { "Projects" }

            div {
                style: "margin-top: 16px; display: flex; gap: 12px;",




            }

            p { style: "margin-top: 20px; color: #666; font-size: 0.95em;",
                "Property of Lyrindra Labs"

            }
        }
    }
}
