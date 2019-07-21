use std::error::Error;
use std::path::Path;

use crate::dir::DirEntry;
use crate::file::FileEntry;
use crate::store::MemStorage;
use crate::store::Storage;

// TODO:
//
// * Proper error handling

pub struct EntryWrapper {
    entry: Entry,
    storage: Option<GenericStorage>,
}

pub type GenericStorage = Box<dyn Storage<String, Entry> + Send + Sync>;

impl EntryWrapper {
    pub fn new<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
    ) -> Result<EntryWrapper, Box<Error>> {
        let entry = Entry::new(path)?;
        Ok(EntryWrapper {
            entry,
            storage: None,
        })
    }
    pub fn new_with_memstorage<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
    ) -> Result<EntryWrapper, Box<Error>> {
        let entry = Entry::new(path)?;
        Ok(EntryWrapper {
            entry,
            storage: Some(Box::new(MemStorage::new())),
        })
    }

    pub fn load_all_childen(&mut self) {
        if let Entry::Dir(ref mut dir) = self.entry {
            // TODO: do something with errors
            let _ = dir.load_all_childen_with_storage(&self.storage);
        }
    }
}

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

    pub fn get_format_path(&self) -> String {
        match self {
            Entry::File(f) => f.get_format_path(),
            Entry::Dir(dir) => dir.get_format_path(),
        }
    }
}
