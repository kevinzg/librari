#![allow(dead_code)]
#![allow(unused)]

use std::env;
use std::sync::{Arc, Mutex};

use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{extract::State, routing::get};

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
        .route("/:slug/*path", get(handle_book_resource))
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
    return Html(templates::render_home(&books)).into_response();
}

async fn handle_book_index(
    Path(slug): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let library = &state.library;
    let Ok((title, book_index)) = library.get_book_index(&slug) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };
    return Html(templates::render_book_index(title, &book_index, &slug)).into_response();
}

async fn handle_book_resource(
    Path((slug, res_path)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let library = &state.library;
    let Ok((content_type, content)) = library.get_resource(&slug, &res_path) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };
    return (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        content,
    )
        .into_response();
}
