//! A shared link's life in the browser: creating it, keeping its story in
//! sync with the server over a WebSocket, and knowing who else is on it and
//! where their cursor stands.
//!
//! The wire speaks the Yjs sync protocol (see the server's `room`): on
//! opening, each side sends its state vector and answers the other's with
//! what it lacks, so that edits made offline go out on reconnecting; then
//! every change goes out as an update. Presence travels as awareness: per
//! browser a clock and a JSON state, `null` once gone.

use std::cell::RefCell;
use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use leptos::prelude::*;
use scenar::shared::Field;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use yrs::sync::awareness::AwarenessUpdateEntry;
use yrs::sync::{AwarenessUpdate, Message, SyncMessage};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ClientID, ReadTxn, Transact};

use crate::state::State;
use crate::storage;

/// Where the link stands with the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sync {
    Connecting,
    Online,
    Offline,
    /// The story reached the size limit; the server takes no more.
    TooLarge,
    /// No story has this link.
    Missing,
}

/// Someone else on the link.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub name: String,
    pub color: String,
    /// The beat their cursor is in, and the byte of its text it stands at.
    pub cursor: Option<(u64, usize)>,
}

/// What a browser tells the others of itself.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Presence {
    name: String,
    color: String,
    /// The beat of the cursor, and an anchor in its text, in base64.
    cursor: Option<(u64, String)>,
}

/// The message the server sends before closing a story that is full.
const TOO_LARGE: u8 = 1;

struct Session {
    id: &'static str,
    socket: Option<web_sys::WebSocket>,
    me: ClientID,
    clock: u32,
    presence: Presence,
    others: HashMap<ClientID, (u32, Presence)>,
    /// Seconds to wait before reconnecting, doubled up to a minute.
    backoff: u32,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Session) -> R) -> Option<R> {
    SESSION.with(|s| s.borrow_mut().as_mut().map(f))
}

/// The id a path names, if it names one: `/` followed by 16 letters and
/// digits.
pub fn id_in(path: &str) -> Option<String> {
    let id = path.strip_prefix('/')?.trim_end_matches('/');
    (id.len() == 16 && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| id.to_string())
}

/// Keeps a new story on the server and answers its id.
pub async fn create(update: Vec<u8>) -> Result<String, String> {
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_body(&js_sys::Uint8Array::from(&update[..]));
    let request = web_sys::Request::new_with_str_and_init("/api/stories", &init).map_err(|_| "requête impossible")?;
    let response = fetch(&request).await.ok_or("le serveur ne répond pas")?;
    let text = wasm_bindgen_futures::JsFuture::from(response.text().map_err(|_| "réponse illisible")?)
        .await
        .ok()
        .and_then(|t| t.as_string())
        .unwrap_or_default();
    match response.status() {
        201 => Ok(text),
        413 => Err("le scénario dépasse 2 Mo".into()),
        429 => Err("trop de liens créés depuis cette adresse cette heure-ci ; réessaie plus tard".into()),
        s => Err(format!("le serveur a refusé ({s})")),
    }
}

async fn fetch(request: &web_sys::Request) -> Option<web_sys::Response> {
    let window = web_sys::window()?;
    let response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(request)).await.ok()?;
    response.dyn_into().ok()
}

/// Opens the link `id`: makes sure its story exists, then keeps it in sync.
pub fn open(st: State, id: &'static str) {
    let me = st.shared.with_value(|s| s.doc().client_id());
    let presence = Presence { name: storage::name(), color: storage::color(), cursor: None };
    SESSION.with(|s| {
        *s.borrow_mut() = Some(Session { id, socket: None, me, clock: 0, presence, others: HashMap::new(), backoff: 1 })
    });
    leptos::task::spawn_local(async move {
        let request = web_sys::Request::new_with_str(&format!("/api/stories/{id}")).expect("a request");
        match fetch(&request).await.map(|r| r.status()) {
            Some(404) => st.sync.set(Sync::Missing),
            _ => connect(st),
        }
    });
    // The cursor follows the selection in whichever beat holds it. A text
    // area's selection changes are its own events, caught on their way
    // down; leaving it is seen once the focus has gone elsewhere.
    let follow = Closure::<dyn Fn()>::new(move || cursor_moved(st));
    let left = Closure::<dyn Fn()>::new(move || set_timeout(move || cursor_moved(st), std::time::Duration::ZERO));
    let document = web_sys::window().and_then(|w| w.document()).expect("a document");
    _ = document.add_event_listener_with_callback_and_bool("selectionchange", follow.as_ref().unchecked_ref(), true);
    _ = document.add_event_listener_with_callback_and_bool("focusout", left.as_ref().unchecked_ref(), true);
    follow.forget();
    left.forget();
}

fn connect(st: State) {
    let Some(id) = with(|s| s.id) else { return };
    let url = format!("{}/api/stories/{id}/live", storage::origin().replacen("http", "ws", 1));
    let Ok(socket) = web_sys::WebSocket::new(&url) else {
        retry(st);
        return;
    };
    socket.set_binary_type(web_sys::BinaryType::Arraybuffer);
    st.sync.set(Sync::Connecting);
    let opened = Closure::<dyn Fn()>::new(move || {
        st.sync.set(Sync::Online);
        with(|s| s.backoff = 1);
        let sv = st.shared.with_value(|s| s.doc().transact().state_vector());
        send(&Message::Sync(SyncMessage::SyncStep1(sv)).encode_v1());
        announce();
    });
    let received = Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
        if let Ok(buffer) = ev.data().dyn_into::<js_sys::ArrayBuffer>() {
            receive(st, &js_sys::Uint8Array::new(&buffer).to_vec());
        }
    });
    let closed = Closure::<dyn Fn()>::new(move || {
        with(|s| s.socket = None);
        if !matches!(st.sync.get_untracked(), Sync::TooLarge | Sync::Missing) {
            st.sync.set(Sync::Offline);
            retry(st);
        }
        with(|s| s.others.clear());
        refresh(st);
    });
    socket.set_onopen(Some(opened.as_ref().unchecked_ref()));
    socket.set_onmessage(Some(received.as_ref().unchecked_ref()));
    socket.set_onclose(Some(closed.as_ref().unchecked_ref()));
    opened.forget();
    received.forget();
    closed.forget();
    with(|s| s.socket = Some(socket));
}

fn retry(st: State) {
    let wait = with(|s| {
        let wait = s.backoff;
        s.backoff = (s.backoff * 2).min(60);
        wait
    })
    .unwrap_or(1);
    set_timeout(move || connect(st), std::time::Duration::from_secs(wait as u64));
}

fn send(bytes: &[u8]) {
    with(|s| {
        if let Some(socket) = s.socket.as_ref().filter(|w| w.ready_state() == web_sys::WebSocket::OPEN) {
            _ = socket.send_with_u8_array(bytes);
        }
    });
}

/// Sends a change made here; offline, it waits in the document for the
/// next sync.
pub fn send_update(update: Vec<u8>) {
    send(&Message::Sync(SyncMessage::Update(update)).encode_v1());
}

fn receive(st: State, bytes: &[u8]) {
    let Ok(message) = Message::decode_v1(bytes) else { return };
    match message {
        Message::Sync(SyncMessage::SyncStep1(sv)) => {
            let diff = st.shared.with_value(|s| s.doc().transact().encode_state_as_update_v1(&sv));
            send(&Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1());
        }
        Message::Sync(SyncMessage::SyncStep2(update) | SyncMessage::Update(update)) => st.receive(&update),
        Message::Awareness(update) => {
            with(|s| {
                for (client, entry) in update.clients {
                    if client == s.me {
                        continue;
                    }
                    let newer = s.others.get(&client).is_none_or(|(clock, _)| entry.clock >= *clock);
                    match serde_json::from_str::<Option<Presence>>(&entry.json) {
                        Ok(Some(p)) if newer => _ = s.others.insert(client, (entry.clock, p)),
                        Ok(None) => _ = s.others.remove(&client),
                        _ => {}
                    }
                }
            });
            refresh(st);
        }
        Message::Custom(TOO_LARGE, _) => st.sync.set(Sync::TooLarge),
        _ => {}
    }
}

/// Tells the others what this browser is now.
fn announce() {
    let message = with(|s| {
        s.clock += 1;
        let json = serde_json::to_string(&s.presence).expect("presence serializes");
        let entry = AwarenessUpdateEntry { clock: s.clock, json: json.into() };
        Message::Awareness(AwarenessUpdate { clients: HashMap::from([(s.me, entry)]) }).encode_v1()
    });
    if let Some(m) = message {
        send(&m);
    }
}

pub fn set_name(name: String) {
    storage::set_name(&name);
    if with(|s| s.presence.name = name).is_some() {
        announce();
    }
}

/// Moves this browser's cursor to where the selection now ends, when it is
/// in a beat, or takes it away.
fn cursor_moved(st: State) {
    let document = web_sys::window().and_then(|w| w.document()).expect("a document");
    let at =
        document.active_element().and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok()).and_then(|el| {
            let beat: u64 = el.get_attribute("data-beat")?.parse().ok()?;
            let end = el.selection_end().ok().flatten()?;
            Some((beat, byte_of(&el.value(), end)))
        });
    let cursor = at.and_then(|(beat, byte)| {
        let anchor = st.shared.with_value(|s| s.anchor(Field::Text(beat), byte))?;
        Some((beat, STANDARD.encode(anchor)))
    });
    let changed = with(|s| {
        let changed = s.presence.cursor != cursor;
        s.presence.cursor = cursor;
        changed
    });
    if changed == Some(true) {
        announce();
    }
}

/// Recomputes who is there and where their cursors stand, after they moved
/// or the text did.
pub fn refresh(st: State) {
    let peers = with(|s| {
        let mut others: Vec<_> = s.others.iter().collect();
        others.sort_by_key(|(c, _)| c.get());
        others
            .into_iter()
            .map(|(_, (_, p))| Peer {
                name: if p.name.trim().is_empty() { "Invité".into() } else { p.name.clone() },
                color: p.color.clone(),
                cursor: p.cursor.as_ref().and_then(|(beat, anchor)| {
                    let anchor = STANDARD.decode(anchor).ok()?;
                    Some((*beat, st.shared.with_value(|s| s.resolve(Field::Text(*beat), &anchor))?))
                }),
            })
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    if st.peers.with_untracked(|p| *p != peers) {
        st.peers.set(peers);
    }
}

/// The byte of `text` that its UTF-16 unit `unit` falls on, as the browser
/// counts a selection.
pub fn byte_of(text: &str, unit: u32) -> usize {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        if units >= unit as usize {
            return i;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// The UTF-16 unit that the byte `byte` of `text` falls on.
pub fn unit_of(text: &str, byte: usize) -> u32 {
    text[..byte.min(text.len())].encode_utf16().count() as u32
}

/// Copies the story to this browser, so that the link opens at once next
/// time, offline too.
pub fn keep(st: State) {
    if let Some(id) = with(|s| s.id) {
        storage::keep_copy(id, &st.shared.with_value(|s| s.encode()));
    }
}
