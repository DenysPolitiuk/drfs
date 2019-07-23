use std::convert::AsRef;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use crate::GenericError;

#[derive(Debug, Clone)]
pub struct FileEntry {
    name: String,
    path: Box<PathBuf>,
    size: u64,
    last_access_time: Result<SystemTime, Arc<io::Error>>,
    last_modified_time: Result<SystemTime, Arc<io::Error>>,
    creation_time: Result<SystemTime, Arc<io::Error>>,
    parent: Option<String>,
}

impl FileEntry {
    pub fn new<P: AsRef<Path> + AsRef<OsStr>>(
        path: P,
        parent: Option<String>,
    ) -> Result<FileEntry, GenericError> {
        let p = Path::new(&path);
        let name = match p.file_name() {
            None => Err(format!("unable to get name from path {}", p.display()))?,
            Some(file_name) => match file_name.to_str() {
                None => Err(format!("unable to parse name from path {}", p.display()))?,
                Some(file_name) => String::from(file_name),
            },
        };
        let metadata = p.metadata()?;

        Ok(FileEntry {
            name,
            path: Box::new(p.to_owned()),
            size: metadata.len(),
            last_access_time: match metadata.accessed() {
                Ok(v) => Ok(v),
                Err(e) => Err(Arc::new(e)),
            },
            last_modified_time: match metadata.modified() {
                Ok(v) => Ok(v),
                Err(e) => Err(Arc::new(e)),
            },
            creation_time: match metadata.created() {
                Ok(v) => Ok(v),
                Err(e) => Err(Arc::new(e)),
            },
            parent,
        })
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_format_path(&self) -> String {
        format!("{}", self.path.display())
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }
}
