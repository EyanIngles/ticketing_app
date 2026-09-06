use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::pages::Ticket;
use crate::utils::BackButton;

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateComment {
    pub text: String,
}

async fn write_comment_attempt(ticket_id: i32, comment: String) {
    let client = reqwest::Client::new();
    let body = CreateComment { text: comment };
    let _ = client
        .post(format!(
            "https://pi.tailcb4684.ts.net:3000/tickets/{ticket_id}/comments"
        ))
        .json(&body)
        .send()
        .await;
}

async fn get_ticket(ticket_id: i32) -> Result<Ticket, String> {
    let client = reqwest::Client::new();

    let ticket_response = client
        .get("https://pi.tailcb4684.ts.net:3000/tickets")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let tickets = ticket_response
        .json::<Vec<Ticket>>()
        .await
        .map_err(|e| e.to_string())?;

    tickets
        .into_iter()
        .find(|t| t.id == ticket_id as i64)
        .ok_or_else(|| format!("Ticket {} not found", ticket_id))
}

#[component]
pub fn TicketDetail(id: i32, ticket_id: i32) -> Element {
    let mut ticket = use_resource(move || async move { get_ticket(ticket_id).await });
    let mut comment_write = use_signal(String::new);
    let mut is_loading = use_signal(|| false);
    let _id = id;

    let mut post_comment = move || {
        let text = comment_write();
        if text.trim().is_empty() || is_loading() {
            return;
        }
        is_loading.set(true);
        spawn(async move {
            write_comment_attempt(ticket_id, text).await;
            is_loading.set(false);
            comment_write.set(String::new());
            ticket.restart();
        });
    };

    rsx! {
        div { class: "page-wrap",
            BackButton {}
            section { class: "card-base-plain",
                match &*ticket.read() {
                    None => rsx! { p { class: "loading", "Loading ticket…" } },
                    Some(Ok(t)) => rsx! {
                        h1 { class: "page-title", "{t.name}" }
                        p { class: "page-subtitle", "{t.description}" }
                        h3 { style: "margin: 24px 0 8px;", "Comments" }
                        if t.comments.is_empty() {
                            p { class: "muted", "No comments yet. Start the thread below." }
                        }
                        for comment in t.comments.iter() {
                            div { class: "comment-card-plain", key: "{comment.id}",
                                p { class: "comment-meta", "Comment #{comment.id}" }
                                p { "{comment.text}" }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                }
                form {
                    class: "comment-form",
                    onsubmit: move |evt| {
                        evt.prevent_default();
                        post_comment();
                    },
                    div { class: "comment-input-wrap",
                        textarea {
                            class: "comment-input",
                            placeholder: "Write a comment",
                            value: "{comment_write}",
                            oninput: move |write| comment_write.set(write.value()),
                            onkeydown: move |evt| {
                                if evt.key() == Key::Enter && !evt.modifiers().shift() {
                                    evt.prevent_default();
                                    post_comment();
                                }
                            }
                        }
                        button {
                            class: "comment-button",
                            r#type: "submit",
                            disabled: is_loading(),
                            if is_loading() { "Sending…" } else { "Send" }
                        }
                    }
                }
            }
        }
    }
}
