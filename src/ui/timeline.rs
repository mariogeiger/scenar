//! The timeline: the acts as headings, their scenes as places to go, the
//! bin of archived blocks, then where the story is kept: the button that
//! shares the draft, or who is on the link; and the links this browser
//! opened. It never changes shape as one reads.

use leptos::prelude::*;
use scenar::model::{Level, Live};

use super::more;
use crate::dnd;
use crate::link::{self, Sync};
use crate::state::{Popup, State};
use crate::storage;

#[component]
pub fn Timeline() -> impl IntoView {
    let st = expect_context::<State>();
    let rows = move || {
        let shown: Vec<Live> = st.live.with(|l| l.iter().filter(|n| n.level != Level::Beat).cloned().collect());
        shown.into_iter().map(|n| row(st, n)).collect_view()
    };
    let add_act = move |_| {
        if let Some(act) = st.edit(|s| s.add(None, None)) {
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
        {kept(st)}
        {links(st)}
    }
}

/// Where the story is kept: on the draft, the button that shares it; on a
/// link, who is there and how it stands with the server.
fn kept(st: State) -> impl IntoView {
    if st.link.is_none() {
        return view! {
            <div class="kept">
                <button class="btn" title="Créer un lien qui partage ce scénario et le sauve" on:click=move |_| st.publish()>
                    "Sauver"
                </button>
                <span class="hint">"Brouillon : il ne vit que dans ce navigateur."</span>
            </div>
        }
        .into_any();
    }
    let state = move || {
        let here = st.peers.with(|p| p.len()) + 1;
        match st.sync.get() {
            Sync::Online => format!("● {here} sur ce lien · à jour"),
            Sync::Connecting => "connexion…".into(),
            Sync::Offline => "hors ligne : tes changements partiront à la reconnexion".into(),
            Sync::TooLarge => "⚠ le scénario a atteint 2 Mo ; le serveur n'en prend plus".into(),
            Sync::Missing => "⚠ ce lien n'existe pas".into(),
        }
    };
    let warns = move || matches!(st.sync.get(), Sync::TooLarge | Sync::Missing | Sync::Offline);
    let who = move || {
        st.peers
            .get()
            .into_iter()
            .map(|p| view! { <span class="who" style=format!("--c: {}", p.color)>{p.name}</span> })
            .collect_view()
    };
    view! {
        <div class="kept">
            <span class="sync" class:warns=warns>{state}</span>
            <span class="peers">{who}</span>
            <input
                class="title name"
                aria-label="Ton nom, pour les autres"
                placeholder="ton nom"
                prop:value=storage::name()
                on:change=move |ev| link::set_name(event_target_value(&ev))
            />
        </div>
    }
    .into_any()
}

/// The links this browser opened.
fn links(st: State) -> impl IntoView {
    let links = storage::links();
    (!links.is_empty()).then(|| {
        let rows = links
            .into_iter()
            .map(|l| {
                let here = st.link == Some(l.id.as_str());
                let title = if l.title.trim().is_empty() { "Sans titre".to_string() } else { l.title };
                view! { <a class="row link" class:sel=here href=format!("/{}", l.id)><span class="label">{title}</span></a> }
            })
            .collect_view();
        view! {
            <div class="row heading">"Mes liens"</div>
            <div class="tree">{rows}</div>
        }
    })
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
