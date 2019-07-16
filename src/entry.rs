use std::error::Error;
use std::path::Path;

use crate::dir::DirEntry;
use crate::file::FileEntry;

// TODO:
//
// * Proper error handling

#[derive(Debug)]
pub enum Entry {
    File(FileEntry),
    Dir(DirEntry),
}

impl Entry {
    pub fn new<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
    ) -> Result<Entry, Box<Error>> {
        let p = Path::new(&path);

        if p.is_dir() {
            Ok(Entry::Dir(DirEntry::new(path)?))
        } else {
            Ok(Entry::File(FileEntry::new(path)?))
        }
    }

    pub fn count_entries(&self) -> u64 {
        match self {
            Entry::File(_) => 1,
            Entry::Dir(dir) => dir.count_entries(),
        }
    }

    pub fn calculate_size(&self) -> u64 {
        match self {
            Entry::File(f) => f.get_size(),
            Entry::Dir(dir) => dir.calculate_size_all_children(),
        }
    }
}
