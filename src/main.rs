//! Scenar in the browser: the page around the core of the `scenar` library.

mod dnd;
mod state;
mod storage;
mod ui;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(ui::App);
}
