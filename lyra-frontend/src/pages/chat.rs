use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
struct ChatMessage {
    from_user: bool,
    text: String,
}

fn reply_for(input: &str) -> String {
    let msg = input.to_lowercase();
    if msg.contains("ticket") {
        "Open Projects, choose a project, then a ticket to read comments or add a note.".into()
    } else if msg.contains("project") {
        "Your active projects live under Projects. Each card shows how many tickets are open.".into()
    } else if msg.contains("hello") || msg.contains("hi") {
        "Hi — I’m the Lyra assistant. Ask about projects, tickets, or how the dashboard works.".into()
    } else if msg.contains("login") {
        "Use your email and password on the login screen. After that, Home is your overview.".into()
    } else {
        "I can help with projects, tickets, and getting around Lyra. Try asking about one of those.".into()
    }
}

#[component]
pub fn Chat() -> Element {
    let mut draft = use_signal(String::new);
    let mut messages = use_signal(|| {
        vec![ChatMessage {
            from_user: false,
            text: "Hi, I’m Lyra’s assistant. Ask me about projects, tickets, or the dashboard."
                .into(),
        }]
    });

    let mut send = move || {
        let text = draft().trim().to_string();
        if text.is_empty() {
            return;
        }
        messages.write().push(ChatMessage {
            from_user: true,
            text: text.clone(),
        });
        let reply = reply_for(&text);
        messages.write().push(ChatMessage {
            from_user: false,
            text: reply,
        });
        draft.set(String::new());
    };

    rsx! {
        div { class: "page-wrap",
            div { class: "page-header",
                h1 { class: "page-title", "Chat" }
                p { class: "page-subtitle", "Quick help while you work through projects and tickets." }
            }
            section { class: "chat-page",
                div { class: "chat-header",
                    h3 { "Lyra assistant" }
                    p { class: "muted", "Frontend preview — replies stay on this page." }
                }
                div { class: "chat-messages",
                    for (i, msg) in messages().into_iter().enumerate() {
                        div {
                            key: "{i}",
                            class: if msg.from_user { "bubble bubble-user" } else { "bubble bubble-bot" },
                            "{msg.text}"
                        }
                    }
                }
                form {
                    class: "chat-composer",
                    onsubmit: move |evt| {
                        evt.prevent_default();
                        send();
                    },
                    input {
                        class: "input-field",
                        r#type: "text",
                        placeholder: "Ask about projects or tickets…",
                        value: "{draft}",
                        oninput: move |e| draft.set(e.value()),
                    }
                    button { class: "btn-primary", r#type: "submit", "Send" }
                }
            }
        }
    }
}
