use std::convert::AsRef;
use std::error::Error;
use std::ffi::OsStr;
use std::path::Path;

use crate::dir::DirEntry;
use crate::file::FileEntry;
use crate::store::{MemStorage, Storage};

// TODO:
//
// * Proper error handling

pub struct EntryWrapper {
    entry: Entry,
    storage: Option<GenericStorage>,
}

pub type GenericStorage = Box<dyn Storage<String, Entry> + Send + Sync>;
pub type GenericError = Box<Error + Send + Sync>;

impl EntryWrapper {
    pub fn new<P: AsRef<Path> + AsRef<OsStr>>(path: P) -> Result<EntryWrapper, GenericError> {
        let entry = Entry::new(path)?;
        Ok(EntryWrapper {
            entry,
            storage: None,
        })
    }
    pub fn new_with_memstorage<P: AsRef<Path> + AsRef<OsStr>>(
        path: P,
    ) -> Result<EntryWrapper, GenericError> {
        let entry = Entry::new(path)?;
        Ok(EntryWrapper {
            entry,
            storage: Some(Box::new(MemStorage::new())),
        })
    }

    pub fn replace_from_storage(&mut self, key: &String) {
        if let Some(storage) = &mut self.storage {
            if let Some(new_entry) = storage.get(&key) {
                self.entry = new_entry;
            }
        }
    }

    pub fn load_all_children(&mut self) -> Vec<GenericError> {
        if let Entry::Dir(ref mut dir) = self.entry {
            dir.load_all_children_with_storage(&self.storage)
        } else {
            vec![]
        }
    }

    pub fn count_entries(&self) -> usize {
        match &self.entry {
            Entry::File(_) => 1,
            Entry::Dir(dir) => dir.count_entries(&self.storage.as_ref()),
        }
    }

    pub fn calculate_size(&self) -> u64 {
        match &self.entry {
            Entry::File(f) => f.get_size(),
            Entry::Dir(dir) => dir.calculate_size_all_children(&self.storage.as_ref()),
        }
    }

    pub fn get_name(&self) -> String {
        self.entry.get_name()
    }

    pub fn get_parent(&self) -> Option<String> {
        self.entry.get_parent()
    }

    pub fn get_children(&self) -> Vec<String> {
        self.entry.get_children()
    }

    pub fn get_children_len(&self) -> usize {
        self.entry.get_children_len()
    }
}

#[derive(Debug, Clone)]
pub enum Entry {
    File(FileEntry),
    Dir(DirEntry),
}

impl Entry {
    pub fn new_with_parent<P: AsRef<Path> + AsRef<OsStr>>(
        path: P,
        parent: Option<String>,
    ) -> Result<Entry, GenericError> {
        let p = Path::new(&path);

        if p.is_dir() {
            Ok(Entry::Dir(DirEntry::new(path, parent)?))
        } else {
            Ok(Entry::File(FileEntry::new(path, parent)?))
        }
    }

    pub fn new<P: AsRef<Path> + AsRef<OsStr>>(path: P) -> Result<Entry, GenericError> {
        Entry::new_with_parent(path, None)
    }

    pub fn count_entries(&self) -> usize {
        match self {
            Entry::File(_) => 1,
            Entry::Dir(dir) => dir.count_entries(&None),
        }
    }

    pub fn calculate_size(&self) -> u64 {
        match self {
            Entry::File(f) => f.get_size(),
            Entry::Dir(dir) => dir.calculate_size_all_children(&None),
        }
    }

    pub fn get_size(&self) -> u64 {
        match self {
            Entry::File(f) => f.get_size(),
            Entry::Dir(dir) => dir.get_size(),
        }
    }

    pub fn get_format_path(&self) -> String {
        match self {
            Entry::File(f) => f.get_format_path(),
            Entry::Dir(dir) => dir.get_format_path(),
        }
    }

    pub fn get_name(&self) -> String {
        match self {
            Entry::File(f) => f.get_name(),
            Entry::Dir(dir) => dir.get_name(),
        }
    }

    pub fn get_children(&self) -> Vec<String> {
        match self {
            Entry::File(_) => vec![],
            Entry::Dir(dir) => dir.get_children(),
        }
    }

    pub fn get_children_len(&self) -> usize {
        match self {
            Entry::File(_) => 0,
            Entry::Dir(dir) => dir.get_children_len(),
        }
    }

    pub fn get_parent(&self) -> Option<String> {
        match self {
            Entry::File(f) => f.get_parent(),
            Entry::Dir(dir) => dir.get_parent(),
        }
    }
}
