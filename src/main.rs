use std::env;
use std::sync::Arc;

use askama::Template;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::Html;
use axum::{extract::State, routing::get};
use epub::doc::EpubDoc;

struct AppState {
    books: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <directory>", args[0]);
        return;
    }

    let dir = &args[1];
    let books = get_books(dir);

    let shared_state = Arc::new(AppState { books });

    let router = axum::Router::new()
        .route("/", get(handle_index))
        .route("/books/:id", get(handle_books))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8007")
        .await
        .unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    title: String,
    books: &'a Vec<String>,
}

async fn handle_index(State(state): State<Arc<AppState>>) -> Html<String> {
    let hello = IndexTemplate {
        title: "My books".to_string(),
        books: &state.books,
    };
    return Html(hello.render().unwrap());
}

async fn handle_books(
    Path(id): Path<u64>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    state.books.get(id as usize).map_or_else(
        || Err((StatusCode::NOT_FOUND, "Book not found")),
        |book| Ok(Html(book.to_string())),
    )
}

fn get_books(dir: &str) -> Vec<String> {
    let books: Vec<String> = walkdir::WalkDir::new(dir)
        .max_depth(3) // enough to run on Calibre's library
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().map_or(false, |e| e == "epub"))
        .filter_map(|e| read_epub(e.path()).ok())
        .collect();
    books
}

fn read_epub(path: &std::path::Path) -> Result<String, String> {
    let doc = EpubDoc::new(path).map_err(|e| e.to_string())?;
    if let Some(title) = doc.mdata("title") {
        return Ok(title);
    } else {
        return Err("No title found".to_string());
    }
}
