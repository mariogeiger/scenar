//! The page: the timeline on the left, the scene on the right with its
//! dependencies in the margin, and the elements and warnings in a footer.

mod footer;
mod scene;
mod timeline;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::state::State;
use crate::storage;

/// A small button that acts without letting the click reach its row.
fn tool(glyph: &'static str, title: &'static str, act: impl Fn() + 'static) -> impl IntoView {
    view! {
        <button class="tool" title=title on:click=move |ev| {
            ev.stop_propagation();
            act();
        }>{glyph}</button>
    }
}

#[component]
pub fn App() -> impl IntoView {
    let st = State::new();
    provide_context(st);
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
    view! {
        <div class="app">
            <nav>
                <div class="logo">"Scenar"</div>
                <input
                    class="story-title"
                    aria-label="Titre du scénario"
                    prop:value=title
                    on:input=move |ev| st.story.update(|s| s.title = event_target_value(&ev))
                />
                <timeline::Timeline/>
                <timeline::Bin/>
                <div class="actions">
                    <button on:click=move |_| st.reset()>"Nouveau"</button>
                    <button on:click=move |_| {
                        if let Some(p) = picker.get() {
                            p.click();
                        }
                    }>"Importer"</button>
                    <button on:click=move |_| {
                        if st.story.with_untracked(storage::export).is_err() {
                            storage::alert("L'export a échoué.");
                        }
                    }>"Exporter"</button>
                    <button title="Clair / sombre" on:click=move |_| st.dark.update(|d| *d = !*d)>
                        {move || if st.dark.get() { "☀" } else { "☾" }}
                    </button>
                </div>
                <input class="hidden" type="file" accept=".json,application/json" node_ref=picker on:change=import/>
            </nav>
            <main>
                <scene::SceneView/>
            </main>
            <footer::Footer/>
        </div>
    }
}
