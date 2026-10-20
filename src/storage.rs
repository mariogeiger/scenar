//! The browser's side of saving: the draft and the copies of shared links
//! in localStorage, the links this browser opened, the name and colour it
//! shows the others, the JSON file a story is exported to and imported
//! from, and the theme.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use scenar::model::Story;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};

const STORY_KEY: &str = "scenar.story";
const THEME_KEY: &str = "scenar.theme";
const LINKS_KEY: &str = "scenar.links";
const NAME_KEY: &str = "scenar.name";
const COLOR_KEY: &str = "scenar.color";

/// The colours a reader's cursor may take: distinct, and none the red of
/// warnings.
const COLORS: [&str; 8] = ["#2f64b0", "#2e8b57", "#d2691e", "#7b4fb3", "#00838f", "#8d6e63", "#c2185b", "#558b2f"];

fn window() -> web_sys::Window {
    web_sys::window().expect("a browser window")
}

fn local() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// The story saved in this browser, if a readable one is there.
pub fn load() -> Option<Story> {
    Story::from_json(&local()?.get_item(STORY_KEY).ok()??).ok()
}

pub fn save(story: &Story) {
    if let Some(s) = local() {
        let _ = s.set_item(STORY_KEY, &story.to_json());
    }
}

fn get(key: &str) -> Option<String> {
    local()?.get_item(key).ok().flatten()
}

fn set(key: &str, value: &str) {
    if let Some(s) = local() {
        let _ = s.set_item(key, value);
    }
}

/// This browser's copy of the shared story `id`, as the update that
/// recreates it.
pub fn copy_of(id: &str) -> Option<Vec<u8>> {
    STANDARD.decode(get(&format!("scenar.link.{id}"))?).ok()
}

pub fn keep_copy(id: &str, update: &[u8]) {
    set(&format!("scenar.link.{id}"), &STANDARD.encode(update));
}

/// A shared link this browser opened, with the title its story had.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub id: String,
    pub title: String,
}

/// The links this browser opened, the latest first.
pub fn links() -> Vec<Link> {
    get(LINKS_KEY).and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

/// Records that the link `id` holds a story titled `title`, keeping its
/// place in the list unless it is new.
pub fn remember(id: &str, title: &str) {
    let mut links = links();
    match links.iter_mut().find(|l| l.id == id) {
        Some(l) if l.title == title => return,
        Some(l) => l.title = title.to_string(),
        None => links.insert(0, Link { id: id.to_string(), title: title.to_string() }),
    }
    set(LINKS_KEY, &serde_json::to_string(&links).expect("links serialize"));
}

/// The name this browser shows the others, empty when none was given.
pub fn name() -> String {
    get(NAME_KEY).unwrap_or_default()
}

pub fn set_name(name: &str) {
    set(NAME_KEY, name);
}

/// The colour of this browser's cursor, drawn once.
pub fn color() -> String {
    get(COLOR_KEY).unwrap_or_else(|| {
        let color = COLORS[fastrand::usize(..COLORS.len())].to_string();
        set(COLOR_KEY, &color);
        color
    })
}

/// The path of the page's address, such as `/` or `/k3PqZ8mWv2LxR7aT`.
pub fn path() -> String {
    window().location().pathname().unwrap_or_default()
}

/// The address of the page's server, such as `https://scenar.geiger.ink`.
pub fn origin() -> String {
    window().location().origin().unwrap_or_default()
}

/// Opens another page of the site.
pub fn go(path: &str) {
    let _ = window().location().assign(path);
}

/// Whether the dark theme was chosen, or else is the system's.
pub fn dark() -> bool {
    match local().and_then(|s| s.get_item(THEME_KEY).ok().flatten()).as_deref() {
        Some(theme) => theme == "dark",
        None => window().match_media("(prefers-color-scheme: dark)").ok().flatten().is_some_and(|m| m.matches()),
    }
}

pub fn set_dark(dark: bool) {
    if let Some(s) = local() {
        let _ = s.set_item(THEME_KEY, if dark { "dark" } else { "light" });
    }
    if let Some(root) = window().document().and_then(|d| d.document_element()) {
        let _ = root.class_list().toggle_with_force("dark", dark);
    }
}

/// Downloads the story as `<title>.scenar.json`.
pub fn export(story: &Story) -> Result<(), JsValue> {
    let parts = blob_parts(&story.to_json());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let a: web_sys::HtmlAnchorElement = window().document().expect("a document").create_element("a")?.dyn_into()?;
    a.set_href(&url);
    a.set_download(&format!("{}.scenar.json", file_stem(&story.title)));
    a.click();
    web_sys::Url::revoke_object_url(&url)
}

/// The parts of a blob holding `text` alone.
fn blob_parts(text: &str) -> JsValue {
    let array = js_sys::Array::new();
    array.push(&JsValue::from_str(text));
    array.into()
}

/// A title as a file name: its words in lowercase, joined by hyphens.
fn file_stem(title: &str) -> String {
    let stem: Vec<String> =
        title.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    if stem.is_empty() { "scenario".into() } else { stem.join("-") }
}

/// The text of the file picked in `input`.
pub async fn picked_text(input: &web_sys::HtmlInputElement) -> Option<String> {
    let file = input.files()?.get(0)?;
    let text = wasm_bindgen_futures::JsFuture::from(file.text()).await.ok()?;
    text.as_string()
}

pub fn confirm(message: &str) -> bool {
    window().confirm_with_message(message).unwrap_or(false)
}

pub fn alert(message: &str) {
    let _ = window().alert_with_message(message);
}

/// The id of the element that shows the block `id` in the script.
pub fn dom_id(id: u64) -> String {
    format!("block-{id}")
}

/// Scrolls the script to the block `id`: to its start, or for a beat to the
/// middle of the view, across pages as well as down.
pub fn reveal(id: u64) {
    let Some(el) = window().document().and_then(|d| d.get_element_by_id(&dom_id(id))) else {
        return;
    };
    let options = web_sys::ScrollIntoViewOptions::new();
    options.set_behavior(web_sys::ScrollBehavior::Smooth);
    let beat = el.class_list().contains("beat");
    options.set_block(if beat {
        web_sys::ScrollLogicalPosition::Center
    } else {
        web_sys::ScrollLogicalPosition::Start
    });
    options.set_inline(web_sys::ScrollLogicalPosition::Nearest);
    el.scroll_into_view_with_scroll_into_view_options(&options);
}

/// The width and height of the window.
pub fn viewport() -> (f64, f64) {
    let w = window();
    let size = |v: Result<JsValue, JsValue>| v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    (size(w.inner_width()), size(w.inner_height()))
}
