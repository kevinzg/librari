use crate::library::{Book, IndexItem};
use askama::Template;

pub fn render_home(books: &Vec<Book>) -> String {
    let home = HomeTemplate {
        title: "My books",
        books: &books,
    };
    return home.render().unwrap();
}

pub fn render_book_index(title: String, book_index: &Vec<IndexItem>, book_slug: &str) -> String {
    return (BookIndexTemplate {
        title: &title,
        items: &book_index,
        book_slug,
    })
    .render()
    .unwrap();
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
