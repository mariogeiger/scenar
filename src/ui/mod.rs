//! The page. Its grammar is the same everywhere: grey capitals are
//! headings, ink is what opens a place in the story, `[brackets]` are
//! buttons, `⋯` opens the actions of a block, and whatever opens does so on
//! a click, over the page, so that nothing ever moves under the pointer.
//!
//! Its columns follow the width: on a phone the script alone, the timeline
//! and the panel opening over it; from a desktop on, the timeline beside
//! it; on an ultra-wide screen, the script in pages and the panel too.

mod footer;
mod frieze;
mod panel;
mod popup;
mod script;
mod timeline;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::state::{Popup, State};
use crate::storage;

/// The `⋯` button that opens the actions of the block `id`.
fn more(st: State, id: u64) -> impl IntoView {
    view! {
        <button
            class="more"
            class:open=move || st.is_open(Popup::Menu(id))
            title="Actions"
            aria-label="Actions"
            draggable="false"
            on:click=move |ev| {
                ev.stop_propagation();
                st.toggle_popup(Popup::Menu(id), &ev);
            }
        >
            "⋯"
        </button>
    }
}

#[component]
pub fn App() -> impl IntoView {
    let st = State::new();
    provide_context(st);
    let _escape = window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.key() == "Escape" {
            st.close_popup();
        }
    });
    let picker = NodeRef::<leptos::html::Input>::new();
    let title = move || st.story.with(|s| s.title.clone());
    let import = move |_| {
        let Some(input) = picker.get() else { return };
        spawn_local(async move {
            if let Some(text) = storage::picked_text(&input).await {
                st.replace(&text);
            }
            input.set_value("");
        });
    };
    let warnings = move || st.analysis.with(|a| a.warnings.len());
    view! {
        <div class="app">
            <header class="topbar">
                <button class="btn" on:click=move |ev| st.toggle_popup(Popup::Timeline, &ev)>"☰"</button>
                <span class="logo">"Scenar"</span>
                <button class="btn" class:warns=move || { warnings() > 0 } on:click=move |ev| st.toggle_popup(Popup::Panel, &ev)>
                    {move || format!("⚠ {}", warnings())}
                </button>
            </header>
            <nav class="side">
                <div class="logo">"Scenar"</div>
                <input
                    class="title story-title"
                    aria-label="Titre du scénario"
                    prop:value=title
                    on:input=move |ev| st.story.update(|s| s.title = event_target_value(&ev))
                />
                <timeline::Timeline/>
                <div class="actions">
                    <button class="btn" on:click=move |_| st.reset()>"Nouveau"</button>
                    <button class="btn" on:click=move |_| {
                        if let Some(p) = picker.get() {
                            p.click();
                        }
                    }>"Importer"</button>
                    <button class="btn" on:click=move |_| {
                        if st.story.with_untracked(storage::export).is_err() {
                            storage::alert("L'export a échoué.");
                        }
                    }>"Exporter"</button>
                    <button class="btn" title="Clair / sombre" on:click=move |_| st.dark.update(|d| *d = !*d)>
                        {move || if st.dark.get() { "☀" } else { "☾" }}
                    </button>
                </div>
                <input class="hidden" type="file" accept=".json,application/json" node_ref=picker on:change=import/>
            </nav>
            <frieze::Frieze/>
            <script::Script/>
            <aside class="panel">
                <panel::Panel/>
            </aside>
            <footer::Footer/>
        </div>
        <popup::Popups/>
    }
}
