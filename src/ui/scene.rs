//! The scene on screen: its act and title, and its beats as a stack of text
//! fields, each numbered in the margin and tagged below with its elements.

use leptos::prelude::*;
use scenar::engine::Kind;
use scenar::model::Side;

use super::tool;
use crate::state::{State, dnd};

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
    let tags = move || {
        let Some(beat) = st.beat_index(id) else { return Vec::new() };
        st.analysis.with(|a| {
            a.in_beat(beat)
                .into_iter()
                .map(|(e, kind)| (a.elements[e].name.clone(), a.elements[e].key.clone(), kind))
                .collect::<Vec<_>>()
        })
    };
    let tag = move |(i, (name, key, kind)): (usize, (String, String, Kind))| {
        let kind_class = match kind {
            Kind::Setup => "setup",
            Kind::Mention => "mention",
            Kind::Payoff => "payoff",
        };
        let focused = {
            let key = key.clone();
            move || st.focus.get().as_deref() == Some(key.as_str())
        };
        view! {
            {(i > 0).then_some(" · ")}
            <span class=format!("tag {kind_class}") class:focused=focused on:click=move |_| st.toggle_focus(key.clone())>
                <b>{name}</b>
                " "
                {kind.verb()}
            </span>
        }
    };
    view! {
        <div
            class=class
            id=format!("beat-{id}")
            on:dragover=move |ev| dnd::over(st, id, &ev)
            on:drop=move |ev| dnd::drop(st, &ev)
        >
            <div
                class="n"
                draggable="true"
                title="Glisser pour déplacer"
                on:dragstart=move |ev| dnd::start(st, id, &ev)
                on:dragend=move |_| dnd::end(st)
            >
                {number}
            </div>
            <div class="body">
                <div class="grow" data-value=move || text.get()>
                    <textarea
                        rows="1"
                        aria-label="Texte du beat"
                        placeholder="Écris le beat. @nom déclare un élément, !nom le paie."
                        prop:value=move || text.get()
                        on:input=move |ev| st.story.update(|s| s.set_text(id, event_target_value(&ev)))
                    ></textarea>
                </div>
                <div class="tags">
                    <span>{move || tags().into_iter().enumerate().map(tag).collect_view()}</span>
                    <span class="tools">
                        {tool("↑", "Monter", move || { st.story.update(|s| { s.step(id, Side::Before); }); })}
                        {tool("↓", "Descendre", move || { st.story.update(|s| { s.step(id, Side::After); }); })}
                        {tool("×", "Archiver", move || st.archive(id))}
                    </span>
                </div>
            </div>
        </div>
    }
}
