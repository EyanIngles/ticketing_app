mod pages;
mod router;
mod states;

use router::Route;

use dioxus::prelude::*;

#[derive(Props, PartialEq, Clone)]
struct LyraProps {
    tickets: String, //just plain string for the moment.
}

fn main() {
    dioxus::launch(|| rsx! { Router::<Route> {} });
}
