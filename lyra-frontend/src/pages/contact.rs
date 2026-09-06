use dioxus::prelude::*;

#[component]
pub fn Contact() -> Element {
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut message = use_signal(String::new);
    let mut sent = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "page-wrap",
            div { class: "page-header",
                h1 { class: "page-title", "Contact" }
                p { class: "page-subtitle", "Questions about Lyra, a project, or an account? Send a note and we’ll get back to you." }
            }

            div { class: "feature-grid",
                section { class: "contact-card",
                    h3 { "Support" }
                    p { class: "muted", "For ticket issues, login problems, or dashboard feedback." }
                    p { "support@lyra.local" }
                }
                section { class: "contact-card",
                    h3 { "Hours" }
                    p { class: "muted", "Weekdays 9:00–17:00. Messages sent after hours are picked up the next working day." }
                }
            }

            section { class: "card-base-plain", style: "margin-top: 24px;",
                if sent() {
                    p { class: "success", "Thanks {name()}. Your message is ready to send — we’ll follow up at {email()}." }
                } else {
                    form {
                        class: "form-stack",
                        onsubmit: move |evt| {
                            evt.prevent_default();
                            if name().trim().is_empty() || email().trim().is_empty() || message().trim().is_empty() {
                                error.set(Some("Please fill in name, email, and a message.".into()));
                                return;
                            }
                            error.set(None);
                            sent.set(true);
                        },
                        if let Some(err) = error() {
                            p { class: "error", "{err}" }
                        }
                        input {
                            class: "input-field",
                            r#type: "text",
                            placeholder: "Your name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                        input {
                            class: "input-field",
                            r#type: "email",
                            placeholder: "Email address",
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                        }
                        textarea {
                            class: "textarea-field",
                            placeholder: "How can we help?",
                            value: "{message}",
                            oninput: move |e| message.set(e.value()),
                        }
                        button { class: "btn-primary", r#type: "submit", "Send message" }
                    }
                }
            }
        }
    }
}
