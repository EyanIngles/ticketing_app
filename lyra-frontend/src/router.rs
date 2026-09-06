use dioxus::prelude::*;

use crate::pages::{
    Chat, Contact, Footer, Home, Info, Login, Navbar, ProjectDetail, Projects, TicketDetail,
    MAIN_CSS,
};

#[derive(Clone, Debug, PartialEq, Routable)]
#[rustfmt::skip]
pub enum RouteView {
    #[route("/login")]
    Login {},
    #[layout(AppLayout)]
        #[route("/")]
        Home {},
        #[route("/chat")]
        Chat {},
        #[route("/projects")]
        Projects {},
        #[route("/projects/:id")]
        ProjectDetail { id: i32 },
        #[route("/projects/:id/:ticket_id")]
        TicketDetail { id: i32, ticket_id: i32 },
        #[route("/contact")]
        Contact {},
        #[route("/info")]
        Info {},
}

#[component]
pub fn AppLayout() -> Element {
    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;800&display=swap",
        }
        Navbar {}
        main { class: "page-shell",
            Outlet::<RouteView> {}
        }
        Footer {}
    }
}
