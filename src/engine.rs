//! The dependency engine: what beats declare, pay off and mention.
//!
//! A beat names an element the way code names a variable. `@pistolet`
//! declares it, its setup, and may give it aliases: `@pistolet(flingue,
//! arme)`. `!pistolet` is its payoff, under its name or an alias. Any other
//! occurrence of the name or of an alias, as whole words, regardless of case
//! but not of accents, is a mention. Plurals are not guessed: a variant is an
//! alias.
//!
//! Read in story order, the occurrences are checked like a compiler checks
//! variables, by the four rules of [`Rule`]. The engine sees only the texts
//! of the beats, in order: the caller leaves archived beats out, so they count
//! as nonexistent.

use std::collections::HashMap;
use std::ops::Range;

/// What an occurrence does to its element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Setup,
    Mention,
    Payoff,
}

impl Kind {
    /// The past participle that says what the beat does with the element.
    pub fn verb(self) -> &'static str {
        match self {
            Kind::Setup => "déclaré",
            Kind::Mention => "mentionné",
            Kind::Payoff => "payé",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Kind::Setup => "▲",
            Kind::Mention => "●",
            Kind::Payoff => "■",
        }
    }
}

/// Where an occurrence stands in the story: the beat's index, then the byte
/// offset in its text. Ordering positions orders the story.
pub type Position = (usize, usize);

#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    pub kind: Kind,
    pub at: Position,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The name as first written.
    pub name: String,
    /// The lowercase words that identify it.
    pub key: String,
    pub aliases: Vec<String>,
    /// In story order.
    pub occurrences: Vec<Occurrence>,
}

/// The four ways an element can break the story.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// A payoff or a mention comes before the first setup.
    UsedBeforeDeclared,
    /// A payoff with no setup anywhere.
    PaidUndeclared,
    /// A setup after the first one.
    DeclaredTwice,
    /// A setup with no payoff after it.
    NeverPaid,
}

impl Rule {
    pub fn message(self) -> &'static str {
        match self {
            Rule::UsedBeforeDeclared => "utilisé avant d'être déclaré",
            Rule::PaidUndeclared => "payé sans avoir été déclaré",
            Rule::DeclaredTwice => "déclaré deux fois",
            Rule::NeverPaid => "déclaré mais jamais payé",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Warning {
    pub rule: Rule,
    /// Index into [`Analysis::elements`].
    pub element: usize,
    /// The beat it points at.
    pub beat: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Analysis {
    /// In order of first occurrence.
    pub elements: Vec<Element>,
    /// In story order.
    pub warnings: Vec<Warning>,
}

impl Analysis {
    /// The elements that `beat` holds, each once per kind, in text order.
    pub fn in_beat(&self, beat: usize) -> Vec<(usize, Kind)> {
        let mut found: Vec<(usize, usize, Kind)> = self
            .elements
            .iter()
            .enumerate()
            .flat_map(|(e, el)| {
                el.occurrences
                    .iter()
                    .filter(move |o| o.at.0 == beat)
                    .map(move |o| (o.at.1, e, o.kind))
            })
            .collect();
        found.sort();
        let mut seen = Vec::new();
        found.retain(|&(_, e, k)| {
            let new = !seen.contains(&(e, k));
            seen.push((e, k));
            new
        });
        found.into_iter().map(|(_, e, k)| (e, k)).collect()
    }

    /// The element whose key is `key`.
    pub fn element(&self, key: &str) -> Option<usize> {
        self.elements.iter().position(|e| e.key == key)
    }
}

/// Checks the beats of a story, given in story order.
pub fn analyze<'a>(texts: impl IntoIterator<Item = &'a str>) -> Analysis {
    let beats: Vec<(Vec<Marker>, Vec<(usize, String)>)> = texts
        .into_iter()
        .map(|text| {
            let marks = markers(text);
            let free = words(text)
                .into_iter()
                .filter(|(at, _)| !marks.iter().any(|m| m.span.contains(at)))
                .collect();
            (marks, free)
        })
        .collect();

    let mut elements: Vec<Element> = Vec::new();
    let mut index: HashMap<Vec<String>, usize> = HashMap::new();

    // Setups first, so that every name and alias is known before payoffs
    // resolve against them, wherever they stand.
    let mut phrases: Vec<(Vec<String>, usize)> = Vec::new();
    for (b, (marks, _)) in beats.iter().enumerate() {
        for m in marks.iter().filter(|m| m.kind == Kind::Setup) {
            let e = intern(&mut elements, &mut index, &m.name);
            phrases.push((key(&m.name), e));
            elements[e].occurrences.push(Occurrence { kind: Kind::Setup, at: (b, m.span.start) });
            for alias in &m.aliases {
                let words = key(alias);
                if words.is_empty() || phrases.iter().any(|(p, _)| *p == words) {
                    continue;
                }
                index.entry(words.clone()).or_insert(e);
                phrases.push((words, e));
                elements[e].aliases.push(alias.clone());
            }
        }
    }
    let declared: HashMap<Vec<String>, usize> = phrases.into_iter().rev().collect();

    for (b, (marks, _)) in beats.iter().enumerate() {
        for m in marks.iter().filter(|m| m.kind == Kind::Payoff) {
            let e = match declared.get(&key(&m.name)) {
                Some(&e) => e,
                None => intern(&mut elements, &mut index, &m.name),
            };
            elements[e].occurrences.push(Occurrence { kind: Kind::Payoff, at: (b, m.span.start) });
        }
    }

    // Mentions, longest phrase first, so that an alias of two words is not
    // also counted as its last word.
    let longest = declared.keys().map(Vec::len).max().unwrap_or(0);
    for (b, (_, free)) in beats.iter().enumerate() {
        let mut i = 0;
        while i < free.len() {
            let found = (1..=longest.min(free.len() - i)).rev().find_map(|n| {
                let phrase: Vec<String> = free[i..i + n].iter().map(|(_, w)| w.clone()).collect();
                declared.get(&phrase).map(|&e| (e, n))
            });
            match found {
                Some((e, n)) => {
                    elements[e].occurrences.push(Occurrence { kind: Kind::Mention, at: (b, free[i].0) });
                    i += n;
                }
                None => i += 1,
            }
        }
    }

    for el in &mut elements {
        el.occurrences.sort_by_key(|o| o.at);
    }
    elements.sort_by_key(|el| el.occurrences.first().map(|o| o.at));

    let mut warnings = Vec::new();
    for (e, el) in elements.iter().enumerate() {
        let mut warn = |rule, beat| warnings.push(Warning { rule, element: e, beat });
        let setups: Vec<Position> =
            el.occurrences.iter().filter(|o| o.kind == Kind::Setup).map(|o| o.at).collect();
        match setups.first() {
            None => {
                for o in el.occurrences.iter().filter(|o| o.kind == Kind::Payoff) {
                    warn(Rule::PaidUndeclared, o.at.0);
                }
            }
            Some(&first) => {
                for o in el.occurrences.iter().filter(|o| o.kind != Kind::Setup && o.at < first) {
                    warn(Rule::UsedBeforeDeclared, o.at.0);
                }
                for s in &setups[1..] {
                    warn(Rule::DeclaredTwice, s.0);
                }
                if !el.occurrences.iter().any(|o| o.kind == Kind::Payoff && o.at > first) {
                    warn(Rule::NeverPaid, first.0);
                }
            }
        }
    }
    warnings.sort_by_key(|w| (w.beat, w.element, w.rule));
    warnings.dedup();

    Analysis { elements, warnings }
}

/// The element that `phrase` names, made on first sight.
fn intern(elements: &mut Vec<Element>, index: &mut HashMap<Vec<String>, usize>, phrase: &str) -> usize {
    let words = key(phrase);
    *index.entry(words.clone()).or_insert_with(|| {
        elements.push(Element {
            name: phrase.to_string(),
            key: words.join(" "),
            aliases: Vec::new(),
            occurrences: Vec::new(),
        });
        elements.len() - 1
    })
}

/// A setup or payoff written in a text.
struct Marker {
    kind: Kind,
    name: String,
    aliases: Vec<String>,
    /// Bytes of the whole marker, aliases included.
    span: Range<usize>,
}

/// Letters and digits make words; everything else separates them.
fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

/// The words of `text`, lowercase, each with its byte offset.
fn words(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices().chain([(text.len(), ' ')]) {
        match (is_word(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, text[s..i].to_lowercase()));
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// What identifies a name or an alias: its lowercase words.
fn key(phrase: &str) -> Vec<String> {
    words(phrase).into_iter().map(|(_, w)| w).collect()
}

/// The length of the name that starts `s`: words joined by `-` or `_`.
fn name_length(s: &str) -> usize {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if is_word(c) {
            end = i + c.len_utf8();
        } else if !((c == '-' || c == '_') && i > 0 && end == i) {
            break;
        }
    }
    end
}

/// The setups and payoffs of `text`. A marker starts a word: `@` or `!`
/// right after a letter, as in an address or `Bang!`, is plain text.
fn markers(text: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut after_word = false;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let kind = match c {
            '@' => Some(Kind::Setup),
            '!' => Some(Kind::Payoff),
            _ => None,
        };
        let len = if kind.is_some() && !after_word { name_length(&text[i + 1..]) } else { 0 };
        if let Some(kind) = kind
            && len > 0
        {
            let start = i + 1;
            let mut end = start + len;
            let mut aliases = Vec::new();
            if kind == Kind::Setup
                && let Some(inner) = text[end..].strip_prefix('(')
                && let Some(close) = inner.find([')', '\n'])
                && inner[close..].starts_with(')')
            {
                aliases = inner[..close]
                    .split(',')
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                    .map(String::from)
                    .collect();
                end += close + 2;
            }
            out.push(Marker { kind, name: text[start..start + len].to_string(), aliases, span: i..end });
            while chars.next_if(|&(j, _)| j < end).is_some() {}
            after_word = text[..end].chars().next_back().is_some_and(is_word);
            continue;
        }
        after_word = is_word(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warnings(texts: &[&str]) -> Vec<(Rule, String, usize)> {
        let a = analyze(texts.iter().copied());
        a.warnings.iter().map(|w| (w.rule, a.elements[w.element].name.clone(), w.beat)).collect()
    }

    fn kinds(texts: &[&str], name: &str) -> Vec<(Kind, usize)> {
        let a = analyze(texts.iter().copied());
        let e = a.element(name).expect("element");
        a.elements[e].occurrences.iter().map(|o| (o.kind, o.at.0)).collect()
    }

    #[test]
    fn setup_then_payoff_is_clean() {
        let t = ["Un @pistolet dans le tiroir.", "Le pistolet brille.", "Elle tire : !pistolet."];
        assert!(warnings(&t).is_empty());
        assert_eq!(kinds(&t, "pistolet"), [(Kind::Setup, 0), (Kind::Mention, 1), (Kind::Payoff, 2)]);
    }

    #[test]
    fn setup_without_payoff_is_never_paid() {
        assert_eq!(warnings(&["Une @lettre.", "La lettre."]), [(Rule::NeverPaid, "lettre".into(), 0)]);
    }

    #[test]
    fn payoff_before_its_only_setup_is_used_before_declared_and_never_paid() {
        assert_eq!(
            warnings(&["!radio", "@radio"]),
            [(Rule::UsedBeforeDeclared, "radio".into(), 0), (Rule::NeverPaid, "radio".into(), 1)]
        );
    }

    #[test]
    fn mention_before_setup_is_used_before_declared() {
        assert_eq!(
            warnings(&["La radio grésille.", "Une @radio.", "!radio"]),
            [(Rule::UsedBeforeDeclared, "radio".into(), 0)]
        );
    }

    #[test]
    fn order_counts_within_a_beat() {
        assert_eq!(warnings(&["@x puis !x"]), []);
        assert_eq!(
            warnings(&["x puis @x et !x"]),
            [(Rule::UsedBeforeDeclared, "x".into(), 0)]
        );
    }

    #[test]
    fn payoff_without_setup_is_paid_undeclared() {
        assert_eq!(warnings(&["La !clé."]), [(Rule::PaidUndeclared, "clé".into(), 0)]);
    }

    #[test]
    fn second_setup_is_declared_twice() {
        assert_eq!(
            warnings(&["@chien", "@Chien", "!chien"]),
            [(Rule::DeclaredTwice, "chien".into(), 1)]
        );
    }

    #[test]
    fn aliases_resolve_payoffs_and_mentions() {
        let t = [
            "Un @pistolet(flingue, vieux revolver, arme) caché.",
            "Le FLINGUE, ce vieux revolver.",
            "Elle prend l'!arme.",
        ];
        assert!(warnings(&t).is_empty());
        assert_eq!(
            kinds(&t, "pistolet"),
            [(Kind::Setup, 0), (Kind::Mention, 1), (Kind::Mention, 1), (Kind::Payoff, 2)]
        );
        let a = analyze(t);
        assert_eq!(a.elements[0].aliases, ["flingue", "vieux revolver", "arme"]);
    }

    #[test]
    fn mentions_are_whole_words_with_accents_and_no_plurals() {
        let t = ["La @clé.", "Les clés, la cle, le porte-clé, CLÉ !clé"];
        assert_eq!(kinds(&t, "clé"), [(Kind::Setup, 0), (Kind::Mention, 1), (Kind::Mention, 1), (Kind::Payoff, 1)]);
    }

    #[test]
    fn markers_need_a_word_start() {
        assert!(analyze(["robin@geiger.ink", "Bang! Pan!", "@ seul, ! seul"]).elements.is_empty());
    }

    #[test]
    fn names_join_words_with_hyphens() {
        let t = ["Un @porte-clé.", "Le porte-clé tombe.", "!porte-clé"];
        assert!(warnings(&t).is_empty());
        assert_eq!(kinds(&t, "porte clé").len(), 3);
    }

    #[test]
    fn beat_lists_each_element_once_per_kind_in_text_order() {
        let a = analyze(["La @lettre, le @pistolet, la lettre, la lettre."]);
        let names: Vec<_> = a.in_beat(0).iter().map(|&(e, k)| (a.elements[e].name.clone(), k)).collect();
        assert_eq!(
            names,
            [("lettre".into(), Kind::Setup), ("pistolet".into(), Kind::Setup), ("lettre".into(), Kind::Mention)]
        );
    }
}
