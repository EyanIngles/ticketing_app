use dioxus::prelude::*;

use crate::pages::{ChatView, ContactView, HomeView, InfoView, LoginView, ProjectsView};

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum Route {
    #[route("/login")]
    LoginView {},
    #[route("/")]
    HomeView,
    #[route("/chat")]
    ChatView {},
    #[route("/projects")]
    ProjectsView {},
    #[route("/contact")]
    ContactView {},
    #[route("/info")]
    InfoView {},
}
