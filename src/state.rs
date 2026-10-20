//! The state the whole page shares, and the actions that change the story
//! with the confirmations they need.

use leptos::prelude::*;
use scenar::engine::{self, Analysis, Kind, Rule};
use scenar::model::{Level, Live, Side, Story};

use crate::storage;

/// What the margin says about one element of a beat.
#[derive(Clone, PartialEq)]
pub struct Note {
    pub name: String,
    pub key: String,
    pub kind: Kind,
    /// Where the other end of the dependency is, or the warning that it
    /// breaks; empty for a mention that breaks nothing.
    pub relation: String,
    pub warns: bool,
}

#[derive(Clone, Copy)]
pub struct State {
    pub story: RwSignal<Story>,
    /// The scene chosen to be shown, which may have been archived since.
    pub chosen: RwSignal<Option<u64>>,
    /// The key of the element whose beats are highlighted.
    pub focus: RwSignal<Option<String>>,
    /// Whether the list of elements is unfolded above the footer.
    pub elements_open: RwSignal<bool>,
    pub dark: RwSignal<bool>,
    pub bin_open: RwSignal<bool>,
    /// The block being dragged, and where it would land.
    pub drag: RwSignal<Option<u64>>,
    pub drop: RwSignal<Option<(u64, Side)>>,
    pub live: Memo<Vec<Live>>,
    /// The living beats, in the order the engine numbers them.
    pub beats: Memo<Vec<u64>>,
    pub analysis: Memo<Analysis>,
    /// The scene on screen: the chosen one if it lives, else the first.
    pub scene: Memo<Option<u64>>,
}

impl State {
    pub fn new() -> State {
        let story = RwSignal::new(storage::load().unwrap_or_else(scenar::example::kerlaz));
        let chosen = RwSignal::new(None);
        let live = Memo::new(move |_| story.with(Story::live));
        let beats: Memo<Vec<u64>> = Memo::new(move |_| {
            live.with(|l| l.iter().filter(|n| n.level == Level::Beat).map(|n| n.id).collect())
        });
        let analysis = Memo::new(move |_| {
            story.with(|s| {
                beats.with(|b| engine::analyze(b.iter().map(|&id| s.find(id).map_or("", |n| n.text.as_str()))))
            })
        });
        let scene = Memo::new(move |_| {
            live.with(|l| {
                let scenes = || l.iter().filter(|n| n.level == Level::Scene);
                chosen.get().filter(|&c| scenes().any(|n| n.id == c)).or_else(|| scenes().next().map(|n| n.id))
            })
        });
        let dark = RwSignal::new(storage::dark());
        Effect::new(move |_| story.with(storage::save));
        Effect::new(move |_| storage::set_dark(dark.get()));
        State {
            story,
            chosen,
            focus: RwSignal::new(None),
            elements_open: RwSignal::new(false),
            dark,
            bin_open: RwSignal::new(false),
            drag: RwSignal::new(None),
            drop: RwSignal::new(None),
            live,
            beats,
            analysis,
            scene,
        }
    }

    /// The living block `id`.
    pub fn live_node(&self, id: u64) -> Option<Live> {
        self.live.with(|l| l.iter().find(|n| n.id == id).cloned())
    }

    /// The engine's index of the beat `id`.
    pub fn beat_index(&self, id: u64) -> Option<usize> {
        self.beats.with(|b| b.iter().position(|&x| x == id))
    }

    /// Whether the focused element occurs in the beat `id`.
    pub fn highlighted(&self, id: u64) -> bool {
        let Some(key) = self.focus.get() else { return false };
        let Some(beat) = self.beat_index(id) else { return false };
        self.analysis.with(|a| {
            a.element(&key).is_some_and(|e| a.elements[e].occurrences.iter().any(|o| o.at.0 == beat))
        })
    }

    /// The margin notes of the beat `id`: one per element and kind, in text
    /// order, each pointing at the other end of its dependency.
    pub fn notes(&self, id: u64) -> Vec<Note> {
        let Some(beat) = self.beat_index(id) else { return Vec::new() };
        let label = |b: usize| self.beat_label(b);
        self.analysis.with(|a| {
            a.in_beat(beat)
                .into_iter()
                .map(|(e, kind)| {
                    let el = &a.elements[e];
                    let at = el.occurrences.iter().find(|o| o.at.0 == beat && o.kind == kind).map(|o| o.at);
                    let first = |k: Kind| el.occurrences.iter().find(|o| o.kind == k).map(|o| o.at);
                    let warning = a.warnings.iter().find(|w| {
                        w.element == e
                            && w.beat == beat
                            && match w.rule {
                                Rule::NeverPaid | Rule::DeclaredTwice => kind == Kind::Setup,
                                Rule::PaidUndeclared => kind == Kind::Payoff,
                                Rule::UsedBeforeDeclared => kind != Kind::Setup,
                            }
                    });
                    let relation = match (warning.map(|w| w.rule), kind) {
                        (Some(Rule::DeclaredTwice), _) => {
                            format!("⚠ déjà déclaré en {}", first(Kind::Setup).map_or_else(String::new, |p| label(p.0)))
                        }
                        (Some(Rule::UsedBeforeDeclared), _) => format!(
                            "⚠ utilisé avant d'être déclaré, en {}",
                            first(Kind::Setup).map_or_else(String::new, |p| label(p.0))
                        ),
                        (Some(rule), _) => format!("⚠ {}", rule.message()),
                        (None, Kind::Setup) => el
                            .occurrences
                            .iter()
                            .find(|o| o.kind == Kind::Payoff && Some(o.at) > at)
                            .map_or_else(String::new, |o| format!("→ payé en {}", label(o.at.0))),
                        (None, Kind::Payoff) => {
                            first(Kind::Setup).map_or_else(String::new, |p| format!("← déclaré en {}", label(p.0)))
                        }
                        (None, Kind::Mention) => String::new(),
                    };
                    Note { name: el.name.clone(), key: el.key.clone(), kind, relation, warns: warning.is_some() }
                })
                .collect()
        })
    }

    /// Whether some warning points into the scene `id`.
    pub fn scene_warns(&self, id: u64) -> bool {
        let beats: Vec<usize> = self.live.with(|l| {
            l.iter().filter(|n| n.parent == Some(id)).filter_map(|n| self.beat_index(n.id)).collect()
        });
        self.analysis.with(|a| a.warnings.iter().any(|w| beats.contains(&w.beat)))
    }

    pub fn toggle_focus(&self, key: String) {
        self.focus.update(|f| *f = if f.as_deref() == Some(&key) { None } else { Some(key) });
    }

    /// Shows the scene of the beat `id`, and scrolls to it.
    pub fn show_beat(&self, id: u64) {
        let scene = self.story.with_untracked(|s| s.parent(id));
        self.chosen.set(scene);
        set_timeout(move || storage::reveal(id), std::time::Duration::from_millis(30));
    }

    /// Archives a block, after listing the elements that also occur outside
    /// it when there are any: archiving changes what those occurrences mean.
    pub fn archive(&self, id: u64) {
        let inside: Vec<usize> = self
            .story
            .with_untracked(|s| s.subtree(id))
            .into_iter()
            .filter_map(|b| self.beats.with_untracked(|v| v.iter().position(|&x| x == b)))
            .collect();
        let dependents: Vec<String> = self.analysis.with_untracked(|a| {
            a.elements
                .iter()
                .filter(|e| e.occurrences.iter().any(|o| inside.contains(&o.at.0)))
                .filter_map(|e| {
                    let outside: Vec<String> = e
                        .occurrences
                        .iter()
                        .filter(|o| !inside.contains(&o.at.0))
                        .map(|o| format!("{} {}", o.kind.symbol(), self.beat_label(o.at.0)))
                        .collect();
                    (!outside.is_empty()).then(|| format!("• {} : {}", e.name, outside.join(", ")))
                })
                .collect()
        });
        let label = self.story.with_untracked(|s| s.label(id));
        if !dependents.is_empty()
            && !storage::confirm(&format!(
                "Archiver « {label} » ?\n\nCes éléments en dépendent ailleurs :\n{}\n\nLe bloc ira dans la bin, d'où il se restaure.",
                dependents.join("\n")
            ))
        {
            return;
        }
        self.story.update(|s| s.set_archived(id, true));
    }

    /// Deletes an archived block for good, once confirmed.
    pub fn delete(&self, id: u64) {
        let label = self.story.with_untracked(|s| s.label(id));
        if storage::confirm(&format!("Supprimer définitivement « {label} » et tout ce qu'il contient ?")) {
            self.story.update(|s| {
                s.remove(id);
            });
        }
    }

    /// The place of the engine's beat `beat`, such as `II.3.1`.
    pub fn beat_label(&self, beat: usize) -> String {
        let id = self.beats.with_untracked(|b| b.get(beat).copied());
        id.and_then(|id| self.live.with_untracked(|l| l.iter().find(|n| n.id == id).map(|n| n.label.clone())))
            .unwrap_or_default()
    }

    pub fn reset(&self) {
        if storage::confirm("Vider le scénario ? Exporte-le d'abord si tu veux le garder.") {
            self.story.set(Story::blank());
            self.focus.set(None);
        }
    }

    pub fn replace(&self, json: &str) {
        match Story::from_json(json) {
            Ok(story) => {
                if storage::confirm(&format!("Remplacer le scénario actuel par « {} » ?", story.title)) {
                    self.story.set(story);
                    self.focus.set(None);
                }
            }
            Err(e) => storage::alert(&format!("Ce fichier n'est pas un scénario : {e}")),
        }
    }
}

/// Drag and drop, shared by the rows of the timeline and the beats.
pub mod dnd {
    use super::*;
    use wasm_bindgen::JsCast;
    use web_sys::DragEvent;

    pub fn start(st: State, id: u64, ev: &DragEvent) {
        st.drag.set(Some(id));
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_data("text/plain", &id.to_string());
            dt.set_effect_allowed("move");
        }
    }

    /// Shows where the dragged block would land on `target`: before or after
    /// it by the half the pointer is in.
    pub fn over(st: State, target: u64, ev: &DragEvent) {
        let Some(id) = st.drag.get_untracked() else { return };
        if !st.story.with_untracked(|s| s.can_place(id, target)) {
            return;
        }
        ev.prevent_default();
        ev.stop_propagation();
        let side = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|el| el.get_bounding_client_rect())
            .map_or(Side::After, |r| {
                if (ev.client_y() as f64) < r.top() + r.height() / 2.0 { Side::Before } else { Side::After }
            });
        if st.drop.get_untracked() != Some((target, side)) {
            st.drop.set(Some((target, side)));
        }
    }

    pub fn drop(st: State, ev: &DragEvent) {
        ev.prevent_default();
        ev.stop_propagation();
        if let (Some(id), Some((target, side))) = (st.drag.get_untracked(), st.drop.get_untracked()) {
            st.story.update(|s| {
                s.place(id, target, side);
            });
        }
        end(st);
    }

    pub fn end(st: State) {
        st.drag.set(None);
        st.drop.set(None);
    }

    /// The class that draws the landing line on `target`.
    pub fn class(st: State, target: u64) -> &'static str {
        match (st.drop.get(), st.drag.get()) {
            (Some((t, side)), Some(id)) if t == target => {
                if st.story.with_untracked(|s| s.level(id) == s.level(target)) {
                    if side == Side::Before { " drop-before" } else { " drop-after" }
                } else {
                    " drop-into"
                }
            }
            _ => "",
        }
    }
}
