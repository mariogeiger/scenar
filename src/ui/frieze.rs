//! The frieze: the story as one band partitioned three times, acts into
//! scenes into beats, each beat as wide as its text and each block the sum
//! of what it holds. Under the band hang the dependencies, from the beat
//! that sets an element up to each beat that pays it off. A red beat warns;
//! grey is the selection; a lit dependency is red.

use std::collections::HashMap;

use leptos::prelude::*;
use scenar::engine::Dependency;
use scenar::model::Level;

use crate::state::State;

/// The width, in characters of text, below which no beat goes, so that a
/// short or empty one stays in sight and in reach.
const LEAST_WEIGHT: f64 = 60.0;

/// A block of the band, with what it holds.
#[derive(Clone, PartialEq)]
struct Block {
    id: u64,
    level: Level,
    label: String,
    inner: Vec<Block>,
}

#[component]
pub fn Frieze() -> impl IntoView {
    let st = expect_context::<State>();
    let band = Memo::new(move |_| {
        st.live.with(|l| {
            fn below(l: &[scenar::model::Live], parent: Option<u64>) -> Vec<Block> {
                l.iter()
                    .filter(|n| n.parent == parent)
                    .map(|n| Block { id: n.id, level: n.level, label: n.label.clone(), inner: below(l, Some(n.id)) })
                    .collect()
            }
            below(l, None)
        })
    });
    // Every block's width, in characters: a beat's text, at least the least
    // weight, and a block's what it holds, or the least weight when empty.
    let weights = Memo::new(move |_| {
        fn weigh(b: &Block, text: &dyn Fn(u64) -> usize, out: &mut HashMap<u64, f64>) -> f64 {
            let w = if b.level == Level::Beat {
                (text(b.id) as f64).max(LEAST_WEIGHT)
            } else {
                let sum: f64 = b.inner.iter().map(|c| weigh(c, text, out)).sum();
                if b.inner.is_empty() { LEAST_WEIGHT } else { sum }
            };
            out.insert(b.id, w);
            w
        }
        st.story.with(|s| {
            let text = |id: u64| s.find(id).map_or(0, |n| n.text.chars().count());
            let mut out = HashMap::new();
            band.with(|band| band.iter().for_each(|b| _ = weigh(b, &text, &mut out)));
            out
        })
    });
    let weight = move |id: u64| weights.with(|w| w.get(&id).copied().unwrap_or(LEAST_WEIGHT));
    let total = move || band.with(|b| b.iter().map(|a| weight(a.id)).sum::<f64>()).max(LEAST_WEIGHT);
    view! {
        <div class="frieze">
            <div class="frieze-inner" style=move || format!("--weight: {}", total())>
                <div class="band">{move || band.get().into_iter().map(|b| block(st, b, weight)).collect_view()}</div>
                <Arcs weight=weight total=total/>
            </div>
        </div>
    }
}

/// One block of the band: a beat is a cell; an act or a scene a title over
/// the row of what it holds.
fn block(st: State, b: Block, weight: impl Fn(u64) -> f64 + Copy + Send + Sync + 'static) -> AnyView {
    let id = b.id;
    let grow = move || format!("flex-grow: {}", weight(id));
    let title = move || st.story.with(|s| s.label(id));
    if b.level == Level::Beat {
        let warns =
            move || st.beat_index(id).is_some_and(|k| st.analysis.with(|a| a.warnings.iter().any(|w| w.beat == k)));
        return view! {
            <button
                class="cell beat-cell"
                style=grow
                title=move || format!("{} {}", b.label, title())
                class:warns=warns
                class:sel=move || st.is_selected(id)
                class:hl=move || st.holds_lit(id)
                class:pointed=move || st.pointed.get() == Some(id)
                on:click=move |_| st.reveal(id)
                on:mouseenter=move |_| st.pointed.set(Some(id))
                on:mouseleave=move |_| st.pointed.set(None)
            ></button>
        }
        .into_any();
    }
    let level = if b.level == Level::Act { "act-cell" } else { "scene-cell" };
    let number = if b.level == Level::Act { format!("{} — ", b.label) } else { format!("{} ", b.label) };
    view! {
        <div class=format!("cell {level}") style=grow class:sel=move || st.is_selected(id)>
            <button class="cell-title" title=move || format!("{number}{}", title()) on:click=move |_| st.reveal(id)>
                <span class="number">{number.clone()}</span>
                {title}
            </button>
            <div class="row-of">{b.inner.into_iter().map(|c| block(st, c, weight)).collect_view()}</div>
        </div>
    }
    .into_any()
}

/// The dependencies, hanging from the band. Their paths stay put while the
/// pointer moves over them; the lit ones are drawn again on top.
#[component]
fn Arcs(
    weight: impl Fn(u64) -> f64 + Copy + Send + Sync + 'static,
    total: impl Fn() -> f64 + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let st = expect_context::<State>();
    // The middle of each beat along the band, by the engine's index.
    let middles = Memo::new(move |_| {
        let mut at = 0.0;
        st.beats.with(|b| {
            b.iter()
                .map(|&id| {
                    let w = weight(id);
                    at += w;
                    (at - w / 2.0, w)
                })
                .collect::<Vec<_>>()
        })
    });
    let dependencies = Memo::new(move |_| st.analysis.with(|a| a.dependencies()));
    let path = move |d: &Dependency| -> Option<String> {
        middles.with(|m| {
            let (x1, w) = *m.get(d.setup.0)?;
            let (x2, _) = *m.get(d.payoff.0)?;
            Some(if d.setup.0 == d.payoff.0 {
                let r = w * 0.3;
                format!("M {} 0 C {} 40, {} 40, {} 0", x1 - r, x1 - r, x1 + r, x1 + r)
            } else {
                let depth = (30.0 + 170.0 * (x2 - x1).abs() / total()).min(133.0);
                format!("M {x1} 0 C {x1} {depth}, {x2} {depth}, {x2} 0")
            })
        })
    };
    let name = move |d: &Dependency| {
        let element = st.analysis.with(|a| a.elements.get(d.element).map(|e| (e.name.clone(), e.key.clone())));
        element.unwrap_or_default()
    };
    let base = move || {
        dependencies
            .get()
            .into_iter()
            .filter_map(|d| {
                let shape = path(&d)?;
                let (name, key) = name(&d);
                let tip = format!("{name} : {} → {}", st.beat_label(d.setup.0), st.beat_label(d.payoff.0));
                Some(view! {
                    <g>
                        <path d=shape.clone()/>
                        <path class="hit" d=shape
                            on:mouseenter=move |_| if st.hovered.get_untracked() != Some(d) { st.hovered.set(Some(d)) }
                            on:mouseleave=move |_| st.hovered.set(None)
                            on:click=move |_| st.toggle_focus(key.clone())>
                            <title>{tip}</title>
                        </path>
                    </g>
                })
            })
            .collect_view()
    };
    let lit = move || {
        dependencies
            .with(|ds| ds.iter().filter(|d| st.dependency_lit(d)).filter_map(path).collect::<Vec<_>>())
            .into_iter()
            .map(|shape| view! { <path class="lit" d=shape/> })
            .collect_view()
    };
    view! {
        <svg class="arcs" viewBox=move || format!("0 0 {} 100", total()) preserveAspectRatio="none" aria-hidden="true">
            {base}
            {lit}
        </svg>
    }
}
