//! The state the whole page shares, and the actions that change the story
//! with the confirmations they need.

use leptos::prelude::*;
use scenar::engine::{self, Analysis, Dependency, Kind, Position, Rule};
use scenar::model::{Level, Live, Side, Story};

use crate::storage;

/// Something that opens on a click, over the page, and closes on a click
/// outside it or on Escape: nothing ever unfolds in place.
#[derive(Clone, Copy, PartialEq)]
pub enum Popup {
    /// The actions on a block.
    Menu(u64),
    Bin,
    /// The elements and the warnings, where no panel shows them.
    Panel,
    /// The timeline, where no column shows it.
    Timeline,
}

/// The box, in viewport pixels, of what opened a popup.
#[derive(Clone, Copy, PartialEq)]
pub struct Anchor {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Anchor {
    /// The box of the element an event is handled on.
    pub fn of(ev: &web_sys::Event) -> Anchor {
        use wasm_bindgen::JsCast;
        let rect = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|e| e.get_bounding_client_rect());
        rect.map_or(
            Anchor {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            |r| Anchor {
                left: r.left(),
                top: r.top(),
                right: r.right(),
                bottom: r.bottom(),
            },
        )
    }
}

/// What the margin says about one element of a beat.
#[derive(Clone, PartialEq)]
pub struct Note {
    /// Index into the analysis' elements.
    pub element: usize,
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
    /// The block last clicked, or the scene being read, which may have been
    /// archived since.
    pub selected: RwSignal<Option<u64>>,
    /// The scrolling box of the script.
    pub script: NodeRef<leptos::html::Div>,
    /// Until when, in milliseconds since the epoch, scrolling is the page's
    /// own doing, so that what was selected stays selected.
    pub scrolled_by_page_until: RwSignal<f64>,
    /// The key of the element whose occurrences stay lit.
    pub focus: RwSignal<Option<String>>,
    /// The dependency under the pointer, lit while it stays there.
    pub hovered: RwSignal<Option<Dependency>>,
    /// The beat under the pointer, in the script or the frieze.
    pub pointed: RwSignal<Option<u64>>,
    pub popup: RwSignal<Option<(Popup, Anchor)>>,
    pub dark: RwSignal<bool>,
    /// The block being dragged, and where it would land.
    pub drag: RwSignal<Option<u64>>,
    pub drop: RwSignal<Option<(u64, Side)>>,
    pub live: Memo<Vec<Live>>,
    /// The living beats, in the order the engine numbers them.
    pub beats: Memo<Vec<u64>>,
    pub analysis: Memo<Analysis>,
    /// The selection: the selected block if it lives, else the first
    /// scene, then the blocks that hold it, up to its act.
    pub path: Memo<Vec<u64>>,
    /// The occurrences lit, by their position and end: every one of the
    /// focused element, and both ends of the hovered dependency.
    pub lit: Memo<Vec<(Position, usize)>>,
}

impl State {
    pub fn new() -> State {
        let story = RwSignal::new(storage::load().unwrap_or_else(scenar::example::kerlaz));
        let selected = RwSignal::new(None);
        let focus = RwSignal::new(None::<String>);
        let hovered = RwSignal::new(None::<Dependency>);
        let live = Memo::new(move |_| story.with(Story::live));
        let beats: Memo<Vec<u64>> = Memo::new(move |_| {
            live.with(|l| {
                l.iter()
                    .filter(|n| n.level == Level::Beat)
                    .map(|n| n.id)
                    .collect()
            })
        });
        let analysis = Memo::new(move |_| {
            story.with(|s| {
                beats.with(|b| {
                    engine::analyze(
                        b.iter()
                            .map(|&id| s.find(id).map_or("", |n| n.text.as_str())),
                    )
                })
            })
        });
        let path = Memo::new(move |_| {
            live.with(|l| {
                let find = |id: u64| l.iter().find(|n| n.id == id);
                let first = || l.iter().find(|n| n.level == Level::Scene);
                let mut node = selected.get().and_then(find).or_else(first);
                let mut path = Vec::new();
                while let Some(n) = node {
                    path.push(n.id);
                    node = n.parent.and_then(find);
                }
                path
            })
        });
        let lit = Memo::new(move |_| {
            let hovered = hovered.get();
            focus.with(|focus| {
                analysis.with(|a| {
                    let mut lit: Vec<(Position, usize)> = a
                        .elements
                        .iter()
                        .enumerate()
                        .flat_map(|(e, el)| {
                            let focused = focus.as_deref() == Some(el.key.as_str());
                            el.occurrences.iter().filter(move |o| {
                                focused
                                    || hovered.is_some_and(|d| {
                                        d.element == e && (o.at == d.setup || o.at == d.payoff)
                                    })
                            })
                        })
                        .map(|o| (o.at, o.end))
                        .collect();
                    lit.sort();
                    lit.dedup();
                    lit
                })
            })
        });
        let dark = RwSignal::new(storage::dark());
        Effect::new(move |_| story.with(storage::save));
        Effect::new(move |_| storage::set_dark(dark.get()));
        State {
            story,
            selected,
            script: NodeRef::new(),
            scrolled_by_page_until: RwSignal::new(0.0),
            focus,
            hovered,
            pointed: RwSignal::new(None),
            popup: RwSignal::new(None),
            dark,
            drag: RwSignal::new(None),
            drop: RwSignal::new(None),
            live,
            beats,
            analysis,
            path,
            lit,
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

    /// Whether the block `id` is selected or holds the selected block.
    pub fn is_selected(&self, id: u64) -> bool {
        self.path.with(|p| p.contains(&id))
    }

    /// Selects the block `id`, where the reader clicked.
    pub fn select(&self, id: u64) {
        if self.selected.get_untracked() != Some(id) {
            self.selected.set(Some(id));
        }
    }

    /// Whether a lit occurrence stands in the block `id` or below it.
    pub fn holds_lit(&self, id: u64) -> bool {
        let beats: Vec<usize> = self
            .story
            .with(|s| s.subtree(id))
            .into_iter()
            .filter_map(|b| self.beat_index(b))
            .collect();
        self.lit
            .with(|l| l.iter().any(|((b, _), _)| beats.contains(b)))
    }

    /// The bytes of the lit occurrences in the text of the beat `id`, in
    /// text order.
    pub fn lit_spans(&self, id: u64) -> Vec<std::ops::Range<usize>> {
        let Some(beat) = self.beat_index(id) else {
            return Vec::new();
        };
        self.lit.with(|l| {
            l.iter()
                .filter(|((b, _), _)| *b == beat)
                .map(|&((_, start), end)| start..end)
                .collect()
        })
    }

    /// Whether the note `note` of the beat `id` stands for a lit occurrence.
    pub fn note_lit(&self, id: u64, note: &Note) -> bool {
        let Some(beat) = self.beat_index(id) else {
            return false;
        };
        self.lit.with(|l| {
            self.analysis.with(|a| {
                a.elements.get(note.element).is_some_and(|el| {
                    el.occurrences.iter().any(|o| {
                        o.at.0 == beat && o.kind == note.kind && l.iter().any(|(p, _)| *p == o.at)
                    })
                })
            })
        })
    }

    /// Whether the dependency `d` stands out: both its ends are lit, or one
    /// stands in the beat under the pointer.
    pub fn dependency_lit(&self, d: &Dependency) -> bool {
        let pointed = self.pointed.get().and_then(|id| self.beat_index(id));
        pointed.is_some_and(|b| d.setup.0 == b || d.payoff.0 == b)
            || self.lit.with(|l| {
                [d.setup, d.payoff]
                    .iter()
                    .all(|end| l.iter().any(|(p, _)| p == end))
            })
    }

    /// Opens `popup` from the element the event `ev` is handled on, or closes
    /// it when it is the one open.
    pub fn toggle_popup(&self, popup: Popup, ev: &web_sys::Event) {
        let anchor = Anchor::of(ev);
        self.popup.update(|p| {
            *p = if p.map(|(q, _)| q) == Some(popup) {
                None
            } else {
                Some((popup, anchor))
            }
        });
    }

    pub fn close_popup(&self) {
        self.popup.set(None);
    }

    pub fn is_open(&self, popup: Popup) -> bool {
        self.popup.with(|p| p.is_some_and(|(q, _)| q == popup))
    }

    /// The margin notes of the beat `id`: one per element and kind, in text
    /// order, each pointing at the other end of its dependency.
    pub fn notes(&self, id: u64) -> Vec<Note> {
        let Some(beat) = self.beat_index(id) else {
            return Vec::new();
        };
        let label = |b: usize| self.beat_label(b);
        self.analysis.with(|a| {
            a.in_beat(beat)
                .into_iter()
                .map(|(e, kind)| {
                    let el = &a.elements[e];
                    let at = el
                        .occurrences
                        .iter()
                        .find(|o| o.at.0 == beat && o.kind == kind)
                        .map(|o| o.at);
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
                            format!(
                                "⚠ déjà déclaré en {}",
                                first(Kind::Setup).map_or_else(String::new, |p| label(p.0))
                            )
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
                        (None, Kind::Payoff) => first(Kind::Setup)
                            .map_or_else(String::new, |p| format!("← déclaré en {}", label(p.0))),
                        (None, Kind::Mention) => String::new(),
                    };
                    Note {
                        element: e,
                        name: el.name.clone(),
                        key: el.key.clone(),
                        kind,
                        relation,
                        warns: warning.is_some(),
                    }
                })
                .collect()
        })
    }

    /// Whether some warning points into the scene `id`.
    pub fn scene_warns(&self, id: u64) -> bool {
        let beats: Vec<usize> = self.live.with(|l| {
            l.iter()
                .filter(|n| n.parent == Some(id))
                .filter_map(|n| self.beat_index(n.id))
                .collect()
        });
        self.analysis
            .with(|a| a.warnings.iter().any(|w| beats.contains(&w.beat)))
    }

    pub fn toggle_focus(&self, key: String) {
        self.focus.update(|f| {
            *f = if f.as_deref() == Some(&key) {
                None
            } else {
                Some(key)
            }
        });
    }

    /// Selects the block `id` and scrolls the script to it: a scene or an act
    /// to its start, a beat to the middle of the view.
    pub fn reveal(&self, id: u64) {
        self.select(id);
        self.scrolled_by_page_until
            .set(js_sys::Date::now() + 1200.0);
        set_timeout(
            move || storage::reveal(id),
            std::time::Duration::from_millis(30),
        );
    }

    /// Selects the scene being read once the selection has scrolled out of
    /// view: the last whose heading has passed the reading line, a third
    /// down the view, or in pages, a quarter across.
    pub fn spy(&self) {
        if js_sys::Date::now() < self.scrolled_by_page_until.get_untracked() {
            return;
        }
        let Some(root) = self.script.get_untracked() else {
            return;
        };
        let view = root.get_bounding_client_rect();
        let across = root.scroll_width() > root.client_width();
        let scenes: Vec<u64> = self.live.with_untracked(|l| {
            l.iter()
                .filter(|n| n.level == Level::Scene)
                .map(|n| n.id)
                .collect()
        });
        let document = root.owner_document().expect("a document");
        let shown = |id: u64| {
            document
                .get_element_by_id(&storage::dom_id(id))
                .is_some_and(|el| {
                    let r = el.get_bounding_client_rect();
                    r.right() > view.left()
                        && r.left() < view.right()
                        && r.bottom() > view.top()
                        && r.top() < view.bottom()
                })
        };
        if self
            .path
            .with_untracked(|p| p.first().copied())
            .is_some_and(shown)
        {
            return;
        }
        let passed = |id: &u64| {
            let Some(head) = document.get_element_by_id(&storage::dom_id(*id)) else {
                return false;
            };
            let r = head.get_bounding_client_rect();
            if across {
                r.left() < view.left() + view.width() / 4.0
            } else {
                r.top() < view.top() + view.height() / 3.0
            }
        };
        let read = scenes
            .iter()
            .copied()
            .take_while(|id| passed(id))
            .last()
            .or(scenes.first().copied());
        if let Some(read) = read {
            self.select(read);
        }
    }

    /// Archives a block, after listing the elements that also occur outside
    /// it when there are any: archiving changes what those occurrences mean.
    pub fn archive(&self, id: u64) {
        let inside: Vec<usize> = self
            .story
            .with_untracked(|s| s.subtree(id))
            .into_iter()
            .filter_map(|b| {
                self.beats
                    .with_untracked(|v| v.iter().position(|&x| x == b))
            })
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
        if storage::confirm(&format!(
            "Supprimer définitivement « {label} » et tout ce qu'il contient ?"
        )) {
            self.story.update(|s| {
                s.remove(id);
            });
        }
    }

    /// The place of the engine's beat `beat`, such as `II.3.1`.
    pub fn beat_label(&self, beat: usize) -> String {
        let id = self.beats.with_untracked(|b| b.get(beat).copied());
        id.and_then(|id| {
            self.live
                .with_untracked(|l| l.iter().find(|n| n.id == id).map(|n| n.label.clone()))
        })
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
                if storage::confirm(&format!(
                    "Remplacer le scénario actuel par « {} » ?",
                    story.title
                )) {
                    self.story.set(story);
                    self.focus.set(None);
                }
            }
            Err(e) => storage::alert(&format!("Ce fichier n'est pas un scénario : {e}")),
        }
    }
}
