//! Il frontend incorporato nel binario: gli stessi file che `RagServer` legge dal classpath
//! (`src/main/resources` di kubirag 09aba65, copiati senza modifiche in `web/`), serviti sulle
//! stesse route e con gli stessi `Content-Type`.

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

static WEB: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/web");

const HTML: &str = "text/html";
const CSS: &str = "text/css";
const JS: &str = "application/javascript";

/// Una route statica: il path HTTP, la risorsa del classpath e il suo tipo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticAsset {
    pub route: &'static str,
    pub resource: &'static str,
    pub content_type: &'static str,
}

const fn asset(
    route: &'static str,
    resource: &'static str,
    content_type: &'static str,
) -> StaticAsset {
    StaticAsset {
        route,
        resource,
        content_type,
    }
}

/// Le route statiche di `RagServer`, nello stesso ordine.
pub const STATIC_ASSETS: &[StaticAsset] = &[
    asset("/", "web/index.html", HTML),
    asset("/web/ragboard.css", "web/ragboard.css", CSS),
    asset("/theme.css", "theme.css", CSS),
    asset("/web/app.js", "web/app.js", JS),
    asset("/web/components.js", "web/components.js", JS),
    asset("/web/panels.js", "web/panels.js", JS),
    asset(
        "/web/modules/diagram-popup.js",
        "web/modules/diagram-popup.js",
        JS,
    ),
    asset(
        "/web/modules/concept-map.js",
        "web/modules/concept-map.js",
        JS,
    ),
    asset(
        "/web/modules/governance.js",
        "web/modules/governance.js",
        JS,
    ),
    asset(
        "/web/modules/wiki-links.js",
        "web/modules/wiki-links.js",
        JS,
    ),
    asset(
        "/web/composables/settings.js",
        "web/composables/settings.js",
        JS,
    ),
    asset(
        "/web/composables/pinned.js",
        "web/composables/pinned.js",
        JS,
    ),
    asset(
        "/web/composables/uploads.js",
        "web/composables/uploads.js",
        JS,
    ),
    asset(
        "/web/composables/studio.js",
        "web/composables/studio.js",
        JS,
    ),
    asset(
        "/web/components/header.html",
        "web/components/header.html",
        HTML,
    ),
    asset(
        "/web/components/sources.html",
        "web/components/sources.html",
        HTML,
    ),
    asset(
        "/web/components/workspace.html",
        "web/components/workspace.html",
        HTML,
    ),
    asset(
        "/web/components/studio.html",
        "web/components/studio.html",
        HTML,
    ),
    asset(
        "/web/components/vue-modals.html",
        "web/components/vue-modals.html",
        HTML,
    ),
    asset(
        "/web/components/legacy-modals.html",
        "web/components/legacy-modals.html",
        HTML,
    ),
    asset("/markboard", "markboard.html", HTML),
    asset("/markview", "markview.html", HTML),
    asset("/flashcards", "flashcards.html", HTML),
    asset("/quiz", "quiz.html", HTML),
    asset("/think", "think/index.html", HTML),
    asset("/report", "report.html", HTML),
    asset("/canvas", "canvas/index.html", HTML),
    asset("/tools/canvas/style.css", "canvas/style.css", CSS),
    asset("/tools/canvas/script.js", "canvas/script.js", JS),
    asset("/tools/think/style.css", "think/style.css", CSS),
    asset("/tools/think/script.js", "think/script.js", JS),
    asset("/marked-5-1-1.min.js", "marked-5-1-1.min.js", JS),
    asset("/mermaid-11-9-0.min.js", "mermaid-11-9-0.min.js", JS),
    asset("/book/print", "book-print.html", HTML),
    asset("/assistant", "assistant.html", HTML),
];

impl StaticAsset {
    pub fn bytes(&self) -> &'static [u8] {
        WEB.get_file(self.resource)
            .map_or(&[], |file| file.contents())
    }

    pub fn response(&self) -> Response {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, self.content_type)],
            self.bytes(),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_static_route_has_its_resource_embedded() {
        for asset in STATIC_ASSETS {
            assert!(!asset.bytes().is_empty(), "{} manca", asset.resource);
        }
    }
}
