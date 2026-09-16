use crate::api::{self, oauth_client_id};
use crate::pages::MAIN_CSS;
use crate::router::RouteView;
use crate::states::IS_LOGGED_IN;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct AuthorizeRequest {
    username: String,
    password: String,
    client_id: String,
    code_challenge: String,
    code_challenge_method: String,
}

#[derive(Deserialize)]
struct AuthorizeResponse {
    code: String,
}

#[derive(Serialize)]
struct TokenRequest {
    grant_type: String,
    client_id: String,
    code: String,
    code_verifier: String,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

async fn login_attempt(username: String, password: String) -> Result<(), String> {
    let _base = api::api_base()?;
    api::clear_jwt_token();
    let (code_verifier, code_challenge) = api::pkce_s256_pair()?;
    let client_id = oauth_client_id().to_string();

    let authorize: AuthorizeResponse = api::post_json(
        "/oauth/authorize",
        &AuthorizeRequest {
            username,
            password,
            client_id: client_id.clone(),
            code_challenge,
            code_challenge_method: "S256".into(),
        },
    )
    .await?;

    let token: TokenResponse = api::post_json(
        "/oauth/token",
        &TokenRequest {
            grant_type: "authorization_code".into(),
            client_id,
            code: authorize.code,
            code_verifier,
        },
    )
    .await?;

    if token.access_token.trim().is_empty() {
        return Err("missing access_token".into());
    }
    api::set_jwt_token(&token.access_token);
    Ok(())
}

#[component]
pub fn Login() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut is_loading = use_signal(|| false);
    let mut error = use_signal(|| api::api_base().err());
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
                            placeholder: "Username",
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
