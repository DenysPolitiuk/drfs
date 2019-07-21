use std::error::Error;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug)]
pub struct FileEntry {
    name: String,
    path: Box<PathBuf>,
    size: u64,
    last_access_time: Result<SystemTime, io::Error>,
    last_modified_time: Result<SystemTime, io::Error>,
    creation_time: Result<SystemTime, io::Error>,
    parent: Option<String>,
}

impl FileEntry {
    pub fn new<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
        parent: Option<String>,
    ) -> Result<FileEntry, Box<Error>> {
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
            last_access_time: metadata.accessed(),
            last_modified_time: metadata.modified(),
            creation_time: metadata.created(),
            parent,
        })
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_format_path(&self) -> String {
        format!("{}", self.path.display())
    }
}
