//! The timeline: acts and scenes as a tree, the beats of the scene on screen
//! below it, and the bin of archived blocks.

use leptos::prelude::*;
use scenar::model::{Level, Live, Side};

use super::tool;
use crate::state::{State, dnd};

#[component]
pub fn Timeline() -> impl IntoView {
    let st = expect_context::<State>();
    let rows = move || {
        let scene = st.scene.get();
        let shown: Vec<Live> =
            st.live.with(|l| l.iter().filter(|n| n.level != Level::Beat || n.parent == scene).cloned().collect());
        shown.into_iter().map(|n| row(st, n)).collect_view()
    };
    let add_act = move |_| {
        if let Some(act) = st.story.try_update(|s| s.add(None, None)).flatten() {
            st.chosen.set(st.story.with_untracked(|s| s.find(act).and_then(|a| a.children.first()).map(|n| n.id)));
        }
    };
    view! {
        <div class="tree">
            {rows}
            <button class="add" on:click=add_act>"+ acte"</button>
        </div>
    }
}

fn row(st: State, n: Live) -> impl IntoView {
    let (id, level) = (n.id, n.level);
    let kind = match level {
        Level::Act => "level-act",
        Level::Scene => "level-scene",
        Level::Beat => "level-beat",
    };
    let class = move || {
        let on = level == Level::Scene && st.scene.get() == Some(id);
        let hl = level == Level::Beat && st.highlighted(id);
        let warns = level == Level::Scene && st.scene_warns(id);
        format!(
            "row {kind}{}{}{}{}",
            if on { " on" } else { "" },
            if hl { " hl" } else { "" },
            if warns { " warns" } else { "" },
            dnd::class(st, id)
        )
    };
    let prefix = (level == Level::Act).then(|| format!("{} · ", n.label));
    let label = move || st.story.with(|s| s.label(id));
    let open = move |_| match level {
        Level::Act => {}
        Level::Scene => st.chosen.set(Some(id)),
        Level::Beat => st.show_beat(id),
    };
    let add = move || {
        let parent = st.story.with_untracked(|s| s.parent(id));
        let Some(new) = st.story.try_update(|s| s.add(parent, Some(id))).flatten() else { return };
        match level {
            Level::Act => st.chosen.set(st.story.with_untracked(|s| s.find(new).and_then(|a| a.children.first()).map(|n| n.id))),
            Level::Scene => st.chosen.set(Some(new)),
            Level::Beat => st.show_beat(new),
        }
    };
    let add_title = match level {
        Level::Act => "Ajouter un acte après",
        Level::Scene => "Ajouter une scène après",
        Level::Beat => "Ajouter un beat après",
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
            <span class="label">{prefix}{label}</span>
            <span class="tools">
                {tool("↑", "Monter", move || { st.story.update(|s| { s.step(id, Side::Before); }); })}
                {tool("↓", "Descendre", move || { st.story.update(|s| { s.step(id, Side::After); }); })}
                {tool("+", add_title, add)}
                {tool("×", "Archiver", move || st.archive(id))}
            </span>
        </div>
    }
}

#[component]
pub fn Bin() -> impl IntoView {
    let st = expect_context::<State>();
    let items = move || {
        st.story.with(|s| s.archived().into_iter().map(|(id, level)| (id, level, s.label(id))).collect::<Vec<_>>())
    };
    view! {
        <div class="bin">
            <button class="bin-head" on:click=move |_| st.bin_open.update(|o| *o = !*o)>
                {move || format!("{} Bin · {}", if st.bin_open.get() { "▾" } else { "▸" }, items().len())}
            </button>
            <Show when=move || st.bin_open.get()>
                {move || {
                    items()
                        .into_iter()
                        .map(|(id, level, label)| {
                            view! {
                                <div class="bin-item">
                                    <span class="label">
                                        <span class="level">{level.name()}</span>
                                        " "
                                        {label}
                                    </span>
                                    {tool("↺", "Restaurer", move || st.story.update(|s| s.set_archived(id, false)))}
                                    {tool("×", "Supprimer définitivement", move || st.delete(id))}
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </Show>
        </div>
    }
}
