```markdown
# Dioxus Text Saver

A simple, fast note-taking web app built entirely in **Rust** using the **Dioxus** framework. Text is saved to the browser's `localStorage` and persists across refreshes.

No backend required. Easy to run locally with **Trunk** and accessible securely from any device via **Tailscale** (no domain name needed).

## Features
- Large editable textarea
- Save button with browser storage
- Automatic loading of saved text
- Clear button
- Clean, responsive UI
- Fully client-side WebAssembly app

## Tech Stack
- Rust + Dioxus (0.6+)
- Trunk (for building and serving)
- gloo-storage (localStorage)

## Quick Start

### 1. Prerequisites
- Rust toolchain: https://rustup.rs/
- Install Trunk:
  ```bash
  cargo install trunk
  ```

### 2. Create the Project

```bash
cargo new dioxus-text-save --bin
cd dioxus-text-save
```

### 3. Update `Cargo.toml`

```toml
[package]
name = "dioxus-text-save"
version = "0.1.0"
edition = "2021"

[dependencies]
dioxus = { version = "0.6", features = ["web"] }
gloo-storage = "0.3"
```

### 4. Create `index.html` in the project root

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Dioxus Text Saver</title>
    <style>
        body { margin: 0; background: #f9f9f9; font-family: system-ui, sans-serif; }
    </style>
</head>
<body>
    <div id="main"></div>
</body>
</html>
```

### 5. Replace `src/main.rs` with the following code:

```rust
#![allow(non_snake_case)]

use dioxus::prelude::*;
use gloo_storage::{LocalStorage, Storage};

fn main() {
    launch(App);
}

#[component]
fn App() -> Element {
    let mut text = use_signal(|| {
        LocalStorage::get("saved_text").unwrap_or_else(|_| "Start typing here...\n\nYour notes are saved in the browser.".to_string())
    });

    let save = move |_| {
        LocalStorage::set("saved_text", text.read().clone()).ok();
        if let Some(window) = web_sys::window() {
            let _ = window.alert_with_message("✅ Text saved successfully!");
        }
    };

    let clear = move |_| {
        text.set(String::new());
        LocalStorage::delete("saved_text");
        if let Some(window) = web_sys::window() {
            let _ = window.alert_with_message("🗑️ All text cleared.");
        }
    };

    rsx! {
        div {
            style: "max-width: 900px; margin: 40px auto; padding: 20px; font-family: system-ui, sans-serif;",

            h1 { "📝 Dioxus Text Saver" }
            p { "Type below. Click Save to persist your text in the browser." }

            textarea {
                style: "width: 100%; height: 500px; padding: 16px; font-size: 1.1em; border: 2px solid #ddd; border-radius: 8px; resize: vertical; font-family: monospace;",
                value: "{text}",
                oninput: move |evt| text.set(evt.value()),
            }

            div {
                style: "margin-top: 20px; display: flex; gap: 16px; flex-wrap: wrap;",

                button {
                    onclick: save,
                    style: "padding: 14px 32px; font-size: 1.1em; background: #0066ff; color: white; border: none; border-radius: 8px; cursor: pointer;",
                    "💾 Save to Browser"
                }

                button {
                    onclick: clear,
                    style: "padding: 14px 32px; font-size: 1.1em; background: #ff4444; color: white; border: none; border-radius: 8px; cursor: pointer;",
                    "🗑️ Clear All"
                }
            }

            p { style: "margin-top: 24px; color: #555; font-size: 0.95em;",
                "Your text is stored locally in the browser. It stays private and works offline."
            }
        }
    }
}
```

## How to Run

```bash
# Start development server with hot reload
trunk serve
```

Open **http://localhost:8080** in your browser.

For production build:
```bash
trunk build --release
```
Output goes to the `dist/` folder — ready to host statically.

## Access from Other Devices with Tailscale

1. Install [Tailscale](https://tailscale.com/download) on this computer and any other devices.
2. Join the same Tailnet.
3. Run `trunk serve` (or serve the `dist/` folder with any static server).
4. On other devices, visit:
   - `http://<tailscale-ip>:8080`
   - or `http://your-machine-name.tailnet.ts.net:8080` (if MagicDNS is enabled)

No public ports, no domain, fully encrypted.

## Useful Links & Resources

- [Dioxus Documentation](https://dioxuslabs.com/learn/0.6/)
- [Dioxus Examples](https://dioxuslabs.com/examples/)
- [Trunk Documentation](https://trunkrs.dev/)
- [Tailscale Docs](https://tailscale.com/kb/)
- [gloo-storage Crate](https://crates.io/crates/gloo-storage)

## Ideas to Extend This App
- Auto-save on change (with debounce)
- Multiple saved notes / notebook system
- Markdown rendering preview
- Dark mode toggle
- Export / Import as `.txt` or `.md`
- Search through notes

This setup gives you a solid foundation for building more complex Rust frontend applications using Dioxus + Trunk + Tailscale.

Happy coding!
```

**Copy everything above** (from the first `# Dioxus Text Saver` to the end) and save it as `README.md`. Let me know if you want any additions!
