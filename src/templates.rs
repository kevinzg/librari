use crate::library::{Book, ChapterInfo, IndexItem};
use askama::Template;

pub fn render_home(books: &Vec<Book>) -> String {
    let home = HomeTemplate {
        title: "My books",
        books,
    };
    home.render().unwrap()
}

pub fn render_book_index(title: String, book_index: &Vec<IndexItem>, book_slug: &str) -> String {
    (BookIndexTemplate {
        title: &title,
        items: book_index,
        book_slug,
    })
    .render()
    .unwrap()
}

pub fn render_page(chapter_info: &ChapterInfo, book_slug: &str, res_path: &str) -> String {
    let prev_page = chapter_info
        .prev_page
        .as_ref()
        .and_then(|p| Some(p.to_str().unwrap()));
    let next_page = chapter_info
        .next_page
        .as_ref()
        .and_then(|p| Some(p.to_str().unwrap()));
    (PageTemplate {
        title: &chapter_info.book_info.title,
        slug: book_slug,
        res_path,
        prev_page,
        next_page,
    })
    .render()
    .unwrap()
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate<'a> {
    title: &'a str,
    books: &'a Vec<Book>,
}

#[derive(Template)]
#[template(path = "book_index.html")]
struct BookIndexTemplate<'a> {
    title: &'a str,
    items: &'a Vec<IndexItem>,
    book_slug: &'a str,
}

#[derive(Template)]
#[template(path = "page.html")]
struct PageTemplate<'a> {
    title: &'a str,
    slug: &'a str,
    res_path: &'a str,
    prev_page: Option<&'a str>,
    next_page: Option<&'a str>,
}
