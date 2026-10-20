//! The footer: one line that counts the elements and lists the warnings,
//! and above it, when unfolded, every element with where it occurs.

use leptos::prelude::*;

use crate::state::State;

/// Opens the beat `beat` of the engine's numbering.
fn open(st: State, beat: usize) {
    if let Some(id) = st.beats.with_untracked(|b| b.get(beat).copied()) {
        st.show_beat(id);
    }
}

#[component]
pub fn Footer() -> impl IntoView {
    let st = expect_context::<State>();
    let count = move || st.analysis.with(|a| a.elements.len());
    let warnings = move || {
        let rows = st.analysis.with(|a| {
            a.warnings
                .iter()
                .map(|w| (a.elements[w.element].name.clone(), a.elements[w.element].key.clone(), w.rule, w.beat))
                .collect::<Vec<_>>()
        });
        let n = rows.len();
        let list = rows
            .into_iter()
            .enumerate()
            .map(|(i, (name, key, rule, beat))| {
                let go = move |_| {
                    st.focus.set(Some(key.clone()));
                    open(st, beat);
                };
                view! {
                    {if i == 0 { " — " } else { " · " }}
                    <button class="warning" title=st.beat_label(beat) on:click=go>
                        <b>{name}</b>" "{rule.message()}
                    </button>
                }
            })
            .collect_view();
        view! {
            <span class:warns={n > 0}>{match n {
                0 => "aucun warning".to_string(),
                1 => "1 warning".to_string(),
                n => format!("{n} warnings"),
            }}</span>
            {list}
        }
    };
    view! {
        <footer>
            <Show when=move || st.elements_open.get()>
                <Elements/>
            </Show>
            <div class="line">
                <button class="count" on:click=move |_| st.elements_open.update(|o| *o = !*o)>
                    {move || format!("{} {} élément{}", if st.elements_open.get() { "▾" } else { "▴" }, count(), if count() == 1 { "" } else { "s" })}
                </button>
                " · "
                {warnings}
            </div>
        </footer>
    }
}

#[component]
fn Elements() -> impl IntoView {
    let st = expect_context::<State>();
    let rows = move || {
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
                        view! {
                            <button class="at" on:click=move |_| open(st, beat)>
                                {symbol}" "{st.beat_label(beat)}
                            </button>
                        }
                    })
                    .collect_view();
                view! {
                    <div class="element" class:focused=focused>
                        <button class="name" on:click=move |_| st.toggle_focus(key.clone())>{name}</button>
                        " "
                        <span class="aliases">{aliases}</span>
                        <span class="flow">{flow}</span>
                    </div>
                }
            })
            .collect_view()
            .into_any()
    };
    view! { <div class="elements">{rows}</div> }
}
