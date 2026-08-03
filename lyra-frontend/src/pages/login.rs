// this file is a starting view and login dealings but main idea is this is a viewing component.
use crate::states::IS_LOGGED_IN;

use crate::Route;

use dioxus::prelude::*;
static CSS: Asset = asset!("/assets/main.css");

pub struct LoginTest {
    pub id: u64,
    pub username: String,
    password: String,
}

impl LoginTest {
    fn new(username: String, password: String) -> LoginTest {
        LoginTest {
            id: 1,
            username: username,
            password: password,
        }
    }
}

#[component]
pub fn LoginView() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut is_loading = use_signal(|| false);

    let nav = navigator();
    rsx! {
        document::Stylesheet { href: CSS }

        h1 {class: "LoginHeader",
        "Welcome to Lyra tickets\nPlease Login"}
        div {class: "loginContainer",
            input {
                class: "loginUsername",
                r#type: "text",
                placeholder: "Enter Username",
                value: "{username.read()}",
                oninput: move |username_entry| username.set(username_entry.value()),
                disabled: *is_loading.read(),
            }
            input {
                class: "loginPassword",
                r#type: "text",
                placeholder: "Enter Password",
                value: "{password.read()}",
                oninput: move |pwd| password.set(pwd.value()),
                disabled: *is_loading.read(),
            }
            button {class: "loginButton",
                onclick: move |_| { info!("{0}", username.read());
                is_loading.set(true);
                *IS_LOGGED_IN.write() = true;
                nav.replace(Route::HomeView {});
                },
                disabled: *is_loading.read(),
                if *is_loading.read() { "Logging in..." } else { "Login" },
            }
        }
    }
}
