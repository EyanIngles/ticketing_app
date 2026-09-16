use dioxus::prelude::*;

use crate::api::{self, PermissionRequest};
use crate::utils::use_interval_tick;

#[component]
pub fn PendingPermissions(ticket_id: Option<i64>) -> Element {
    let tick = use_interval_tick(5_000);
    let mut list = use_resource(move || {
        let _ = tick();
        async move { api::get_json::<Vec<PermissionRequest>>("/permissions?status=pending").await }
    });
    let mut error = use_signal(|| Option::<String>::None);
    let mut busy_id = use_signal(|| Option::<i64>::None);

    let mut decide = move |id: i64, approve: bool| {
        if busy_id().is_some() {
            return;
        }
        busy_id.set(Some(id));
        error.set(None);
        spawn(async move {
            let path = if approve {
                format!("/permissions/{id}/approve")
            } else {
                format!("/permissions/{id}/deny")
            };
            match api::post_empty::<PermissionRequest>(&path).await {
                Ok(_) => {
                    error.set(None);
                    list.restart();
                }
                Err(err) => error.set(Some(err)),
            }
            busy_id.set(None);
        });
    };

    rsx! {
        section { class: "card-base-plain", style: "margin-top: 18px;",
            h3 { class: "page-title", "Pending permissions" }
            p { class: "page-subtitle", "Human JWT only. Approve or deny OpenCode tool requests." }
            if let Some(err) = error() {
                p { class: "error", "{err}" }
            }
            match &*list.read() {
                None => rsx! { p { class: "loading", "Loading permissions…" } },
                Some(Err(err)) => rsx! { p { class: "error", "{err}" } },
                Some(Ok(rows)) => {
                    let filtered: Vec<PermissionRequest> = rows
                        .iter()
                        .filter(|row| ticket_id.map(|id| row.ticket_id == id).unwrap_or(true))
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        rsx! { p { class: "muted", "No pending permissions." } }
                    } else {
                        rsx! {
                            for row in filtered {
                                div { class: "comment-card-plain", key: "{row.id}",
                                    p { class: "comment-meta",
                                        "{row.display} · ticket #{row.ticket_id} · {row.tool}"
                                    }
                                    p { "{row.payload}" }
                                    div { class: "hero-actions", style: "margin-top: 12px;",
                                        button {
                                            class: "btn-primary",
                                            r#type: "button",
                                            disabled: busy_id() == Some(row.id),
                                            onclick: move |_| decide(row.id, true),
                                            "Approve"
                                        }
                                        button {
                                            class: "btn-secondary",
                                            r#type: "button",
                                            disabled: busy_id() == Some(row.id),
                                            onclick: move |_| decide(row.id, false),
                                            "Deny"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
