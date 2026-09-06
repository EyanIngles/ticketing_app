use crate::router::RouteView;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Comments {
    pub id: i64,
    pub text: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Ticket {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub project_id: i64,
    #[serde(default)]
    pub comments: Vec<Comments>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Project {
    pub id: i32,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tickets: Vec<Ticket>,
}

async fn get_projects() -> Result<Vec<Project>, String> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://pi.tailcb4684.ts.net:3000/projects")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let mut projects = response
        .json::<Vec<Project>>()
        .await
        .map_err(|e| e.to_string())?;

    let ticket_reponse = client
        .get("https://pi.tailcb4684.ts.net:3000/tickets")
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let all_tickets = ticket_reponse
        .json::<Vec<Ticket>>()
        .await
        .map_err(|e| e.to_string())?;

    for project in projects.iter_mut() {
        project.tickets = all_tickets
            .iter()
            .filter(|ticket| ticket.project_id == project.id as i64)
            .cloned()
            .collect();
    }

    Ok(projects)
}

#[component]
fn ProjectList(preview: bool) -> Element {
    let projects_resource = use_resource(|| async move { get_projects().await });

    rsx! {
        match &*projects_resource.read() {
            Some(Ok(project_list)) if project_list.is_empty() => rsx! {
                p { class: "empty-state", "No projects yet." }
            },
            Some(Ok(project_list)) => {
                let list: Vec<Project> = if preview {
                    project_list.iter().take(3).cloned().collect()
                } else {
                    project_list.clone()
                };
                rsx! {
                    div { class: "project-grid",
                        for project in list {
                            Link {
                                to: RouteView::ProjectDetail { id: project.id },
                                class: "card-base-hover",
                                style: "color: inherit; text-decoration: none;",
                                h3 { "{project.name}" }
                                p { class: "muted", "{project.description}" }
                                span { class: "status-badge", "{project.tickets.len()} Tickets" }
                                ul { class: "card-list",
                                    for ticket in project.tickets.iter().take(4) {
                                        li { key: "{ticket.id}", "{ticket.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(err_msg)) => rsx! {
                p { class: "error", "Failed to load projects: {err_msg}" }
            },
            None => rsx! {
                p { class: "loading", "Loading projects…" }
            },
        }
    }
}

#[component]
pub fn Projects() -> Element {
    rsx! {
        div { class: "page-wrap",
            div { class: "page-header",
                h2 { class: "page-title", "Active projects" }
                p { class: "page-subtitle", "Open a project to view its tickets and comments." }
            }
            ProjectList { preview: false }
        }
    }
}

#[component]
pub fn ProjectPreview() -> Element {
    rsx! {
        section { class: "card-base-plain", id: "projects",
            h2 { class: "page-title", "Active projects" }
            p { class: "page-subtitle", "A snapshot of work in progress." }
            ProjectList { preview: true }
            div { style: "margin-top: 18px;",
                Link { to: RouteView::Projects {}, class: "btn-secondary", "See all projects" }
            }
        }
    }
}
