//! The footer: one line that counts the elements, opening the panel, and
//! lists the warnings, each going to its beat.

use leptos::prelude::*;

use crate::state::{Popup, State};

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
                    if let Some(id) = st.beats.with_untracked(|b| b.get(beat).copied()) {
                        st.reveal(id);
                    }
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
        <footer class="footer">
            <button class="btn" on:click=move |ev| st.toggle_popup(Popup::Panel, &ev)>
                {move || format!("{} élément{}", count(), if count() == 1 { "" } else { "s" })}
            </button>
            " · "
            {warnings}
        </footer>
    }
}
