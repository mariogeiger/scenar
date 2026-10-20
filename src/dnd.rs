//! Drag and drop of blocks, shared by the rows of the timeline and the beats:
//! a block lands before or after one of its level, or into one a level up.

use leptos::prelude::*;
use scenar::model::Side;

use crate::state::State;
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
