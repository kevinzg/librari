use std::env;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use askama::Template;
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::{extract::State, routing::get};
use epub::doc::{EpubDoc, NavPoint};

struct AppState {
    books: Vec<Book>,
}

#[derive(Debug)]
struct Book {
    title: String,
    authors: String,
    doc: EpubDoc<BufReader<File>>,
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

    let shared_state = Arc::new(Mutex::new(AppState { books }));

    let router = axum::Router::new()
        .route("/", get(handle_home))
        .route("/books/:id", get(handle_book_index))
        .route("/books/:id/cover", get(handle_cover))
        .route("/books/:id/res/*path", get(handle_book_resource))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8007")
        .await
        .unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    title: String,
    books: &'a Vec<Book>,
}

async fn handle_home(State(state): State<Arc<Mutex<AppState>>>) -> Html<String> {
    let hello = HomeTemplate {
        title: "My books".to_string(),
        books: &state.lock().unwrap().books,
    };
    return Html(hello.render().unwrap());
}

#[derive(Template)]
#[template(path = "book_index.html", escape = "none")]
struct BookIndexTemplate<'a> {
    title: &'a String,
    toc: NavPointTemplate<'a>,
}

#[derive(Template)]
#[template(path = "nav_point.html", escape = "none")]
struct NavPointTemplate<'a> {
    label: &'a str,
    res: String,
    children: Vec<NavPointTemplate<'a>>,
    book_path: &'a str,
}

async fn handle_book_index(
    Path(id): Path<u64>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    state.lock().unwrap().books.get(id as usize).map_or_else(
        || Err((StatusCode::NOT_FOUND, "Book not found")),
        |book| {
            return Ok(Html(
                BookIndexTemplate {
                    title: &book.title,
                    toc: make_nav_point_template(
                        &NavPoint {
                            label: "Table of contents".to_owned(),
                            play_order: 0,
                            content: PathBuf::new(),
                            children: book.doc.toc.clone(),
                        },
                        &format!("/books/{}", id),
                    ),
                }
                .render()
                .unwrap(),
            ));
        },
    )
}

async fn handle_book_resource(
    Path((id, res_path)): Path<(u64, String)>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    state
        .lock()
        .unwrap()
        .books
        .get_mut(id as usize)
        .map_or_else(
            || Err((StatusCode::NOT_FOUND, "Book not found")),
            |book| {
                book.doc.get_resource_str_by_path(res_path).map_or_else(
                    || Err((StatusCode::NOT_FOUND, "Resource not found")),
                    |content| Ok(Html(content)),
                )
            },
        )
}

fn make_nav_point_template<'a>(
    nav_point: &'a NavPoint,
    book_path: &'a str,
) -> NavPointTemplate<'a> {
    return NavPointTemplate {
        label: &nav_point.label,
        res: nav_point.content.to_str().unwrap().to_owned(),
        children: nav_point
            .children
            .iter()
            .map(|e| make_nav_point_template(e, book_path))
            .collect(),
        book_path,
    };
}

async fn handle_cover(
    Path(id): Path<u64>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    let mut state = state.lock().unwrap();
    let book = state.books.get_mut(id as usize).unwrap();
    let cover = book.doc.get_cover().unwrap();
    return ([(header::CONTENT_TYPE, cover.1)], cover.0);
}

fn get_books(dir: &str) -> Vec<Book> {
    let books: Vec<Book> = walkdir::WalkDir::new(dir)
        .max_depth(3) // enough to run on Calibre's library
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().map_or(false, |e| e == "epub"))
        .filter_map(|e| read_epub(e.path()).ok())
        .collect();
    books
}

fn read_epub(path: &std::path::Path) -> Result<Book, String> {
    let doc = EpubDoc::new(path).map_err(|e| e.to_string())?;
    if let Some(title) = doc.mdata("title") {
        return Ok(Book {
            title,
            authors: doc.mdata("creator").unwrap_or("".to_owned()),
            doc,
        });
    } else {
        return Err("No title found".to_string());
    }
}
