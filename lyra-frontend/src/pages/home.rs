use dioxus::prelude::*;

use crate::pages::{InfoPreview, ProjectPreview};
use crate::router::RouteView;
use crate::states::IS_LOGGED_IN;

#[component]
pub fn Home() -> Element {
    let nav = navigator();

    use_effect(move || {
        if !*IS_LOGGED_IN.read() {
            nav.replace(RouteView::Login {});
        }
    });

    rsx! {
        section { class: "hero",
            h1 { "Welcome to Lyra" }
            p { "Manage projects, track tickets, and keep conversations in one dashboard." }
            div { class: "hero-actions",
                Link { to: RouteView::Projects {}, class: "btn-primary", "View projects" }
                Link { to: RouteView::Chat {}, class: "btn-secondary", "Open chat" }
            }
        }

        div { class: "home-grid",
            InfoPreview {}
            ProjectPreview {}
        }

        Link {
            to: RouteView::Chat {},
            class: "chat-widget",
            title: "Open chat",
            "💬"
        }
    }
}
