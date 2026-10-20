//! The browser's side of saving: the automatic copy in localStorage, the
//! JSON file a story is exported to and imported from, and the theme.

use scenar::model::Story;
use wasm_bindgen::{JsCast, JsValue};

const STORY_KEY: &str = "scenar.story";
const THEME_KEY: &str = "scenar.theme";

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

/// Whether the dark theme was chosen, or else is the system's.
pub fn dark() -> bool {
    match local().and_then(|s| s.get_item(THEME_KEY).ok().flatten()).as_deref() {
        Some(theme) => theme == "dark",
        None => window()
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches()),
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
    let stem: Vec<String> = title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
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

/// Brings the beat `id` of the scene on screen into view.
pub fn reveal(id: u64) {
    if let Some(el) = window().document().and_then(|d| d.get_element_by_id(&format!("beat-{id}"))) {
        el.scroll_into_view();
    }
}
