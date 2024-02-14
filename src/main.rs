use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use askama::Template;
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::{extract::State, routing::get};
use epub::doc::{EpubDoc, NavPoint};

mod utils;

struct AppState {
    base_path: std::path::PathBuf,
    db: rusqlite::Connection,
}

#[derive(Debug)]
struct Book {
    #[allow(dead_code)]
    id: u64,
    slug: String,
    title: String,
    authors: String,
    year: String,
    has_cover: bool,
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
    .unwrap();

    let shared_state = Arc::new(Mutex::new(AppState {
        base_path: dir.to_owned(),
        db,
    }));

    let router = axum::Router::new()
        .route("/", axum::routing::get(handle_home))
        .route("/:slug", get(handle_book_index))
        .route("/:slug/cover", get(handle_cover))
        .route("/:slug/*path", get(handle_book_resource))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8007")
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router).await.unwrap();
}

async fn handle_home(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let db = &state.lock().unwrap().db;
    let mut stmt = db
        .prepare("SELECT id, title, author_sort, strftime('%Y', pubdate) as year, sort, has_cover FROM books")
        .unwrap();
    let books: Vec<Book> = stmt
        .query_map((), |row| {
            let id = row.get(0)?;
            let sort_title: String = row.get(4)?;
            Ok(Book {
                id,
                slug: format!("{}-{}", id, utils::slugify(&sort_title)),
                title: row.get(1)?,
                authors: row.get(2)?,
                year: row.get(3)?,
                has_cover: row.get(5)?,
            })
        })
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    let hello = HomeTemplate {
        title: "My books".to_string(),
        books: &books,
    };
    return Html(hello.render().unwrap());
}

async fn handle_book_index(
    Path(slug): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> axum::response::Response {
    let state = state.lock().unwrap();

    let Ok((title, doc)) = get_epub_doc(&slug, &state) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };

    let Ok(html) = (BookIndexTemplate {
        title: &title,
        toc: make_nav_point_template(
            &NavPoint {
                label: "Table of contents".to_owned(),
                play_order: 0,
                content: PathBuf::new(),
                children: doc.toc.clone(),
            },
            &format!("/{}", slug),
        ),
    })
    .render() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error rendering template",
        )
            .into_response();
    };

    return Html(html).into_response();
}

async fn handle_book_resource(
    Path((slug, res_path)): Path<(String, String)>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> axum::response::Response {
    let state = state.lock().unwrap();
    let Ok((_, mut doc)) = get_epub_doc(&slug, &state) else {
        return (StatusCode::NOT_FOUND, "Book not found").into_response();
    };
    let Some(content) = doc.get_resource_str_by_path(res_path) else {
        return (StatusCode::NOT_FOUND, "Resource not found").into_response();
    };
    return Html(content).into_response();
}

async fn handle_cover(
    Path(slug): Path<String>,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    let id = utils::extract_id(&slug);
    let state = state.lock().unwrap();
    let path = state
        .db
        .query_row("SELECT path FROM books WHERE id = ?", [id], |row| {
            let path: String = row.get(0)?;
            Ok(std::path::Path::new(&path).to_owned())
        })
        .unwrap();

    let cover = {
        let jpg = state.base_path.join(&path).join("cover.jpg");
        let png = state.base_path.join(&path).join("cover.png");
        if jpg.is_file() {
            jpg
        } else if png.is_file() {
            png
        } else {
            return (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "Cover not found".as_bytes().to_vec(),
            );
        }
    };

    let data = {
        let file = File::open(&cover).unwrap();
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).unwrap();
        buf
    };

    let content_type = match cover.extension().unwrap().to_str().unwrap() {
        "jpg" => "image/jpeg",
        "png" => "image/png",
        _ => "application/octet-stream",
    };

    return (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], data);
}

fn get_epub_doc(
    slug: &str,
    state: &AppState,
) -> Result<(String, EpubDoc<BufReader<File>>), &'static str> {
    let id = utils::extract_id(&slug);
    let (title, path) = state
        .db
        .query_row("SELECT title, path FROM books WHERE id = ?", [id], |row| {
            let title: String = row.get(0)?;
            let path: String = row.get(1)?;
            Ok((title, state.base_path.join(std::path::Path::new(&path))))
        })
        .map_err(|_| "Not found")?;

    if !path.is_dir() {
        return Err("Calibre book path is not a directory");
    }

    let Some(epub_path) = (match path.read_dir() {
        Ok(list) => list
            .filter_map(|f| f.ok().map(|e| e.path()))
            .find(|p| p.extension().map_or(false, |e| e == "epub")),
        Err(_) => None,
    }) else {
        return Err("Book not found");
    };

    return EpubDoc::new(epub_path)
        .map(|doc| (title, doc))
        .map_err(|_| "Error reading epub");
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    title: String,
    books: &'a Vec<Book>,
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
