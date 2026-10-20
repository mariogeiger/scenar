//! The timeline: the acts as headings, their scenes as places to go, and
//! the bin of archived blocks. It never changes shape as one reads.

use leptos::prelude::*;
use scenar::model::{Level, Live};

use super::more;
use crate::dnd;
use crate::state::{Popup, State};

#[component]
pub fn Timeline() -> impl IntoView {
    let st = expect_context::<State>();
    let rows = move || {
        let shown: Vec<Live> = st.live.with(|l| l.iter().filter(|n| n.level != Level::Beat).cloned().collect());
        shown.into_iter().map(|n| row(st, n)).collect_view()
    };
    let add_act = move |_| {
        if let Some(act) = st.story.try_update(|s| s.add(None, None)).flatten() {
            st.reveal(act);
        }
    };
    let archived = move || st.story.with(|s| s.archived().len());
    view! {
        <div class="tree">{rows}</div>
        <div class="tree-actions">
            <button class="btn" on:click=add_act>"+ acte"</button>
            <button class="btn" on:click=move |ev| st.toggle_popup(Popup::Bin, &ev)>
                {move || format!("Bin · {}", archived())}
            </button>
        </div>
    }
}

fn row(st: State, n: Live) -> impl IntoView {
    let (id, level) = (n.id, n.level);
    let act = level == Level::Act;
    let class = move || {
        let mut class = String::from(if act { "row heading" } else { "row link" });
        if st.is_selected(id) {
            class.push_str(" sel");
        }
        if !act && st.holds_lit(id) {
            class.push_str(" hl");
        }
        if !act && st.scene_warns(id) {
            class.push_str(" warns");
        }
        class.push_str(dnd::class(st, id));
        class
    };
    let label = move || st.story.with(|s| s.label(id));
    let number = if act { format!("{} · ", n.label) } else { String::new() };
    let open = move |_| {
        if !act {
            st.close_popup();
            st.reveal(id);
        }
    };
    view! {
        <div
            class=class
            draggable="true"
            on:click=open
            on:dragstart=move |ev| dnd::start(st, id, &ev)
            on:dragover=move |ev| dnd::over(st, id, &ev)
            on:drop=move |ev| dnd::drop(st, &ev)
            on:dragend=move |_| dnd::end(st)
        >
            <span class="label">{number}{label}</span>
            {more(st, id)}
        </div>
    }
}
