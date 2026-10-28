//! scenar-server: the shared stories of scenar.geiger.ink.
//!
//! `POST /api/stories` keeps a new story, sent as a Yrs update, under a
//! fresh random id and answers that id. `GET /api/stories/{id}` says whether
//! it exists. `GET /api/stories/{id}/live` is a WebSocket on which the
//! browsers open on a story keep it in sync, by the Yjs sync protocol (see
//! [`room`]). Limits: 2 MiB a story, 20 new stories an hour from one
//! address; nothing expires.
//!
//! It serves the listening socket systemd passes it (socket activation),
//! else `127.0.0.1:SCENAR_PORT`. Environment: `SCENAR_DATA`, the directory of
//! the stories, and `SCENAR_PORT`, 4010 by default.

mod room;

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

use room::Rooms;

/// The most a story may weigh, as the update that recreates it.
pub const MAX_BYTES: usize = 2 << 20;
/// New stories from one address within [`WINDOW`].
const MAX_NEW: usize = 20;
const WINDOW: Duration = Duration::from_secs(3600);
/// The characters of an id, and how many it has: 16 of 62, about 95 bits.
const ALPHABET: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const ID_LEN: usize = 16;

#[derive(Clone)]
struct App {
    rooms: Rooms,
    data: Arc<PathBuf>,
    created: Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>,
}

#[tokio::main]
async fn main() {
    let data = PathBuf::from(std::env::var("SCENAR_DATA").unwrap_or_else(|_| "data".into()));
    std::fs::create_dir_all(&data).expect("the data directory can be made");
    let port: u16 = std::env::var("SCENAR_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(4010);
    let app = App { rooms: Rooms::new(data.clone()), data: Arc::new(data), created: Default::default() };
    tokio::spawn(app.rooms.clone().keep());
    let router = Router::new()
        .route("/api/stories", post(create))
        .route("/api/stories/{id}", get(exists))
        .route("/api/stories/{id}/live", get(live))
        .layer(DefaultBodyLimit::max(MAX_BYTES))
        .with_state(app.clone());
    println!("scenar-server: stories in {}", app.data.display());
    match inherited_listener() {
        Some(socket) => {
            socket.set_nonblocking(true).expect("the socket is usable");
            let listener = tokio::net::UnixListener::from_std(socket).expect("the socket is usable");
            println!("scenar-server on the socket systemd passed");
            axum::serve(listener, router).with_graceful_shutdown(stopped()).await
        }
        None => {
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.expect("the port is free");
            println!("scenar-server on 127.0.0.1:{port}");
            axum::serve(listener, router).with_graceful_shutdown(stopped()).await
        }
    }
    .expect("the server runs");
    app.rooms.flush_all().await;
}

/// The one listening socket systemd passes this process, by the protocol of
/// `sd_listen_fds`: `LISTEN_PID` names it, `LISTEN_FDS` counts the sockets,
/// which begin at descriptor 3. It comes back as a duplicate that closes on
/// exec, and descriptor 3 is closed, so no child could inherit the listener.
fn inherited_listener() -> Option<UnixListener> {
    let ours = std::env::var("LISTEN_PID").ok()?.parse::<u32>().ok()? == std::process::id();
    let one = std::env::var("LISTEN_FDS").ok()? == "1";
    if !(ours && one) {
        return None;
    }
    // SAFETY: by the protocol, descriptor 3 is a listening socket this process owns.
    let passed = unsafe { OwnedFd::from_raw_fd(3) };
    Some(UnixListener::from(passed.try_clone().expect("the socket can be duplicated")))
}

/// Resolves when the process is asked to stop: SIGINT, or SIGTERM, which
/// systemd and Podman send.
async fn stopped() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM can be awaited");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}

/// A fresh id, from the system's randomness, each character drawn evenly.
fn fresh_id() -> String {
    let mut id = String::with_capacity(ID_LEN);
    while id.len() < ID_LEN {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("the system has randomness");
        // 248 = 4 × 62: bytes below it map evenly onto the alphabet.
        for b in bytes.into_iter().filter(|&b| b < 248).take(ID_LEN - id.len()) {
            id.push(ALPHABET[(b % 62) as usize] as char);
        }
    }
    id
}

pub fn valid_id(id: &str) -> bool {
    id.len() == ID_LEN && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The address a request comes from, as Caddy forwards it. Only Caddy, or
/// a local client in development, reaches the server; without the header the
/// request is local.
fn client(headers: &HeaderMap) -> IpAddr {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

async fn create(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    if body.len() > MAX_BYTES {
        return (StatusCode::PAYLOAD_TOO_LARGE, "story too large").into_response();
    }
    {
        let now = Instant::now();
        let mut created = app.created.lock().expect("not poisoned");
        created.retain(|_, times| {
            times.retain(|t| now.duration_since(*t) < WINDOW);
            !times.is_empty()
        });
        let times = created.entry(client(&headers)).or_default();
        if times.len() >= MAX_NEW {
            return (StatusCode::TOO_MANY_REQUESTS, "too many new stories this hour").into_response();
        }
        times.push_back(now);
    }
    let id = loop {
        let id = fresh_id();
        if !app.data.join(format!("{id}.ydoc")).exists() {
            break id;
        }
    };
    match room::create(&app.data, &id, &body).await {
        Ok(()) => (StatusCode::CREATED, id).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn exists(State(app): State<App>, Path(id): Path<String>) -> StatusCode {
    if valid_id(&id) && app.data.join(format!("{id}.ydoc")).exists() {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn live(State(app): State<App>, Path(id): Path<String>, ws: WebSocketUpgrade) -> Response {
    let Some(room) = (if valid_id(&id) { app.rooms.open(&id).await } else { None }) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    ws.max_message_size(MAX_BYTES + 1024).on_upgrade(move |socket| room::serve(room, socket))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_sixteen_alphanumerics_and_differ() {
        let (a, b) = (fresh_id(), fresh_id());
        assert!(valid_id(&a) && valid_id(&b) && a != b);
        assert!(!valid_id("../../etc/passwd") && !valid_id("abc"));
    }
}
