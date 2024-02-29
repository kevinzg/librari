#![allow(dead_code)]
#![allow(unused)]

use std::{
    cell::RefCell,
    fs::File,
    io::BufReader,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    rc::Rc,
};

// TODO: Should I use tokio::{BufReader, File} instead?
type Epub = epub::doc::EpubDoc<BufReader<File>>;

pub struct Library {
    base_path: PathBuf,
    db: rusqlite::Connection,
    cache: RefCell<lru::LruCache<usize, Rc<RefCell<Epub>>>>,
}

impl Library {
    pub fn new(path: &Path, db: rusqlite::Connection) -> Self {
        Self {
            base_path: path.to_owned(),
            db,
            cache: RefCell::new(lru::LruCache::new(NonZeroUsize::new(5).unwrap())),
        }
    }

    /// List all books in the library
    pub fn list_books(&self, slug: &str) -> Option<Vec<Book>> {
        todo!()
    }

    /// Get any resource from the epub file
    pub fn get_resource(
        &self,
        slug: &str,
        res_path: &str,
    ) -> Result<(String, Vec<u8>), LibraryError> {
        let info = self.get_book_info(slug)?;
        let binding = self.get_epub_doc(&info)?;
        let mut doc = binding.borrow_mut();
        let content = doc
            // TODO: Make this method not need a mutable reference to self
            .get_resource_by_path(&res_path)
            .ok_or(LibraryError::NotFound)?;
        let mime = doc
            .get_resource_mime_by_path(res_path)
            .unwrap_or("application/octet-stream".to_owned());
        Ok((mime, content))
    }

    /// Get the book info from the database
    fn get_book_info(&self, slug: &str) -> Result<BookInfo, LibraryError> {
        let id = get_id(slug)?;
        let info = self
            .db
            .query_row("SELECT title, path FROM books WHERE id = ?", [id], |row| {
                let title: String = row.get(0)?;
                let path: String = row.get(1)?;
                Ok(BookInfo {
                    id,
                    path: self.base_path.join(&path),
                    title,
                })
            })
            .map_err(|e| LibraryError::Sqlite(e))?;
        return Ok(info);
    }

    /// Get the epub document from the cache or load it from the file system
    fn get_epub_doc(&self, info: &BookInfo) -> Result<Rc<RefCell<Epub>>, LibraryError> {
        let cache = &mut self.cache.borrow_mut();
        let ptr = cache.try_get_or_insert(info.id, || {
            let doc = self.load_epub_doc(&info.path)?;
            Ok(Rc::new(RefCell::new(doc)))
        })?;
        Ok(ptr.clone())
    }

    /// Load the epub document from the file system
    fn load_epub_doc(&self, path: &Path) -> Result<Epub, LibraryError> {
        let epub_path = path
            .read_dir()
            .map_err(|e| LibraryError::Io(e))?
            .filter_map(|f| f.ok().map(|e| e.path()))
            .find(|p| p.extension().map_or(false, |e| e == "epub"))
            .ok_or(LibraryError::NotFound)?;
        Epub::new(epub_path).map_err(|e| LibraryError::Epub(e))
    }
}

pub struct Book<'b> {
    title: &'b str,
}

struct BookInfo {
    /// Book id
    id: usize,

    /// Path to the directory on Calibre's library, not to the epub
    path: PathBuf,

    /// Book title
    title: String,
}

pub enum LibraryError {
    NotFound,
    InvalidId(String),
    Io(std::io::Error),
    Epub(epub::doc::DocError),
    Sqlite(rusqlite::Error),
}

pub enum ImageType {
    JPEG,
    PNG,
}

impl ImageType {
    pub fn to_mime_type(&self) -> &'static str {
        match self {
            Self::JPEG => "image/jpeg",
            Self::PNG => "image/png",
        }
    }
}

fn get_id(slug: &str) -> Result<usize, LibraryError> {
    crate::utils::extract_id(slug).map_err(|_| LibraryError::InvalidId(slug.to_owned()))
}