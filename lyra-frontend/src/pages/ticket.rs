use dioxus::prelude::*;

use crate::api::{self, CreateComment, Ticket};
use crate::utils::{use_interval_tick, BackButton};
use super::permissions::PendingPermissions;

async fn get_ticket(ticket_id: i32) -> Result<Ticket, String> {
    api::get_json::<Ticket>(&format!("/tickets/{ticket_id}")).await
}

#[component]
pub fn TicketDetail(id: i32, ticket_id: i32) -> Element {
    let tick = use_interval_tick(5_000);
    let mut ticket = use_resource(move || {
        let _ = tick();
        async move { get_ticket(ticket_id).await }
    });
    let mut comment_write = use_signal(String::new);
    let mut is_loading = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut action_busy = use_signal(|| false);
    let _id = id;

    let mut post_comment = move || {
        let text = comment_write();
        if text.trim().is_empty() || is_loading() {
            return;
        }
        is_loading.set(true);
        error.set(None);
        spawn(async move {
            match api::post_json::<_, crate::api::Comment>(
                &format!("/tickets/{ticket_id}/comments"),
                &CreateComment { text },
            )
            .await
            {
                Ok(_) => {
                    comment_write.set(String::new());
                    ticket.restart();
                }
                Err(err) => error.set(Some(err)),
            }
            is_loading.set(false);
        });
    };

    let mut run_action = move |path: &'static str| {
        if action_busy() {
            return;
        }
        action_busy.set(true);
        error.set(None);
        spawn(async move {
            match api::post_empty::<Ticket>(&format!("/tickets/{ticket_id}{path}")).await {
                Ok(_) => ticket.restart(),
                Err(err) => error.set(Some(err)),
            }
            action_busy.set(false);
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
                        p { class: "muted",
                            "status: "
                            if t.status.is_empty() { "queued" } else { "{t.status}" }
                            if !t.last_model.is_empty() {
                                " · last_model: {t.last_model}"
                            }
                        }
                        p { class: "muted",
                            "github_pr_url: "
                            if t.github_pr_url.is_empty() { "(none)" } else { "{t.github_pr_url}" }
                        }
                        div { class: "hero-actions", style: "margin: 16px 0;",
                            button {
                                class: "btn-secondary",
                                r#type: "button",
                                disabled: action_busy(),
                                onclick: move |_| run_action("/actions/request_pr"),
                                "Request PR"
                            }
                            button {
                                class: "btn-secondary",
                                r#type: "button",
                                disabled: action_busy(),
                                onclick: move |_| run_action("/actions/close"),
                                "Close"
                            }
                            button {
                                class: "btn-primary",
                                r#type: "button",
                                disabled: action_busy(),
                                onclick: move |_| run_action("/actions/deploy"),
                                "Deploy"
                            }
                        }
                        h3 { style: "margin: 24px 0 8px;", "Comments" }
                        if t.comments.is_empty() {
                            p { class: "muted", "No comments yet. Start the thread below." }
                        }
                        for comment in t.comments.iter() {
                            div { class: "comment-card-plain", key: "{comment.id}",
                                p { class: "comment-meta",
                                    if comment.display.is_empty() {
                                        "Comment #{comment.id}"
                                    } else {
                                        "{comment.display}"
                                    }
                                }
                                p { "{comment.text}" }
                            }
                        }
                    },
                    Some(Err(e)) => rsx! { p { class: "error", "{e}" } },
                }
                if let Some(err) = error() {
                    p { class: "error", "{err}" }
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
            PendingPermissions { ticket_id: Some(ticket_id as i64) }
        }
    }
}
