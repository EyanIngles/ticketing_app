use dioxus::prelude::*;

use crate::pages::projects::Ticket;
use crate::router::RouteView;
use crate::utils::BackButton;

async fn fetch_tickets(id: i32) -> Result<Vec<Ticket>, String> {
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

    Ok(tickets
        .into_iter()
        .filter(|t| t.project_id == id as i64)
        .collect())
}

#[component]
pub fn ProjectDetail(id: i32) -> Element {
    let tickets = use_resource(move || async move { fetch_tickets(id).await });

    rsx! {
        div { class: "ticket-container",
            BackButton {}
            div { class: "page-header",
                h1 { class: "page-title", "Project tickets" }
                p { class: "page-subtitle", "Select a ticket to read the thread and add a comment." }
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
                                span { class: "ticket-badge", "{ticket.comments.len()} Comments" }
                            }
                        }
                    }
                },
            }
        }
    }
}
