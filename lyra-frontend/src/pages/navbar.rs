use dioxus::prelude::*;

use crate::router::RouteView;

pub static MAIN_CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn Navbar() -> Element {
    let mut open = use_signal(|| false);

    rsx! {
        nav {
            Link { to: RouteView::Home {}, class: "logo", "LYRA" }
            ul {
                class: if open() { "nav-links open" } else { "nav-links" },
                id: "navLinks",
                Link { to: RouteView::Home {}, active_class: "active", onclick: move |_| open.set(false), "Home" }
                Link { to: RouteView::Projects {}, active_class: "active", onclick: move |_| open.set(false), "Projects" }
                Link { to: RouteView::Info {}, active_class: "active", onclick: move |_| open.set(false), "Info" }
                Link { to: RouteView::Chat {}, active_class: "active", onclick: move |_| open.set(false), "Chat" }
                Link { to: RouteView::Contact {}, active_class: "active", onclick: move |_| open.set(false), "Contact" }
            }
            button {
                class: "hamburger",
                r#type: "button",
                aria_label: "Toggle navigation",
                onclick: move |_| open.set(!open()),
                if open() { "✕" } else { "☰" }
            }
        }
    }
}

#[component]
pub fn Footer() -> Element {
    rsx! {
        footer { class: "site-footer",
            span { "Lyra Ticketing" }
            span { "Projects, tickets, and support in one place." }
        }
    }
}
