use dioxus::prelude::*;

use crate::api::{self, CreateTicket, Ticket};
use crate::router::RouteView;
use crate::utils::BackButton;

async fn fetch_tickets(id: i32) -> Result<Vec<Ticket>, String> {
    let tickets = api::get_json::<Vec<Ticket>>("/tickets").await?;
    Ok(tickets
        .into_iter()
        .filter(|t| t.project_id == id as i64)
        .collect())
}

#[component]
pub fn ProjectDetail(id: i32) -> Element {
    let mut tickets = use_resource(move || async move { fetch_tickets(id).await });
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let mut error = use_signal(|| Option::<String>::None);
    let mut is_loading = use_signal(|| false);

    let mut create_ticket = move || {
        let ticket_name = name();
        let ticket_description = description();
        if ticket_name.trim().is_empty() {
            error.set(Some("Enter a ticket name.".into()));
            return;
        }
        if is_loading() {
            return;
        }
        error.set(None);
        is_loading.set(true);
        spawn(async move {
            match api::post_json::<_, Ticket>(
                "/tickets",
                &CreateTicket {
                    name: ticket_name,
                    description: ticket_description,
                    project_id: id as i64,
                },
            )
            .await
            {
                Ok(_) => {
                    name.set(String::new());
                    description.set(String::new());
                    tickets.restart();
                }
                Err(err) => error.set(Some(err)),
            }
            is_loading.set(false);
        });
    };

    rsx! {
        div { class: "ticket-container",
            BackButton {}
            div { class: "page-header",
                h1 { class: "page-title", "Project tickets" }
                p { class: "page-subtitle", "Select a ticket to read the thread and add a comment." }
            }
            section { class: "card-base-plain", style: "margin-bottom: 18px;",
                h3 { class: "page-title", "New ticket" }
                form {
                    class: "form-stack",
                    onsubmit: move |evt| {
                        evt.prevent_default();
                        create_ticket();
                    },
                    if let Some(err) = error() {
                        p { class: "error", "{err}" }
                    }
                    input {
                        class: "input-field",
                        r#type: "text",
                        placeholder: "Name",
                        value: "{name}",
                        oninput: move |e| name.set(e.value()),
                        disabled: is_loading(),
                    }
                    textarea {
                        class: "textarea-field",
                        placeholder: "Description",
                        value: "{description}",
                        oninput: move |e| description.set(e.value()),
                        disabled: is_loading(),
                    }
                    button {
                        class: "btn-primary",
                        r#type: "submit",
                        disabled: is_loading(),
                        if is_loading() { "Creating…" } else { "Create ticket" }
                    }
                }
            }
            match &*tickets.read() {
                None => rsx! { p { class: "loading", "Loading tickets…" } },
                Some(Err(err)) => rsx! { p { class: "error", "{err}" } },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "empty-state", "No tickets in this project yet." }
                },
                Some(Ok(list)) => rsx! {
                    div { class: "ticket-grid",
                        for ticket in list.iter() {
                            Link {
                                to: RouteView::TicketDetail { id, ticket_id: ticket.id as i32 },
                                class: "project-card",
                                style: "color: inherit; text-decoration: none;",
                                h2 { class: "ticket-header", "{ticket.name}" }
                                p { class: "ticket-body", "{ticket.description}" }
                                span { class: "ticket-badge",
                                    if ticket.status.is_empty() { "queued" } else { "{ticket.status}" }
                                }
                                span { class: "ticket-badge", "{ticket.comments.len()} Comments" }
                            }
                        }
                    }
                },
            }
        }
    }
}
