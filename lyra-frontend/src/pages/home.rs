use dioxus::html::button::popovertarget;
use dioxus::prelude::*;

use crate::states::IS_LOGGED_IN;
use crate::Route;

#[component]
pub fn HomeView() -> Element {
    let nav = navigator();

    if !*IS_LOGGED_IN.read() {
        // if login has not happened, nav to login page
        nav.replace(Route::LoginView {});
    }
    rsx! {
        nav {
            div {
                class: "logo"
            }
            ul {
                class: "nav-links",
                id: "navLinks",
                Link {to: Route::HomeView {}, "Home" }
                Link {to: Route::InfoView {}, "Info" }
                Link {to: Route::ProjectsView {}, "Projects" }
                Link {to: Route::ContactView {}, "Contacts" }
            }
            div {
                class: "hamburger",
                i { class: "fas fa-bars" }
            }
        }


        // Hero section
        section {
            class: "hero",
            h1 { "Welcome to Lyra Dashboard" }
            p { "Manage your projects, view tickets, and stay updated in real-time." }
        }


        // Info section
        section {
                class: "info-section",
                id: "info",
                div {
                    class: "info-container",
                    h2 {
                        "System Overview"
                    }
                    p { "This section is for your custom information. You can replace this text with details about the company, current goals, or general instructions for the users." }
                    p {class: "margin-top:10px; color: var(--gray)", "Update status: ", span { style: "color: green", "Online"} }
                }
            }

        // Projects section
        section {
            class: "projects-section",
            id: "projects",
            h2 { class: "section-title", "Active Projects" }
            // need to change to foreach()
            div {
                class: "project-grid",
                div { class: "project-card",
                    h3 { "example title" }
                    p { "Updating the main marketing site assets." }
                    span { class: "ticket-badge", "5 Tickets" }
                }
            }
        }

        // chat bot section
        div {
            class: "chat-widget",
            h1 { "Chatbot" }
            //onclick: move |_| "Chatbot: Hello! How can I help you today?",
            i { class: "fas fa-comment-dots" }
        }

        div {
            h1{ "Whats new!" }
        }
        footer { "Footer" }
    }
}
