use dioxus::prelude::*;

use crate::router::RouteView;

#[component]
pub fn Info() -> Element {
    rsx! {
        div { class: "page-wrap",
            div { class: "page-header",
                h1 { class: "page-title", "Product updates" }
                p { class: "page-subtitle", "What’s new in Lyra, and what’s coming next." }
            }

            div { class: "update-list",
                section { class: "card-base-plain",
                    span { class: "status-badge badge-green", "Now" }
                    h3 { "Unified project workspace" }
                    p { class: "muted", "Browse projects, open tickets, and leave comments without leaving the dashboard." }
                }
                section { class: "card-base-plain",
                    span { class: "status-badge badge-blue", "Soon" }
                    h3 { "AI-powered contextual support" }
                    p { class: "muted", "An assistant that learns from your tickets and conversations so you don’t have to manage memory or tokens by hand." }
                }
                section { class: "card-base-plain",
                    span { class: "status-badge", "Planned" }
                    h3 { "Smarter notifications" }
                    p { class: "muted", "Get notified when comments land on tickets you follow, with digest options for quieter days." }
                }
            }
        }
    }
}

#[component]
pub fn InfoPreview() -> Element {
    rsx! {
        section { class: "card-base-plain",
            h2 { class: "page-title", "Recent updates" }
            p { class: "page-subtitle", "AI support is on the way — built for solo workflows, not extra setup." }
            p { class: "muted", "We’re adding an assistant that understands your tickets and current conversations so help shows up in context." }
            div { style: "margin-top: 18px;",
                Link { to: RouteView::Info {}, class: "btn-secondary", "Read all updates" }
            }
        }
    }
}
