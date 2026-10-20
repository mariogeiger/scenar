//! The story as a shared document: a Yrs CRDT that several browsers edit at
//! once and that merges what they do, letter by letter.
//!
//! The document holds the story's title as the root text `title`, and the
//! root map `nodes` from each block's id to its fields: `level` (0 for an
//! act, 1 a scene, 2 a beat), `parent` (the parent's id, empty for an act),
//! `rank` (see [`crate::rank`]), `archived`, and the texts `title` and
//! `text`.
//!
//! A block's place, its parent and rank, is a register that the last writer
//! wins: a block two people move at once lands in one place, never two, and
//! since a parent is always one level up, no move makes a cycle. Siblings
//! are ordered by rank, then id. A block whose parent is gone, deleted while
//! someone added to it, is left out.

use std::collections::{HashMap, HashSet};

use yrs::updates::decoder::Decode;
use yrs::{
    Doc, GetString, Map, MapPrelim, MapRef, Out, ReadTxn, StateVector, Text, TextPrelim, TextRef, Transact, TransactionMut,
    Update,
};

use crate::model::{FORMAT, Node, Story};
use crate::rank;

/// The origin of the transactions that apply what came from elsewhere.
pub const REMOTE: &str = "remote";

/// A text of the story: its title, or a block's title or text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    StoryTitle,
    Title(u64),
    Text(u64),
}

/// One edit of a text: `removed` bytes from `at` replaced by `inserted`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Splice {
    pub at: usize,
    pub removed: usize,
    pub inserted: String,
}

impl Splice {
    /// The edit that turns `old` into `new`: what lies between their common
    /// start and their common end.
    pub fn between(old: &str, new: &str) -> Splice {
        let start = old.char_indices().zip(new.chars()).take_while(|((_, a), b)| a == b).last();
        let at = start.map_or(0, |((i, c), _)| i + c.len_utf8());
        let room = old.len().min(new.len()) - at;
        let end = old[at..].chars().rev().zip(new[at..].chars().rev()).take_while(|(a, b)| a == b);
        let mut tail = 0;
        for (c, _) in end {
            if tail + c.len_utf8() > room {
                break;
            }
            tail += c.len_utf8();
        }
        Splice { at, removed: old.len() - tail - at, inserted: new[at..new.len() - tail].to_string() }
    }

    /// Where the byte `p` of the text before the edit stands after it; a
    /// position inside what was removed goes to the end of what came in.
    pub fn map(&self, p: usize) -> usize {
        if p <= self.at {
            p
        } else if p >= self.at + self.removed {
            p - self.removed + self.inserted.len()
        } else {
            self.at + self.inserted.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.removed == 0 && self.inserted.is_empty()
    }
}

pub struct Shared {
    doc: Doc,
    title: TextRef,
    nodes: MapRef,
}

/// A block as the document holds it.
struct Entry {
    id: u64,
    level: usize,
    parent: Option<u64>,
    rank: String,
    title: String,
    text: String,
    archived: bool,
}

impl Default for Shared {
    fn default() -> Shared {
        Shared::new(Doc::new())
    }
}

impl Shared {
    pub fn new(doc: Doc) -> Shared {
        let title = doc.get_or_insert_text("title");
        let nodes = doc.get_or_insert_map("nodes");
        Shared { doc, title, nodes }
    }

    /// A new document that holds `story`.
    pub fn of(story: &Story) -> Shared {
        let shared = Shared::default();
        shared.write(story);
        shared
    }

    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// The whole document, as an update that recreates it.
    pub fn encode(&self) -> Vec<u8> {
        self.doc.transact().encode_state_as_update_v1(&StateVector::default())
    }

    /// Merges an update that came from elsewhere.
    pub fn apply(&self, update: &[u8]) -> Result<(), String> {
        let update = Update::decode_v1(update).map_err(|e| e.to_string())?;
        self.doc.transact_mut_with(REMOTE).apply_update(update).map_err(|e| e.to_string())
    }

    /// The story the document holds now.
    pub fn read(&self) -> Story {
        let txn = self.doc.transact();
        let mut below: HashMap<Option<u64>, Vec<Entry>> = HashMap::new();
        for (key, value) in self.nodes.iter(&txn) {
            if let (Ok(id), Out::YMap(map)) = (key.parse(), value) {
                let e = entry(&txn, id, &map);
                below.entry(e.parent).or_default().push(e);
            }
        }
        for list in below.values_mut() {
            list.sort_by(|a, b| (&a.rank, a.id).cmp(&(&b.rank, b.id)));
        }
        fn build(below: &HashMap<Option<u64>, Vec<Entry>>, parent: Option<u64>, level: usize) -> Vec<Node> {
            let Some(list) = below.get(&parent) else { return Vec::new() };
            list.iter()
                .filter(|e| e.level == level)
                .map(|e| Node {
                    id: e.id,
                    title: e.title.clone(),
                    text: e.text.clone(),
                    archived: e.archived,
                    children: if level < 2 { build(below, Some(e.id), level + 1) } else { Vec::new() },
                })
                .collect()
        }
        Story { format: FORMAT, title: self.title.get_string(&txn), acts: build(&below, None, 0) }
    }

    /// Makes the document hold `story`, by the fewest changes: blocks gone
    /// are deleted, new ones added, texts spliced, and among siblings only
    /// those out of order get a new rank.
    pub fn write(&self, story: &Story) {
        let mut txn = self.doc.transact_mut();
        splice_to(&mut txn, &self.title, &story.title);
        let mut wanted: Vec<(&Node, usize, Option<u64>)> = Vec::new();
        fn walk<'a>(nodes: &'a [Node], level: usize, parent: Option<u64>, out: &mut Vec<(&'a Node, usize, Option<u64>)>) {
            for n in nodes {
                out.push((n, level, parent));
                walk(&n.children, level + 1, Some(n.id), out);
            }
        }
        walk(&story.acts, 0, None, &mut wanted);
        let kept: HashSet<u64> = wanted.iter().map(|(n, _, _)| n.id).collect();
        let mut was: HashMap<u64, (Option<u64>, String)> = HashMap::new();
        let mut gone = Vec::new();
        for (key, value) in self.nodes.iter(&txn) {
            match (key.parse::<u64>(), value) {
                (Ok(id), Out::YMap(map)) if kept.contains(&id) => {
                    let e = entry(&txn, id, &map);
                    was.insert(id, (e.parent, e.rank));
                }
                _ => gone.push(key.to_string()),
            }
        }
        for key in gone {
            self.nodes.remove(&mut txn, &key);
        }
        for &(n, level, _) in &wanted {
            let map = match self.nodes.get(&txn, &n.id.to_string()) {
                Some(Out::YMap(map)) => map,
                _ => {
                    let map = self.nodes.insert(&mut txn, n.id.to_string(), MapPrelim::default());
                    map.insert(&mut txn, "level", level as i64);
                    map.insert(&mut txn, "title", TextPrelim::new(""));
                    map.insert(&mut txn, "text", TextPrelim::new(""));
                    map
                }
            };
            for (key, value) in [("title", &n.title), ("text", &n.text)] {
                if let Some(Out::YText(text)) = map.get(&txn, key) {
                    splice_to(&mut txn, &text, value);
                }
            }
            if flag(&txn, &map, "archived") != n.archived {
                map.insert(&mut txn, "archived", n.archived);
            }
        }
        // Siblings, list by list: the longest run already in order keeps its
        // ranks; every other block gets one between its neighbours.
        let mut lists: Vec<(Option<u64>, Vec<u64>)> = Vec::new();
        for &(n, _, parent) in &wanted {
            match lists.iter_mut().find(|(p, _)| *p == parent) {
                Some((_, list)) => list.push(n.id),
                None => lists.push((parent, vec![n.id])),
            }
        }
        for (parent, list) in lists {
            let old: Vec<Option<&str>> = list
                .iter()
                .map(|id| was.get(id).filter(|(p, _)| *p == parent).map(|(_, r)| r.as_str()))
                .collect();
            let keep = increasing(&old);
            let mut low: Option<String> = None;
            for (i, &id) in list.iter().enumerate() {
                let rank = if keep[i] {
                    old[i].expect("kept ranks exist").to_string()
                } else {
                    let high = (i + 1..list.len()).find(|&j| keep[j]).and_then(|j| old[j]);
                    rank::between(low.as_deref(), high)
                };
                let Some(Out::YMap(map)) = self.nodes.get(&txn, &id.to_string()) else { continue };
                let place = parent.map_or_else(String::new, |p| p.to_string());
                if was.get(&id).map(|(p, _)| *p) != Some(parent) || !was.contains_key(&id) {
                    map.insert(&mut txn, "parent", place);
                }
                if !keep[i] {
                    map.insert(&mut txn, "rank", rank.clone());
                }
                low = Some(rank);
            }
        }
    }

    /// Applies to a text the edit a reader made to `seen`, the text as they
    /// saw it. When the document has changed since, by someone else, the edit
    /// is moved past that change.
    pub fn splice(&self, field: Field, seen: &str, edit: &Splice) {
        let mut txn = self.doc.transact_mut();
        let Some(text) = self.text(&txn, field) else { return };
        let now = text.get_string(&txn);
        let (mut at, mut end) = (edit.at, edit.at + edit.removed);
        if now != seen {
            let drift = Splice::between(seen, &now);
            (at, end) = (drift.map(at), drift.map(end));
        }
        let fit = |p: usize| (0..=p.min(now.len())).rev().find(|&q| now.is_char_boundary(q)).unwrap_or(0);
        let (at, end) = (fit(at), fit(end.max(at)));
        if end > at {
            text.remove_range(&mut txn, at as u32, (end - at) as u32);
        }
        if !edit.inserted.is_empty() {
            text.insert(&mut txn, at as u32, &edit.inserted);
        }
    }

    /// The text `field` stands for, if it exists.
    pub fn text<T: ReadTxn>(&self, txn: &T, field: Field) -> Option<TextRef> {
        let (id, key) = match field {
            Field::StoryTitle => return Some(self.title.clone()),
            Field::Title(id) => (id, "title"),
            Field::Text(id) => (id, "text"),
        };
        let Some(Out::YMap(map)) = self.nodes.get(txn, &id.to_string()) else { return None };
        match map.get(txn, key) {
            Some(Out::YText(text)) => Some(text),
            _ => None,
        }
    }
}

fn entry<T: ReadTxn>(txn: &T, id: u64, map: &MapRef) -> Entry {
    let string = |key: &str| match map.get(txn, key) {
        Some(Out::YText(t)) => t.get_string(txn),
        Some(Out::Any(yrs::Any::String(s))) => s.to_string(),
        _ => String::new(),
    };
    let level = match map.get(txn, "level") {
        Some(Out::Any(yrs::Any::Number(yrs::Number::Int(n)))) => n as usize,
        Some(Out::Any(yrs::Any::Number(yrs::Number::Float(n)))) => n as usize,
        _ => usize::MAX,
    };
    Entry {
        id,
        level,
        parent: string("parent").parse().ok(),
        rank: string("rank"),
        title: string("title"),
        text: string("text"),
        archived: flag(txn, map, "archived"),
    }
}

fn flag<T: ReadTxn>(txn: &T, map: &MapRef, key: &str) -> bool {
    matches!(map.get(txn, key), Some(Out::Any(yrs::Any::Bool(true))))
}

/// Makes `text` read `wanted`, by one splice.
fn splice_to(txn: &mut TransactionMut, text: &TextRef, wanted: &str) {
    let now = text.get_string(txn);
    let edit = Splice::between(&now, wanted);
    if edit.removed > 0 {
        text.remove_range(txn, edit.at as u32, edit.removed as u32);
    }
    if !edit.inserted.is_empty() {
        text.insert(txn, edit.at as u32, &edit.inserted);
    }
}

/// Which of `ranks` to keep: a longest run of them that rises strictly, as
/// they stand; a missing rank is never kept.
fn increasing(ranks: &[Option<&str>]) -> Vec<bool> {
    let n = ranks.len();
    // best[i]: the length of the longest rising run that ends at i.
    let mut best = vec![0usize; n];
    let mut from = vec![None; n];
    for i in 0..n {
        let Some(r) = ranks[i] else { continue };
        best[i] = 1;
        for j in 0..i {
            if ranks[j].is_some_and(|q| q < r) && best[j] + 1 > best[i] {
                best[i] = best[j] + 1;
                from[i] = Some(j);
            }
        }
    }
    let mut keep = vec![false; n];
    let mut at = (0..n).filter(|&i| best[i] > 0).max_by_key(|&i| best[i]);
    while let Some(i) = at {
        keep[i] = true;
        at = from[i];
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Side;

    fn pair(story: &Story) -> (Shared, Shared) {
        let a = Shared::of(story);
        let b = Shared::default();
        b.apply(&a.encode()).unwrap();
        (a, b)
    }

    /// Sends each side what the other has.
    fn exchange(a: &Shared, b: &Shared) {
        let (ua, ub) = (a.encode(), b.encode());
        a.apply(&ub).unwrap();
        b.apply(&ua).unwrap();
    }

    fn ranks(s: &Shared) -> HashMap<u64, String> {
        let txn = s.doc.transact();
        s.nodes
            .iter(&txn)
            .filter_map(|(k, v)| match v {
                Out::YMap(m) => Some((k.parse().unwrap(), entry(&txn, 0, &m).rank)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_story_round_trips() {
        let story = crate::example::kerlaz();
        let (a, b) = pair(&story);
        assert_eq!(a.read(), story);
        assert_eq!(b.read(), story);
    }

    #[test]
    fn splice_between_and_map() {
        let s = Splice::between("le chat dort", "le gros chat dort");
        assert_eq!(s, Splice { at: 3, removed: 0, inserted: "gros ".into() });
        assert_eq!((s.map(2), s.map(3), s.map(12)), (2, 3, 17));
        assert_eq!(Splice::between("aa", "aaa"), Splice { at: 2, removed: 0, inserted: "a".into() });
        assert_eq!(Splice::between("é", "è"), Splice { at: 0, removed: 2, inserted: "è".into() });
        assert!(Splice::between("même", "même").is_empty());
    }

    #[test]
    fn concurrent_edits_merge_letter_by_letter_and_block_by_block() {
        let story = crate::example::kerlaz();
        let (a, b) = pair(&story);
        let beat = story.acts[0].children[0].children[0].id;
        let text = story.acts[0].children[0].children[0].text.clone();
        // A types at the start of a beat, B at its end, and B adds a beat.
        a.splice(Field::Text(beat), &text, &Splice { at: 0, removed: 0, inserted: "Nuit. ".into() });
        b.splice(Field::Text(beat), &text, &Splice { at: text.len(), removed: 0, inserted: " Fin.".into() });
        let mut more = b.read();
        let scene = more.acts[0].children[0].id;
        let new = more.add(Some(scene), None).unwrap();
        b.write(&more);
        exchange(&a, &b);
        let (ra, rb) = (a.read(), b.read());
        assert_eq!(ra, rb);
        assert_eq!(ra.find(beat).unwrap().text, format!("Nuit. {text} Fin."));
        assert!(ra.find(new).is_some());
    }

    #[test]
    fn a_block_moved_by_two_lands_in_one_place() {
        let story = crate::example::kerlaz();
        let (a, b) = pair(&story);
        let scenes: Vec<u64> = story.acts[0].children.iter().map(|n| n.id).collect();
        let beat = story.acts[0].children[0].children[0].id;
        let (mut sa, mut sb) = (a.read(), b.read());
        assert!(sa.place(beat, scenes[1], Side::After));
        assert!(sb.place(beat, scenes[2], Side::After));
        a.write(&sa);
        b.write(&sb);
        exchange(&a, &b);
        let merged = a.read();
        assert_eq!(merged, b.read());
        let everywhere: usize = merged.acts.iter().map(|act| merged.subtree(act.id).iter().filter(|&&n| n == beat).count()).sum();
        assert_eq!(everywhere, 1);
        assert!([sa.parent(beat), sb.parent(beat)].contains(&merged.parent(beat)));
    }

    #[test]
    fn a_move_rewrites_one_rank() {
        let story = crate::example::kerlaz();
        let a = Shared::of(&story);
        let before = ranks(&a);
        let mut s = a.read();
        let scene = &s.acts[0].children[1];
        let (first, last) = (scene.children[0].id, scene.children.last().unwrap().id);
        assert!(s.place(first, last, Side::After));
        a.write(&s);
        assert_eq!(a.read(), s);
        let after = ranks(&a);
        assert_eq!(before.iter().filter(|(k, r)| after[*k] != **r).count(), 1);
    }

    #[test]
    fn an_edit_on_a_stale_text_lands_where_it_was_meant() {
        let story = crate::example::kerlaz();
        let a = Shared::of(&story);
        let beat = story.acts[0].children[0].children[0].id;
        let seen = a.read().find(beat).unwrap().text.clone();
        // Someone else prepends; the reader, unaware, inserts after the first word.
        a.splice(Field::Text(beat), &seen, &Splice { at: 0, removed: 0, inserted: "Hier. ".into() });
        let word = seen.find(' ').unwrap();
        a.splice(Field::Text(beat), &seen, &Splice { at: word, removed: 0, inserted: " (seule)".into() });
        let text = a.read().find(beat).unwrap().text.clone();
        assert!(text.starts_with(&format!("Hier. {} (seule) ", &seen[..word])), "{text}");
    }

    #[test]
    fn a_block_under_a_deleted_parent_is_left_out() {
        let story = crate::example::kerlaz();
        let (a, b) = pair(&story);
        let act = story.acts[2].id;
        let mut sa = a.read();
        sa.remove(act);
        a.write(&sa);
        let mut sb = b.read();
        let scene = sb.add(Some(act), None).unwrap();
        b.write(&sb);
        exchange(&a, &b);
        assert_eq!(a.read(), b.read());
        assert!(a.read().find(scene).is_none());
    }
}
