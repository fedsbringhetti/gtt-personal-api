//! The web app built in `web/dist/`, embedded in the binary and served for every path that is
//! not an API route.

use std::fmt;

use axum::{
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::{EmbeddedFile, RustEmbed};

/// `npm run build` output. Release builds include the files; debug builds read them from disk
/// at every request, so a frontend rebuild needs no Rust rebuild. `allow_missing` keeps the
/// Rust build working before the frontend has ever been built.
#[derive(RustEmbed)]
#[folder = "web/dist"]
#[allow_missing = true]
struct Dist;

/// Where the files come from: the real build, or a fixture folder in tests.
#[derive(Clone, Copy)]
pub struct WebAssets {
    get: fn(&str) -> Option<EmbeddedFile>,
}

impl fmt::Debug for WebAssets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebAssets")
    }
}

impl WebAssets {
    /// The web app built in `web/dist/`.
    pub fn dist() -> Self {
        Self::from_embed::<Dist>()
    }

    /// Files from any `#[derive(RustEmbed)]` folder.
    pub fn from_embed<E: RustEmbed>() -> Self {
        Self { get: E::get }
    }

    /// False when the frontend was not built: the server still runs, with the API only.
    pub fn is_built(&self) -> bool {
        (self.get)(INDEX).is_some()
    }
}

const INDEX: &str = "index.html";

/// Vite puts every built file except `index.html` in `assets/`, with a content hash in its name.
const HASHED_DIR: &str = "assets/";

/// Fallback handler for every GET that no other route matched.
pub async fn serve(State(assets): State<WebAssets>, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // An unknown API path, or an old script name after an update, must not get the page:
    // the client would read HTML where it expects JSON or JavaScript.
    if path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let path = if path.is_empty() { INDEX } else { path };
    if let Some(file) = (assets.get)(path) {
        return file_response(path, file);
    }
    if path.starts_with(HASHED_DIR) {
        return StatusCode::NOT_FOUND.into_response();
    }

    // Any other path is a screen of the app (`/fermata/39`): the page loads and shows it.
    match (assets.get)(INDEX) {
        Some(file) => file_response(INDEX, file),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn file_response(path: &str, file: EmbeddedFile) -> Response {
    (
        [
            (header::CONTENT_TYPE, file.metadata.mimetype().to_owned()),
            (header::CACHE_CONTROL, cache_control(path).to_owned()),
        ],
        file.data,
    )
        .into_response()
}

/// Hashed files never change under the same name, so the browser keeps them for a year.
/// Everything else (the page, later the manifest and the service worker) is checked at every
/// load, so a new version shows up at once.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with(HASHED_DIR) {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_hashed_files_are_cached() {
        assert_eq!(
            cache_control("assets/index-BP-zhLR-.js"),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("favicon.svg"), "no-cache");
    }
}
