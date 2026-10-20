//! The scene on screen: its act and title, and its beats as a stack of text
//! fields, each numbered in the left margin and annotated in the right one
//! with its elements and where their dependencies lead.

use leptos::prelude::*;
use scenar::engine::Kind;
use scenar::model::Side;

use super::tool;
use crate::state::{Note, State, dnd};

#[component]
pub fn SceneView() -> impl IntoView {
    let st = expect_context::<State>();
    move || match st.scene.get() {
        Some(id) => view! { <Scene id/> }.into_any(),
        None => view! { <p class="empty">"Aucune scène. Ajoute un acte dans la timeline."</p> }.into_any(),
    }
}

#[component]
fn Scene(id: u64) -> impl IntoView {
    let st = expect_context::<State>();
    let act = Memo::new(move |_| st.story.with(|s| s.parent(id)));
    let title_of = move |block: Option<u64>| {
        block.and_then(|b| st.story.with(|s| s.find(b).map(|n| n.title.clone()))).unwrap_or_default()
    };
    let retitle = move |block: Option<u64>, ev: leptos::ev::Event| {
        if let Some(b) = block {
            st.story.update(|s| s.set_title(b, event_target_value(&ev)));
        }
    };
    let act_label = move || act.get().and_then(|a| st.live_node(a)).map(|n| format!("Acte {} ·", n.label));
    let beats = Memo::new(move |_| {
        st.live.with(|l| l.iter().filter(|n| n.parent == Some(id)).map(|n| n.id).collect::<Vec<_>>())
    });
    let add = move |_| {
        if let Some(new) = st.story.try_update(|s| s.add(Some(id), None)).flatten() {
            st.show_beat(new);
        }
    };
    view! {
        <div class="crumb">
            <span>{act_label}</span>
            <input
                class="act-title"
                aria-label="Titre de l'acte"
                prop:value=move || title_of(act.get())
                on:input=move |ev| retitle(act.get_untracked(), ev)
            />
        </div>
        <input
            class="scene-title"
            aria-label="Titre de la scène"
            prop:value=move || title_of(Some(id))
            on:input=move |ev| retitle(Some(id), ev)
        />
        <For each=move || beats.get() key=|b| *b let:beat>
            <Beat id=beat/>
        </For>
        <button class="add-beat" on:click=add>"+ beat"</button>
    }
}

#[component]
fn Beat(id: u64) -> impl IntoView {
    let st = expect_context::<State>();
    let text = Memo::new(move |_| st.story.with(|s| s.find(id).map(|n| n.text.clone()).unwrap_or_default()));
    let number = move || st.live_node(id).map(|n| n.number);
    let class = move || format!("beat{}{}", if st.highlighted(id) { " hl" } else { "" }, dnd::class(st, id));
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
            <div class=format!("note {kind}") class:warns=n.warns class:focused=focused
                on:click=move |_| st.toggle_focus(key.clone())>
                <b>{n.name}</b>
                " "
                {n.kind.verb()}
                {(!n.relation.is_empty()).then(|| view! { <br/><span class="relation">{n.relation}</span> })}
            </div>
        }
    };
    view! {
        <div
            class=class
            id=format!("beat-{id}")
            on:dragover=move |ev| dnd::over(st, id, &ev)
            on:drop=move |ev| dnd::drop(st, &ev)
        >
            <div class="margin">
                <div
                    class="n"
                    draggable="true"
                    title="Glisser pour déplacer"
                    on:dragstart=move |ev| dnd::start(st, id, &ev)
                    on:dragend=move |_| dnd::end(st)
                >
                    {number}
                </div>
                <span class="tools">
                    {tool("↑", "Monter", move || { st.story.update(|s| { s.step(id, Side::Before); }); })}
                    {tool("↓", "Descendre", move || { st.story.update(|s| { s.step(id, Side::After); }); })}
                    {tool("×", "Archiver", move || st.archive(id))}
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
