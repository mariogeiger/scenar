//! Text fields bound to a text of the shared story. What the reader types
//! goes to the document as the edit it is, against the text as they saw
//! it; what the others type comes in without moving the reader's caret or
//! selection.

use leptos::prelude::*;
use scenar::shared::{Field, Splice};
use wasm_bindgen::JsCast;

use crate::link::{byte_of, unit_of};
use crate::state::State;

/// What a text input and a text area share.
trait Editable: JsCast + Clone {
    fn text(&self) -> String;
    fn set_text(&self, text: &str);
    fn selection(&self) -> Option<(u32, u32)>;
    fn select(&self, start: u32, end: u32);
}

macro_rules! editable {
    ($t:ty) => {
        impl Editable for $t {
            fn text(&self) -> String {
                self.value()
            }
            fn set_text(&self, text: &str) {
                self.set_value(text)
            }
            fn selection(&self) -> Option<(u32, u32)> {
                Some((self.selection_start().ok()??, self.selection_end().ok()??))
            }
            fn select(&self, start: u32, end: u32) {
                _ = self.set_selection_range(start, end);
            }
        }
    };
}
editable!(web_sys::HtmlTextAreaElement);
editable!(web_sys::HtmlInputElement);

/// The text `field` stands for, as the story holds it now.
pub fn text_of(st: State, field: Field) -> String {
    st.story.with(|s| match field {
        Field::StoryTitle => s.title.clone(),
        Field::Title(id) => s.find(id).map(|n| n.title.clone()).unwrap_or_default(),
        Field::Text(id) => s.find(id).map(|n| n.text.clone()).unwrap_or_default(),
    })
}

/// Shows `text` in `el`; when the reader is in it, their selection keeps
/// to the same letters.
fn show<E: Editable>(el: &E, text: &str) {
    let old = el.text();
    if old == text {
        return;
    }
    let focused = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        .is_some_and(|a| a.is_same_node(Some(el.unchecked_ref())));
    let selection = el.selection().filter(|_| focused);
    el.set_text(text);
    if let Some((start, end)) = selection {
        let change = Splice::between(&old, text);
        let keep = |unit: u32| unit_of(text, change.map(byte_of(&old, unit)));
        el.select(keep(start), keep(end));
    }
}

/// Binds `el`, once mounted, to `field`: returns what to do on input.
fn bind<E: Editable + 'static>(
    st: State,
    field: Field,
    node: impl Fn() -> Option<E> + Copy + 'static,
) -> impl Fn() + Copy {
    let seen = StoredValue::new(String::new());
    Effect::new(move |_| {
        let text = text_of(st, field);
        if let Some(el) = node() {
            show(&el, &text);
        }
        seen.set_value(text);
    });
    move || {
        let Some(el) = node() else { return };
        let typed = el.text();
        let old = seen.get_value();
        let edit = Splice::between(&old, &typed);
        if !edit.is_empty() {
            seen.set_value(typed);
            st.splice(field, &old, &edit);
        }
    }
}

/// A one-line field for a title.
pub fn line(st: State, field: Field, class: &'static str, label: &'static str) -> impl IntoView {
    let node = NodeRef::<leptos::html::Input>::new();
    let typed = bind(st, field, move || node.get());
    view! { <input class=class aria-label=label node_ref=node on:input=move |_| typed()/> }
}

/// The text area of a beat.
pub fn area(st: State, id: u64) -> impl IntoView {
    let node = NodeRef::<leptos::html::Textarea>::new();
    let typed = bind(st, Field::Text(id), move || node.get());
    view! {
        <textarea
            rows="1"
            aria-label="Texte du beat"
            placeholder="Écris le beat. @nom déclare un élément, !nom le paie."
            data-beat=id.to_string()
            node_ref=node
            on:input=move |_| typed()
        ></textarea>
    }
}
