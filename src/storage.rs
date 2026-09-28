//! ハイスコアの保存 (Web では localStorage、ネイティブでは保存しない)。

const KEY: &str = "door-defense-3d.hi-score";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn load_hi_score() -> u32 {
    storage()
        .and_then(|s| s.get_item(KEY).ok()?)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
pub fn save_hi_score(score: u32) {
    if let Some(s) = storage() {
        let _ = s.set_item(KEY, &score.to_string());
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_hi_score() -> u32 {
    let _ = KEY;
    0
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_hi_score(_score: u32) {}
