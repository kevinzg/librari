use std::collections::HashMap;
use std::env;
use std::sync::Arc;

use axum::extract::Path;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{extract::State, routing::get};
use lazy_static::lazy_static;
use md5::Digest;

struct StaticFile {
    content_type: &'static str,
    content: &'static [u8],
    etag: String, // TODO: How can I make it &'static str?
}

// TODO: Make a macro for this
lazy_static! {
    static ref ASSETS: HashMap<&'static str, StaticFile> = {
        let reset_css_file = {
            // TODO: compress this file?
            let content = include_bytes!("../assets/modern-normalize.css");
            let mut hasher = md5::Md5::new();
            hasher.update(content);
            let etag = format!("\"{:x}\"", hasher.finalize());
            StaticFile {
                content_type: "text/css",
                content,
                etag: etag.to_owned(),
            }
        };

        // TODO: Not sure if I can make a function for this
        //       assuming include_bytes! doesn't work with variables
        let page_css_file = {
            let content = include_bytes!("../assets/page.css");
            let mut hasher = md5::Md5::new();
            hasher.update(content);
            let etag = format!("\"{:x}\"", hasher.finalize());
            StaticFile {
                content_type: "text/css",
                content,
                etag: etag.to_owned(),
            }
        };

        let mut map: HashMap<&'static str, StaticFile> = HashMap::new();
        map.insert("modern-normalize.css", reset_css_file);
        map.insert("page.css", page_css_file);
        map
    };
}

mod library;
mod templates;
mod utils;

struct AppState {
    library: library::Library,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <calibre library>", args[0]);
        return;
    }

    let dir = std::path::Path::new(&args[1]);
    let db = rusqlite::Connection::open_with_flags(
        dir.join("metadata.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("Error opening database");

    let shared_state = Arc::new(AppState {
        library: library::Library::new(dir, db),
    });

    let router = axum::Router::new()
        .route("/", get(handle_home))
        .route("/:slug", get(handle_book_index))
        .route("/:slug/cover", get(handle_book_cover))
        .route("/:slug/*page", get(handle_book_page))
        .route("/_/:slug/*path", get(handle_book_resource))
        .route("/assets/*path", get(handle_assets))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8007")
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

async fn handle_home(State(state): State<Arc<AppState>>) -> Response {
    let library = &state.library;
    let Ok(books) = library.list_books() else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Error listing books").into_response();
    };
    Html(templates::render_home(&books)).into_response()
}

async fn handle_book_index(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let library = &state.library;
    let Ok((title, book_index)) = library.get_book_index(&slug) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };
    Html(templates::render_book_index(title, &book_index, &slug)).into_response()
}

async fn handle_book_cover(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let library = &state.library;
    let Ok((content_type, content)) = library.get_cover(&slug) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.as_ref()),
            (header::CACHE_CONTROL, "private, max-age=2592000"), // 30 days
        ],
        content,
    )
        .into_response()
}

async fn handle_book_page(
    Path((slug, res_path)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let library = &state.library;
    let Ok(book_info) = library.get_book_info(&slug) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };

    Html(templates::render_page(&book_info.title, &slug, &res_path)).into_response()
}

async fn handle_book_resource(
    Path((slug, res_path)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let library = &state.library;
    let Ok((content_type, content)) = library.get_resource(&slug, &res_path) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.as_ref()),
            (header::CACHE_CONTROL, "private, max-age=3600"),
        ],
        content,
    )
        .into_response()
}

async fn handle_assets(Path(asset_path): Path<String>, headers: HeaderMap) -> Response {
    let Some(file) = ASSETS.get(&*asset_path) else {
        return (StatusCode::NOT_FOUND, "Asset not found").into_response();
    };

    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        if if_none_match.as_ref() == file.etag.as_bytes() {
            return (StatusCode::NOT_MODIFIED, ()).into_response();
        }
    }

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, file.content_type),
            (header::ETAG, &file.etag),
        ],
        file.content,
    )
        .into_response()
}
