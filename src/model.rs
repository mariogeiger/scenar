//! The story: a tree of blocks at three fixed levels, acts, scenes and beats,
//! that can be written, moved, archived and restored.
//!
//! Text lives in beats; acts and scenes group them. An archived block stays
//! in the tree, restorable, but it and everything below it count as
//! nonexistent: they are left out of [`Story::live`], hence out of the
//! numbering and of the dependency engine.
//!
//! The JSON of a [`Story`] is the file format, for the browser's storage as
//! for export and import.

use serde::{Deserialize, Serialize};

/// The version of the file format this code writes and reads.
pub const FORMAT: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Act,
    Scene,
    Beat,
}

impl Level {
    fn of_depth(depth: usize) -> Level {
        [Level::Act, Level::Scene, Level::Beat][depth]
    }

    pub fn child(self) -> Option<Level> {
        match self {
            Level::Act => Some(Level::Scene),
            Level::Scene => Some(Level::Beat),
            Level::Beat => None,
        }
    }

    /// Its name with the indefinite article: `un acte`.
    pub fn indefinite(self) -> &'static str {
        match self {
            Level::Act => "un acte",
            Level::Scene => "une scène",
            Level::Beat => "un beat",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Level::Act => "Acte",
            Level::Scene => "Scène",
            Level::Beat => "Beat",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

impl Node {
    /// A new block of `level`, and below it one new block of each lower
    /// level, so that it holds a beat to write in. Ids are drawn at random,
    /// below 2⁵³ so that JavaScript reads them exactly: blocks that several
    /// people add at once to a shared story must not share one.
    fn fresh(level: Level) -> Node {
        let id = fastrand::u64(1..1 << 53);
        let title = match level {
            Level::Act => "Nouvel acte",
            Level::Scene => "Nouvelle scène",
            Level::Beat => "",
        };
        let children = level.child().map(|l| vec![Node::fresh(l)]).unwrap_or_default();
        Node { id, title: title.into(), text: String::new(), archived: false, children }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub format: u32,
    pub title: String,
    pub acts: Vec<Node>,
}

/// Before or after a block, where a moved block lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Before,
    After,
}

/// A block that exists, that is, neither archived nor below an archived one.
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    pub id: u64,
    pub level: Level,
    pub parent: Option<u64>,
    /// Its rank among the living blocks of its parent, from 1.
    pub number: usize,
    /// Its place in the story: `II`, `II.3`, `II.3.1`.
    pub label: String,
}

impl Story {
    /// One act holding one scene holding one empty beat.
    pub fn blank() -> Story {
        let mut story = Story { format: FORMAT, title: "Sans titre".into(), acts: Vec::new() };
        story.add(None, None);
        story
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a story serializes")
    }

    /// Reads a story, refusing one this code cannot have written.
    pub fn from_json(json: &str) -> Result<Story, String> {
        let story: Story = serde_json::from_str(json).map_err(|e| format!("JSON illisible : {e}"))?;
        if story.format != FORMAT {
            return Err(format!("format {} inconnu (attendu : {FORMAT})", story.format));
        }
        fn check(nodes: &[Node], depth: usize, ids: &mut Vec<u64>) -> Result<(), String> {
            for n in nodes {
                if depth > 2 {
                    return Err("un beat ne contient pas de blocs".into());
                }
                if ids.contains(&n.id) {
                    return Err(format!("identifiant {} en double", n.id));
                }
                ids.push(n.id);
                check(&n.children, depth + 1, ids)?;
            }
            Ok(())
        }
        check(&story.acts, 0, &mut Vec::new())?;
        Ok(story)
    }

    /// The indices that lead from the acts to the block `id`.
    fn path(&self, id: u64) -> Option<Vec<usize>> {
        fn search(nodes: &[Node], id: u64, path: &mut Vec<usize>) -> bool {
            for (i, n) in nodes.iter().enumerate() {
                path.push(i);
                if n.id == id || search(&n.children, id, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        search(&self.acts, id, &mut path).then_some(path)
    }

    fn at(&self, path: &[usize]) -> &Node {
        let mut node = &self.acts[path[0]];
        for &i in &path[1..] {
            node = &node.children[i];
        }
        node
    }

    /// The children of the block at `parent`, or the acts for the empty path.
    fn list_mut(&mut self, parent: &[usize]) -> &mut Vec<Node> {
        let mut list = &mut self.acts;
        for &i in parent {
            list = &mut list[i].children;
        }
        list
    }

    pub fn find(&self, id: u64) -> Option<&Node> {
        self.path(id).map(|p| self.at(&p))
    }

    fn find_mut(&mut self, id: u64) -> Option<&mut Node> {
        let path = self.path(id)?;
        let (&last, parent) = path.split_last()?;
        Some(&mut self.list_mut(parent)[last])
    }

    pub fn level(&self, id: u64) -> Option<Level> {
        self.path(id).map(|p| Level::of_depth(p.len() - 1))
    }

    pub fn parent(&self, id: u64) -> Option<u64> {
        let path = self.path(id)?;
        (path.len() > 1).then(|| self.at(&path[..path.len() - 1]).id)
    }

    /// The ids of `id` and of every block below it.
    pub fn subtree(&self, id: u64) -> Vec<u64> {
        fn collect(n: &Node, out: &mut Vec<u64>) {
            out.push(n.id);
            n.children.iter().for_each(|c| collect(c, out));
        }
        let mut out = Vec::new();
        if let Some(n) = self.find(id) {
            collect(n, &mut out);
        }
        out
    }

    /// What names a block in a list: its title, or for an untitled beat the
    /// start of its text.
    pub fn label(&self, id: u64) -> String {
        let Some(n) = self.find(id) else {
            return String::new();
        };
        if !n.title.trim().is_empty() {
            return n.title.clone();
        }
        let words: Vec<&str> = n.text.split_whitespace().collect();
        match words.len() {
            0 => "(beat vide)".into(),
            1..=5 => words.join(" "),
            _ => format!("{}…", words[..5].join(" ")),
        }
    }

    /// Adds a new block, with a beat below it, as the last child of `parent`
    /// (a new act when `None`), or right after its child `after`. Returns its
    /// id, or `None` when `parent` is a beat or does not exist.
    pub fn add(&mut self, parent: Option<u64>, after: Option<u64>) -> Option<u64> {
        let (level, path) = match parent {
            None => (Level::Act, Vec::new()),
            Some(p) => (self.level(p)?.child()?, self.path(p)?),
        };
        let node = Node::fresh(level);
        let id = node.id;
        let list = self.list_mut(&path);
        let at = after.and_then(|a| list.iter().position(|n| n.id == a)).map_or(list.len(), |i| i + 1);
        list.insert(at, node);
        Some(id)
    }

    pub fn set_title(&mut self, id: u64, title: String) {
        if let Some(n) = self.find_mut(id) {
            n.title = title;
        }
    }

    pub fn set_text(&mut self, id: u64, text: String) {
        if let Some(n) = self.find_mut(id) {
            n.text = text;
        }
    }

    pub fn set_archived(&mut self, id: u64, archived: bool) {
        if let Some(n) = self.find_mut(id) {
            n.archived = archived;
        }
    }

    /// Deletes a block and everything below it, for good.
    pub fn remove(&mut self, id: u64) -> Option<Node> {
        let path = self.path(id)?;
        let (&last, parent) = path.split_last()?;
        Some(self.list_mut(parent).remove(last))
    }

    /// Swaps a block with its nearest living sibling on `side`, skipping
    /// archived ones. Returns whether it moved.
    pub fn step(&mut self, id: u64, side: Side) -> bool {
        let Some(path) = self.path(id) else {
            return false;
        };
        let (&i, parent) = path.split_last().expect("a path is never empty");
        let list = self.list_mut(parent);
        let other = match side {
            Side::Before => (0..i).rev().find(|&j| !list[j].archived),
            Side::After => (i + 1..list.len()).find(|&j| !list[j].archived),
        };
        other.map(|j| list.swap(i, j)).is_some()
    }

    /// Whether `id` can be dropped on `target`: beside a block of its level,
    /// or into a block of the level above.
    pub fn can_place(&self, id: u64, target: u64) -> bool {
        match (self.level(id), self.level(target)) {
            (Some(l), Some(t)) => id != target && (l == t || t.child() == Some(l)),
            _ => false,
        }
    }

    /// Moves `id` to `side` of `target` when they share a level, or to the end
    /// of `target` when it is one level up. Returns whether it moved.
    pub fn place(&mut self, id: u64, target: u64, side: Side) -> bool {
        if !self.can_place(id, target) {
            return false;
        }
        let beside = self.level(id) == self.level(target);
        let node = self.remove(id).expect("can_place found it");
        let path = self.path(target).expect("target is not below id");
        if beside {
            let (&i, parent) = path.split_last().expect("a path is never empty");
            self.list_mut(parent).insert(i + (side == Side::After) as usize, node);
        } else {
            self.list_mut(&path).push(node);
        }
        true
    }

    /// The living blocks, in story order: each before its children.
    pub fn live(&self) -> Vec<Live> {
        fn walk(nodes: &[Node], depth: usize, parent: Option<u64>, prefix: &str, out: &mut Vec<Live>) {
            for (number, n) in nodes.iter().filter(|n| !n.archived).enumerate().map(|(i, n)| (i + 1, n)) {
                let label = if depth == 0 { roman(number) } else { format!("{prefix}.{number}") };
                out.push(Live { id: n.id, level: Level::of_depth(depth), parent, number, label: label.clone() });
                walk(&n.children, depth + 1, Some(n.id), &label, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.acts, 0, None, "", &mut out);
        out
    }

    /// The archived blocks, as the bin lists them: each topmost one, not
    /// what lies below it.
    pub fn archived(&self) -> Vec<(u64, Level)> {
        fn walk(nodes: &[Node], depth: usize, out: &mut Vec<(u64, Level)>) {
            for n in nodes {
                if n.archived {
                    out.push((n.id, Level::of_depth(depth)));
                } else {
                    walk(&n.children, depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.acts, 0, &mut out);
        out
    }
}

/// `n` in Roman numerals, for acts.
fn roman(mut n: usize) -> String {
    const DIGITS: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, digit) in DIGITS {
        while n >= value {
            out.push_str(digit);
            n -= value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two acts; the first holds scenes 2 and 5, of beats 3, 4 and 6.
    fn story() -> Story {
        let beat =
            |id| Node { id, title: String::new(), text: format!("beat {id}"), archived: false, children: vec![] };
        let node = |id, children| Node { id, title: format!("n{id}"), text: String::new(), archived: false, children };
        Story {
            format: FORMAT,
            title: "t".into(),
            acts: vec![node(1, vec![node(2, vec![beat(3), beat(4)]), node(5, vec![beat(6)])]), node(7, vec![])],
        }
    }

    fn labels(s: &Story) -> Vec<(u64, String)> {
        s.live().into_iter().map(|l| (l.id, l.label)).collect()
    }

    fn children(s: &Story, id: u64) -> Vec<u64> {
        s.find(id).unwrap().children.iter().map(|n| n.id).collect()
    }

    #[test]
    fn live_numbers_the_tree_and_skips_archived_blocks_with_their_children() {
        let mut s = story();
        assert_eq!(
            labels(&s),
            [(1, "I"), (2, "I.1"), (3, "I.1.1"), (4, "I.1.2"), (5, "I.2"), (6, "I.2.1"), (7, "II")]
                .map(|(i, l)| (i, l.to_string()))
        );
        s.set_archived(2, true);
        assert_eq!(labels(&s), [(1, "I"), (5, "I.1"), (6, "I.1.1"), (7, "II")].map(|(i, l)| (i, l.to_string())));
        assert_eq!(s.archived(), [(2, Level::Scene)]);
    }

    #[test]
    fn step_skips_archived_siblings() {
        let mut s = story();
        let new = s.add(Some(2), None).unwrap();
        s.set_archived(4, true);
        assert!(s.step(new, Side::Before));
        assert_eq!(children(&s, 2), [new, 4, 3]);
        assert!(!s.step(new, Side::Before));
    }

    #[test]
    fn place_beside_same_level_and_into_the_level_above() {
        let mut s = story();
        assert!(s.place(6, 3, Side::Before));
        assert_eq!(children(&s, 2), [6, 3, 4]);
        assert!(s.place(3, 7, Side::After) == false);
        assert!(s.place(5, 7, Side::Before));
        assert_eq!(children(&s, 7), [5]);
        assert!(s.place(4, 5, Side::After));
        assert_eq!(children(&s, 5), [4]);
        assert!(!s.place(1, 3, Side::After));
        assert!(!s.place(4, 4, Side::After));
    }

    #[test]
    fn add_makes_blocks_down_to_a_beat_after_the_given_sibling() {
        let mut s = story();
        let scene = s.add(Some(1), Some(2)).unwrap();
        assert_eq!(children(&s, 1), [2, scene, 5]);
        assert_eq!(s.find(scene).unwrap().children.len(), 1);
        assert_eq!(s.add(Some(3), None), None);
        let act = s.add(None, None).unwrap();
        assert_eq!(s.subtree(act).len(), 3);
    }

    #[test]
    fn json_round_trips_and_rejects_what_it_cannot_have_written() {
        let s = story();
        assert_eq!(Story::from_json(&s.to_json()).unwrap(), s);
        assert!(Story::from_json(&s.to_json().replace("\"id\": 4", "\"id\": 3")).is_err());
        assert!(Story::from_json(&s.to_json().replace("\"format\": 1", "\"format\": 2")).is_err());
    }

    #[test]
    fn untitled_beats_are_labelled_by_their_text() {
        let mut s = story();
        s.set_text(3, "un deux trois quatre cinq six".into());
        assert_eq!(s.label(3), "un deux trois quatre cinq…");
        assert_eq!(s.label(2), "n2");
    }
}
