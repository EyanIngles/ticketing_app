use dioxus::prelude::*;

#[component]
pub fn BackButton() -> Element {
    rsx! {
        div { class: "back-button-container",
            button {
                class: "back-button",
                r#type: "button",
                onclick: move |_| {
                    navigator().go_back();
                },
                "Back"
            }
        }
    }
}
