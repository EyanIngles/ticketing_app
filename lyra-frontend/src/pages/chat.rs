use dioxus::prelude::*;

#[component]
pub fn ChatView() -> Element {
    rsx!( div {
        h1 { "Chat with AI" }
    })
}
