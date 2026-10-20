//! The rooms: one per story open in some browser, holding its document, the
//! awareness of who is there and where their cursor is, and the browsers
//! connected. A change one browser sends is merged, passed on to the others
//! and written to disk within a second.
//!
//! The wire speaks the Yjs sync protocol, as Yrs encodes it: each side sends
//! its state vector (step 1), answers the other's with what it lacks (step
//! 2), then sends each change as an update. Awareness messages carry, per
//! client, a clock and a JSON state; a state of `null` says it left.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message as Frame, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{Mutex, mpsc};
use yrs::sync::{AwarenessUpdate, Message, SyncMessage};
use yrs::sync::awareness::AwarenessUpdateEntry;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{ClientID, Doc, ReadTxn, StateVector, Transact, Update};

use crate::MAX_BYTES;

/// What a room tells a browser before closing it: the story is full.
const TOO_LARGE: u8 = 1;

pub struct Room {
    id: String,
    doc: Doc,
    /// The latest awareness entry of each client present.
    present: HashMap<ClientID, AwarenessUpdateEntry>,
    /// The browsers connected, each with the clients it spoke for.
    peers: HashMap<u64, (mpsc::UnboundedSender<Vec<u8>>, HashSet<ClientID>)>,
    next_peer: u64,
    /// Whether it changed since last written, and its size then plus what
    /// came since: an upper bound of its weight.
    dirty: bool,
    weight: usize,
    /// Since when nobody is in it; it closes a while after.
    idle_since: Option<Instant>,
}

/// How long an empty room stays open, so that a browser on its way in or
/// reconnecting finds it.
const LINGER: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Rooms {
    data: Arc<PathBuf>,
    open: Arc<Mutex<HashMap<String, Arc<Mutex<Room>>>>>,
}

fn file(data: &Path, id: &str) -> PathBuf {
    data.join(format!("{id}.ydoc"))
}

/// Writes `bytes` to `path` whole or not at all.
async fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await
}

/// Keeps a new story, after checking it is one.
pub async fn create(data: &Path, id: &str, update: &[u8]) -> Result<(), String> {
    let doc = Doc::new();
    let update = Update::decode_v1(update).map_err(|e| format!("not a story: {e}"))?;
    doc.transact_mut().apply_update(update).map_err(|e| format!("not a story: {e}"))?;
    let bytes = doc.transact().encode_state_as_update_v1(&StateVector::default());
    write_atomically(&file(data, id), &bytes).await.map_err(|e| e.to_string())
}

impl Rooms {
    pub fn new(data: PathBuf) -> Rooms {
        Rooms { data: Arc::new(data), open: Default::default() }
    }

    /// The room of the story `id`, opened from disk if need be; `None` when
    /// there is no such story.
    pub async fn open(&self, id: &str) -> Option<Arc<Mutex<Room>>> {
        let mut open = self.open.lock().await;
        if let Some(room) = open.get(id) {
            return Some(room.clone());
        }
        let bytes = tokio::fs::read(file(&self.data, id)).await.ok()?;
        let doc = Doc::new();
        doc.transact_mut().apply_update(Update::decode_v1(&bytes).ok()?).ok()?;
        let room = Arc::new(Mutex::new(Room {
            id: id.to_string(),
            doc,
            present: HashMap::new(),
            peers: HashMap::new(),
            next_peer: 0,
            dirty: false,
            weight: bytes.len(),
            idle_since: Some(Instant::now()),
        }));
        open.insert(id.to_string(), room.clone());
        Some(room)
    }

    /// Writes changed stories every second, and closes rooms nobody is in.
    pub async fn keep(self) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            self.flush(false).await;
        }
    }

    pub async fn flush_all(&self) {
        self.flush(true).await;
    }

    async fn flush(&self, all: bool) {
        let rooms: Vec<(String, Arc<Mutex<Room>>)> =
            self.open.lock().await.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (id, room) in rooms {
            let mut r = room.lock().await;
            if r.dirty || all {
                let bytes = r.doc.transact().encode_state_as_update_v1(&StateVector::default());
                match write_atomically(&file(&self.data, &r.id), &bytes).await {
                    Ok(()) => {
                        r.dirty = false;
                        r.weight = bytes.len();
                    }
                    Err(e) => eprintln!("cannot write story {id}: {e}"),
                }
            }
            let idle = |r: &Room| !r.dirty && r.idle_since.is_some_and(|t| t.elapsed() > LINGER);
            if idle(&r) {
                drop(r);
                let mut open = self.open.lock().await;
                // Someone may have come in meanwhile.
                if open.get(&id).is_some_and(|room| room.try_lock().is_ok_and(|r| idle(&r))) {
                    open.remove(&id);
                }
            }
        }
    }
}

impl Room {
    fn send_others(&self, from: u64, bytes: &[u8]) {
        for (&peer, (tx, _)) in &self.peers {
            if peer != from {
                _ = tx.send(bytes.to_vec());
            }
        }
    }

    fn presence(&self) -> Option<Vec<u8>> {
        (!self.present.is_empty())
            .then(|| Message::Awareness(AwarenessUpdate { clients: self.present.clone() }).encode_v1())
    }

    /// Handles one message of the browser `from`, answering on `reply`.
    /// Returns false when the browser must be closed.
    fn receive(&mut self, from: u64, message: Message, reply: &mpsc::UnboundedSender<Vec<u8>>) -> bool {
        match message {
            Message::Sync(SyncMessage::SyncStep1(sv)) => {
                let diff = self.doc.transact().encode_state_as_update_v1(&sv);
                _ = reply.send(Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1());
            }
            Message::Sync(SyncMessage::SyncStep2(update) | SyncMessage::Update(update)) => {
                if self.weight + update.len() > MAX_BYTES {
                    _ = reply.send(Message::Custom(TOO_LARGE, Vec::new()).encode_v1());
                    return false;
                }
                let Ok(decoded) = Update::decode_v1(&update) else { return false };
                if self.doc.transact_mut().apply_update(decoded).is_err() {
                    return false;
                }
                self.weight += update.len();
                self.dirty = true;
                self.send_others(from, &Message::Sync(SyncMessage::Update(update)).encode_v1());
            }
            Message::Awareness(update) => {
                let mine = &mut self.peers.get_mut(&from).expect("a connected peer").1;
                for (&client, entry) in &update.clients {
                    mine.insert(client);
                    if &*entry.json == "null" {
                        self.present.remove(&client);
                    } else {
                        self.present.insert(client, entry.clone());
                    }
                }
                self.send_others(from, &Message::Awareness(update).encode_v1());
            }
            Message::AwarenessQuery => {
                if let Some(p) = self.presence() {
                    _ = reply.send(p);
                }
            }
            Message::Auth(_) | Message::Custom(..) => {}
        }
        true
    }

    /// Lets the others know that the clients of `peer` left.
    fn leave(&mut self, peer: u64) {
        let Some((_, clients)) = self.peers.remove(&peer) else { return };
        if self.peers.is_empty() {
            self.idle_since = Some(Instant::now());
        }
        let gone: HashMap<ClientID, AwarenessUpdateEntry> = clients
            .into_iter()
            .filter_map(|c| {
                let last = self.present.remove(&c)?;
                Some((c, AwarenessUpdateEntry { clock: last.clock + 1, json: "null".into() }))
            })
            .collect();
        if !gone.is_empty() {
            self.send_others(peer, &Message::Awareness(AwarenessUpdate { clients: gone }).encode_v1());
        }
    }
}

/// Serves one browser in `room` until it leaves.
pub async fn serve(room: Arc<Mutex<Room>>, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let me = {
        let mut r = room.lock().await;
        let me = r.next_peer;
        r.next_peer += 1;
        r.peers.insert(me, (tx.clone(), HashSet::new()));
        r.idle_since = None;
        let sv = r.doc.transact().state_vector();
        _ = tx.send(Message::Sync(SyncMessage::SyncStep1(sv)).encode_v1());
        if let Some(p) = r.presence() {
            _ = tx.send(p);
        }
        me
    };
    let writer = tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            if sink.send(Frame::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
        _ = sink.close().await;
    });
    while let Some(Ok(frame)) = stream.next().await {
        let bytes = match frame {
            Frame::Binary(b) => b,
            Frame::Close(_) => break,
            _ => continue,
        };
        let Ok(message) = Message::decode_v1(&bytes) else { break };
        if !room.lock().await.receive(me, message, &tx) {
            break;
        }
    }
    room.lock().await.leave(me);
    drop(tx);
    _ = writer.await;
}
