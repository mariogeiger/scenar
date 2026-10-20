//! The frieze: every scene as a card in story order, with an arc from the
//! scene where an element is set up to each scene where it pays off. The
//! arcs of the focused element stand out; a red dot is a beat that warns.

use std::collections::HashMap;

use leptos::prelude::*;
use scenar::engine::Kind;
use scenar::model::Level;

use crate::state::State;

#[derive(Clone, PartialEq)]
struct Card {
    id: u64,
    label: String,
    /// Whether it opens an act.
    first: bool,
    beats: Vec<u64>,
}

#[component]
pub fn Frieze() -> impl IntoView {
    let st = expect_context::<State>();
    let cards = Memo::new(move |_| {
        st.live.with(|l| {
            let mut last_act = None;
            l.iter()
                .filter(|n| n.level == Level::Scene)
                .map(|n| {
                    let first = last_act != n.parent;
                    last_act = n.parent;
                    let beats = l.iter().filter(|b| b.parent == Some(n.id)).map(|b| b.id).collect();
                    Card { id: n.id, label: n.label.clone(), first, beats }
                })
                .collect::<Vec<_>>()
        })
    });
    let count = move || cards.with(|c| c.len().max(1));
    let arcs = move || {
        let scene_of: HashMap<usize, usize> = cards.with(|cards| {
            cards
                .iter()
                .enumerate()
                .flat_map(|(i, c)| c.beats.iter().filter_map(move |&b| st.beat_index(b).map(|k| (k, i))))
                .collect()
        });
        let focus = st.focus.get();
        let mut arcs: Vec<(usize, usize, bool)> = st.analysis.with(|a| {
            a.elements
                .iter()
                .flat_map(|e| {
                    let setup = e.occurrences.iter().find(|o| o.kind == Kind::Setup);
                    let focused = focus.as_deref() == Some(e.key.as_str());
                    setup
                        .into_iter()
                        .flat_map(|s| {
                            e.occurrences
                                .iter()
                                .filter(move |o| o.kind == Kind::Payoff && o.at > s.at)
                                .map(move |o| (s.at.0, o.at.0))
                        })
                        .filter_map(|(from, to)| Some((*scene_of.get(&from)?, *scene_of.get(&to)?, focused)))
                        .collect::<Vec<_>>()
                })
                .collect()
        });
        arcs.sort_by_key(|&(_, _, focused)| focused);
        let n = count() as f64;
        arcs.into_iter()
            .map(|(from, to, focused)| {
                let (x1, x2) = (from as f64 + 0.5, to as f64 + 0.5);
                let d = if from == to {
                    format!("M {} 100 C {} 45, {} 45, {} 100", x1 - 0.2, x1 - 0.2, x1 + 0.2, x1 + 0.2)
                } else {
                    let top = 100.0 - (35.0 + 90.0 * (x2 - x1).abs() / n).min(125.0);
                    format!("M {x1} 100 C {x1} {top}, {x2} {top}, {x2} 100")
                };
                view! { <path d=d class:focused=focused/> }
            })
            .collect_view()
    };
    let card = move |c: Card| {
        let id = c.id;
        let title = move || st.story.with(|s| s.label(id));
        let dots = c
            .beats
            .iter()
            .map(|&b| {
                let warns = move || {
                    st.beat_index(b).is_some_and(|k| st.analysis.with(|a| a.warnings.iter().any(|w| w.beat == k)))
                };
                view! { <i class:warns=warns></i> }
            })
            .collect_view();
        view! {
            <button
                class="card"
                class:first=c.first
                class:on=move || st.scene.get() == Some(id)
                class:hl=move || st.holds_focus(id)
                on:click=move |_| st.reveal(id)
            >
                <span class="card-title"><span class="number">{c.label}</span>" "{title}</span>
                <span class="dots">{dots}</span>
            </button>
        }
    };
    view! {
        <div class="frieze">
            <div class="frieze-inner" style=move || format!("--scenes: {}", count())>
                <svg class="arcs" viewBox=move || format!("0 0 {} 100", count()) preserveAspectRatio="none" aria-hidden="true">
                    {arcs}
                </svg>
                <div class="cards">{move || cards.get().into_iter().map(card).collect_view()}</div>
            </div>
        </div>
    }
}
