//! The panel: every element with where it occurs, then every warning. A
//! column of its own on an ultra-wide screen, a popup elsewhere.

use leptos::prelude::*;

use crate::state::State;

/// Goes to the beat `beat` of the engine's numbering.
fn open(st: State, beat: usize) {
    if let Some(id) = st.beats.with_untracked(|b| b.get(beat).copied()) {
        st.reveal(id);
    }
}

#[component]
pub fn Panel() -> impl IntoView {
    view! {
        <section>
            <h3 class="heading">"Éléments"</h3>
            <Elements/>
        </section>
        <section>
            <h3 class="heading">"Warnings"</h3>
            <Warnings/>
        </section>
    }
}

#[component]
fn Elements() -> impl IntoView {
    let st = expect_context::<State>();
    move || {
        let rows = st.analysis.with(|a| {
            a.elements
                .iter()
                .map(|e| {
                    let mut flow: Vec<(usize, &'static str)> =
                        e.occurrences.iter().map(|o| (o.at.0, o.kind.symbol())).collect();
                    flow.dedup();
                    (e.name.clone(), e.key.clone(), e.aliases.join(", "), flow)
                })
                .collect::<Vec<_>>()
        });
        if rows.is_empty() {
            return view! { <p class="empty">"Aucun élément. Écris @nom dans un beat pour en déclarer un."</p> }
                .into_any();
        }
        rows.into_iter()
            .map(|(name, key, aliases, flow)| {
                let focused = {
                    let key = key.clone();
                    move || st.focus.get().as_deref() == Some(key.as_str())
                };
                let flow = flow
                    .into_iter()
                    .map(|(beat, symbol)| {
                        view! { <button class="at" on:click=move |_| open(st, beat)>{symbol}" "{st.beat_label(beat)}</button> }
                    })
                    .collect_view();
                view! {
                    <div class="element" class:focused=focused>
                        <button class="name" title="Surligner ses beats" on:click=move |_| st.toggle_focus(key.clone())>
                            {name}
                        </button>
                        " "
                        <span class="aliases">{aliases}</span>
                        <span class="flow">{flow}</span>
                    </div>
                }
            })
            .collect_view()
            .into_any()
    }
}

#[component]
fn Warnings() -> impl IntoView {
    let st = expect_context::<State>();
    move || {
        let rows = st.analysis.with(|a| {
            a.warnings
                .iter()
                .map(|w| (a.elements[w.element].name.clone(), a.elements[w.element].key.clone(), w.rule, w.beat))
                .collect::<Vec<_>>()
        });
        if rows.is_empty() {
            return view! { <p class="empty">"Aucun warning."</p> }.into_any();
        }
        rows.into_iter()
            .map(|(name, key, rule, beat)| {
                let scene = st
                    .beats
                    .with_untracked(|b| b.get(beat).copied())
                    .and_then(|id| st.story.with_untracked(|s| s.parent(id).map(|p| s.label(p))))
                    .unwrap_or_default();
                let go = move |_| {
                    st.focus.set(Some(key.clone()));
                    open(st, beat);
                };
                view! {
                    <button class="warning" on:click=go>
                        <span><b>{name}</b>" "{rule.message()}</span>
                        <span class="where">{st.beat_label(beat)}" · "{scene}</span>
                    </button>
                }
            })
            .collect_view()
            .into_any()
    }
}
