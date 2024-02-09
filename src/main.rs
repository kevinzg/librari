use std::env;
use std::path::Path;

use epub::doc::EpubDoc;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Usage: {} <directory>", args[0]);
        return;
    }

    let dir = &args[1];

    for entry in walkdir::WalkDir::new(dir)
        .max_depth(3) // enough to run on Calibre's library
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().map_or(false, |e| e == "epub"))
    {
        if let Ok(title) = read_epub(entry.path()) {
            println!("{}", title);
        } else {
            println!("Error reading {}", entry.path().display());
        }
    }
}

fn read_epub(path: &Path) -> Result<String, String> {
    let doc = EpubDoc::new(path).map_err(|e| e.to_string())?;
    if let Some(title) = doc.mdata("title") {
        return Ok(title);
    } else {
        return Err("No title found".to_string());
    }
}
