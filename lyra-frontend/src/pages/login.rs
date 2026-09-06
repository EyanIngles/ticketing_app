use crate::pages::MAIN_CSS;
use crate::router::RouteView;
use crate::states::IS_LOGGED_IN;
use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, Clone)]
struct _User {
    email: String,
}

pub fn _set_jwt(jwt_key: String, bear_token: String) {
    LocalStorage::set(jwt_key, bear_token).unwrap();
}
pub fn _del_jwt(jwt_key: String) {
    LocalStorage::delete(jwt_key);
}

pub fn _get_jwt() -> String {
    LocalStorage::get("JWT").unwrap()
}

async fn login_attempt(email: String, password: String) -> Result<(), String> {
    let local_storage = LocalStorage::length();
    if local_storage == 0 {
        _set_jwt("JWT".to_string(), "length was set to 0 ".to_string());
    } else {
        _del_jwt("JWT".to_string());
    }

    let client = reqwest::Client::new();
    let payload = LoginRequest { email, password };

    let response = client
        .post("https://pi.tailcb4684.ts.net:3000/login")
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status() == 302 {
        Ok(())
    } else {
        Err(format!("Login failed with status: {}", response.status()))
    }
}

#[component]
pub fn Login() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut is_loading = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let nav = navigator();

    let mut submit = move |_| {
        let email = username();
        let pwd = password();
        if email.trim().is_empty() || pwd.trim().is_empty() {
            error.set(Some("Enter both username and password.".into()));
            return;
        }
        error.set(None);
        is_loading.set(true);

        spawn(async move {
            match login_attempt(email, pwd).await {
                Ok(_) => {
                    *IS_LOGGED_IN.write() = true;
                    is_loading.set(false);
                    nav.push(RouteView::Home {});
                }
                Err(err) => {
                    error.set(Some(err));
                    is_loading.set(false);
                }
            }
        });
    };

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;800&display=swap",
        }
        div { class: "auth-page",
            div { class: "login-container",
                h1 { class: "logo", "LYRA" }
                p { class: "page-subtitle", "Sign in to open the dashboard" }
                div { class: "card-base-plain",
                    form {
                        class: "login-form",
                        onsubmit: move |evt| {
                            evt.prevent_default();
                            submit(());
                        },
                        if let Some(err) = error() {
                            p { class: "error", "{err}" }
                        }
                        input {
                            class: "input-field",
                            r#type: "text",
                            placeholder: "Username or email",
                            value: "{username}",
                            oninput: move |e| username.set(e.value()),
                            disabled: is_loading(),
                        }
                        input {
                            class: "input-field",
                            r#type: "password",
                            placeholder: "Password",
                            value: "{password}",
                            oninput: move |e| password.set(e.value()),
                            disabled: is_loading(),
                        }
                        button {
                            class: "btn-primary",
                            r#type: "submit",
                            disabled: is_loading(),
                            if is_loading() { "Signing in…" } else { "Login" }
                        }
                    }
                }
            }
        }
    }
}
