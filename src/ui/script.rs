//! The script: the whole story in one flow, act by act and scene by scene,
//! each beat a text field numbered in the left margin and annotated in the
//! right one with its elements and where their dependencies lead. On an
//! ultra-wide screen the flow is set in pages, side by side.

use leptos::prelude::*;
use scenar::engine::Kind;
use scenar::model::Level;

use super::more;
use crate::dnd;
use crate::state::{Note, Popup, State};
use crate::storage::dom_id;

/// The living children of `id`, or the acts for `None`.
fn children(st: State, id: Option<u64>, level: Level) -> Memo<Vec<u64>> {
    Memo::new(move |_| {
        st.live.with(|l| l.iter().filter(|n| n.level == level && n.parent == id).map(|n| n.id).collect())
    })
}

#[component]
pub fn Script() -> impl IntoView {
    let st = expect_context::<State>();
    let acts = children(st, None, Level::Act);
    let add_act = move |_| {
        if let Some(act) = st.story.try_update(|s| s.add(None, None)).flatten() {
            st.reveal(act);
        }
    };
    // A box that scrolls only across, as pages do, scrolls across with the wheel too.
    let wheel = move |ev: web_sys::WheelEvent| {
        let Some(root) = st.script.get_untracked() else { return };
        if root.scroll_width() > root.client_width() && root.scroll_height() <= root.client_height() {
            root.set_scroll_left(root.scroll_left() + ev.delta_y() as i32);
            ev.prevent_default();
        }
    };
    view! {
        <main class="script">
            <div
                class="pages"
                node_ref=st.script
                on:scroll=move |_| {
                    st.spy();
                    if st.popup.with_untracked(|p| matches!(p, Some((Popup::Menu(_), _)))) {
                        st.close_popup();
                    }
                }
                on:wheel=wheel
            >
                <For each=move || acts.get() key=|id| *id let:act>
                    <Act id=act/>
                </For>
                <button class="btn add" on:click=add_act>"+ acte"</button>
            </div>
        </main>
    }
}

/// A title the reader edits in place.
fn title_input(st: State, id: u64, class: &'static str, label: &'static str) -> impl IntoView {
    view! {
        <input
            class=format!("title {class}")
            aria-label=label
            prop:value=move || st.story.with(|s| s.find(id).map(|n| n.title.clone()).unwrap_or_default())
            on:input=move |ev| st.story.update(|s| s.set_title(id, event_target_value(&ev)))
        />
    }
}

#[component]
fn Act(id: u64) -> impl IntoView {
    let st = expect_context::<State>();
    let scenes = children(st, Some(id), Level::Scene);
    let label = move || st.live_node(id).map(|n| format!("Acte {} —", n.label));
    view! {
        <div
            class=move || format!("act-head{}", dnd::class(st, id))
            id=dom_id(id)
            on:dragover=move |ev| dnd::over(st, id, &ev)
            on:drop=move |ev| dnd::drop(st, &ev)
        >
            <span class="grip" draggable="true" title="Glisser pour déplacer"
                on:dragstart=move |ev| dnd::start(st, id, &ev) on:dragend=move |_| dnd::end(st)>
                {label}
            </span>
            {title_input(st, id, "act-title", "Titre de l'acte")}
            {more(st, id)}
        </div>
        <For each=move || scenes.get() key=|id| *id let:scene>
            <Scene id=scene/>
        </For>
    }
}

#[component]
fn Scene(id: u64) -> impl IntoView {
    let st = expect_context::<State>();
    let beats = children(st, Some(id), Level::Beat);
    let label = move || st.live_node(id).map(|n| n.label);
    let add = move |_| {
        if let Some(new) = st.story.try_update(|s| s.add(Some(id), None)).flatten() {
            st.reveal(new);
        }
    };
    view! {
        <section class="scene" id=dom_id(id)>
            <div
                class=move || format!("slug{}", dnd::class(st, id))
                on:dragover=move |ev| dnd::over(st, id, &ev)
                on:drop=move |ev| dnd::drop(st, &ev)
            >
                <span class="grip" draggable="true" title="Glisser pour déplacer"
                    on:dragstart=move |ev| dnd::start(st, id, &ev) on:dragend=move |_| dnd::end(st)>
                    {label}
                </span>
                {title_input(st, id, "scene-title", "Titre de la scène")}
                {more(st, id)}
            </div>
            <For each=move || beats.get() key=|id| *id let:beat>
                <Beat id=beat/>
            </For>
            <button class="btn add" on:click=add>"+ beat"</button>
        </section>
    }
}

#[component]
fn Beat(id: u64) -> impl IntoView {
    let st = expect_context::<State>();
    let text = Memo::new(move |_| st.story.with(|s| s.find(id).map(|n| n.text.clone()).unwrap_or_default()));
    let number = move || st.live_node(id).map(|n| n.number);
    let class = move || format!("beat{}{}", if st.holds_focus(id) { " hl" } else { "" }, dnd::class(st, id));
    let notes = Memo::new(move |_| st.notes(id));
    let note = move |n: Note| {
        let kind = match n.kind {
            Kind::Setup => "setup",
            Kind::Mention => "mention",
            Kind::Payoff => "payoff",
        };
        let focused = {
            let key = n.key.clone();
            move || st.focus.get().as_deref() == Some(key.as_str())
        };
        let key = n.key;
        view! {
            <button class=format!("note {kind}") class:warns=n.warns class:focused=focused
                title="Surligner ses beats"
                on:click=move |_| st.toggle_focus(key.clone())>
                <b>{n.name}</b>
                " "
                {n.kind.verb()}
                {(!n.relation.is_empty()).then(|| view! { <span class="relation">{n.relation}</span> })}
            </button>
        }
    };
    view! {
        <div
            class=class
            id=dom_id(id)
            on:dragover=move |ev| dnd::over(st, id, &ev)
            on:drop=move |ev| dnd::drop(st, &ev)
        >
            <div class="margin">
                {more(st, id)}
                <span class="grip n" draggable="true" title="Glisser pour déplacer"
                    on:dragstart=move |ev| dnd::start(st, id, &ev) on:dragend=move |_| dnd::end(st)>
                    {number}
                </span>
            </div>
            <div class="grow" data-value=move || text.get()>
                <textarea
                    rows="1"
                    aria-label="Texte du beat"
                    placeholder="Écris le beat. @nom déclare un élément, !nom le paie."
                    prop:value=move || text.get()
                    on:input=move |ev| st.story.update(|s| s.set_text(id, event_target_value(&ev)))
                ></textarea>
            </div>
            <div class="notes">{move || notes.get().into_iter().map(note).collect_view()}</div>
        </div>
    }
}
