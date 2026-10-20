//! What opens over the page: the actions of a block, the bin, the panel and,
//! on a phone, the timeline. One at a time, placed beside what opened it,
//! closed by a click outside, Escape, or the choice made in it.

use leptos::prelude::*;
use scenar::model::Side;

use super::{panel::Panel, timeline::Timeline};
use crate::state::{Anchor, Popup, State};
use crate::storage;

#[component]
pub fn Popups() -> impl IntoView {
    let st = expect_context::<State>();
    move || {
        st.popup.get().map(|(popup, anchor)| {
            let (kind, body) = match popup {
                Popup::Menu(id) => ("menu", menu(st, id)),
                Popup::Bin => ("bin", bin(st).into_any()),
                Popup::Panel => ("wide", view! { <Panel/> }.into_any()),
                Popup::Timeline => ("wide", view! { <Timeline/> }.into_any()),
            };
            view! {
                <div class="backdrop" on:click=move |_| st.close_popup()></div>
                <div class=format!("popup {kind}") style=place(anchor) role="dialog">{body}</div>
            }
        })
    }
}

/// Places a popup below what opened it, or above when that stands low,
/// aligned on its side of the window; across the width on a phone.
fn place(a: Anchor) -> String {
    let (w, h) = storage::viewport();
    let across = if w < 600.0 {
        "left: 8px; right: 8px;".to_string()
    } else if a.left > w / 2.0 {
        format!("right: {}px;", w - a.right)
    } else {
        format!("left: {}px;", a.left)
    };
    let down = if a.bottom > h * 0.6 {
        format!("bottom: {}px; max-height: {}px;", h - a.top + 4.0, a.top - 12.0)
    } else {
        format!("top: {}px; max-height: {}px;", a.bottom + 4.0, h - a.bottom - 12.0)
    };
    across + &down
}

/// The actions on the block `id`, the same at every level.
fn menu(st: State, id: u64) -> AnyView {
    let Some(level) = st.story.with_untracked(|s| s.level(id)) else {
        return ().into_any();
    };
    let parent = st.story.with_untracked(|s| s.parent(id));
    let label = st.story.with_untracked(|s| s.label(id));
    let step = move |side| {
        st.edit(|s| s.step(id, side));
        st.reveal(id);
    };
    let add = move |parent: Option<u64>, after: Option<u64>| {
        if let Some(new) = st.edit(|s| s.add(parent, after)) {
            st.reveal(new);
        }
    };
    let mut items: Vec<(String, Box<dyn Fn()>)> = vec![
        ("↑ Monter".into(), Box::new(move || step(Side::Before))),
        ("↓ Descendre".into(), Box::new(move || step(Side::After))),
        (format!("+ Ajouter {} après", level.indefinite()), Box::new(move || add(parent, Some(id)))),
    ];
    if let Some(child) = level.child() {
        items.push((format!("+ Ajouter {} à la fin", child.indefinite()), Box::new(move || add(Some(id), None))));
    }
    items.push(("× Archiver".into(), Box::new(move || st.archive(id))));
    view! {
        <div class="heading">{level.name()}" · "{label}</div>
        {items
            .into_iter()
            .map(|(label, act)| {
                view! {
                    <button class="item" on:click=move |_| {
                        st.close_popup();
                        act();
                    }>{label}</button>
                }
            })
            .collect_view()}
    }
    .into_any()
}

/// The archived blocks, each restorable or deletable for good.
fn bin(st: State) -> impl IntoView {
    move || {
        let items =
            st.story.with(|s| s.archived().into_iter().map(|(id, level)| (id, level, s.label(id))).collect::<Vec<_>>());
        let rows = items
            .into_iter()
            .map(|(id, level, label)| {
                view! {
                    <div class="bin-item">
                        <span class="label"><span class="level">{level.name()}</span>" "{label}</span>
                        <button class="btn" on:click=move |_| st.edit(|s| s.set_archived(id, false))>"restaurer"</button>
                        <button class="btn" on:click=move |_| st.delete(id)>"supprimer"</button>
                    </div>
                }
            })
            .collect::<Vec<_>>();
        view! {
            <div class="heading">"Bin"</div>
            {if rows.is_empty() { Some(view! { <p class="empty">"La bin est vide."</p> }) } else { None }}
            {rows}
        }
    }
}
