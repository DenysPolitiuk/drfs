extern crate crossbeam;
extern crate num_cpus;

use crossbeam::deque::{Injector, Stealer, Worker};

use std::error::Error;
use std::fs;
use std::io;
use std::iter;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use crate::entry::GenericStorage;
use crate::Entry;

#[derive(Debug)]
pub struct DirEntry {
    name: String,
    path_buf: Box<PathBuf>,
    size_all_children: u64,
    last_access_time: Result<SystemTime, io::Error>,
    last_modified_time: Result<SystemTime, io::Error>,
    creation_time: Result<SystemTime, io::Error>,
    children: Vec<String>,
    parent: Option<String>,
}

impl DirEntry {
    pub fn new<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
        parent: Option<String>,
    ) -> Result<DirEntry, Box<Error>> {
        let p = Path::new(&path);
        let name = match p.file_name() {
            None => Err(format!("unable to get name from path {}", p.display()))?,
            Some(file_name) => match file_name.to_str() {
                None => Err(format!("unable to parse name from path {}", p.display()))?,
                Some(file_name) => String::from(file_name),
            },
        };
        let metadata = p.metadata()?;

        // TODO: optionally properly fill children and parent?
        Ok(DirEntry {
            name,
            path_buf: Box::new(p.to_owned()),
            size_all_children: 0,
            last_access_time: metadata.accessed(),
            last_modified_time: metadata.modified(),
            creation_time: metadata.created(),
            children: vec![],
            parent,
        })
    }

    pub fn get_size_all_children(&self) -> u64 {
        self.size_all_children
    }

    pub fn get_format_path(&self) -> String {
        format!("{}", self.path_buf.display())
    }

    pub fn count_entries(&self, storage: &Option<&GenericStorage>) -> usize {
        let storage = match storage {
            // if no storage can only know about it's own children
            None => return self.children.len(),
            Some(v) => v,
        };

        let mut counter = 0;

        for c in &self.children {
            let entry = match storage.pull_out(&c) {
                None => continue,
                Some(v) => v,
            };

            match entry {
                Entry::File(_) => counter += 1,
                Entry::Dir(ref dir) => {
                    counter += dir.count_entries(&Some(*storage));
                }
            }

            storage.set(c.clone(), entry);
        }

        counter
    }

    pub fn calculate_size_all_children(&self, storage: &Option<&GenericStorage>) -> u64 {
        let storage = match storage {
            // if no storage not able to know size of children
            None => return 0,
            Some(v) => v,
        };

        let mut total = 0;

        for c in &self.children {
            let entry = match storage.pull_out(&c) {
                None => continue,
                Some(v) => v,
            };

            total += match entry {
                Entry::File(ref f) => f.get_size(),
                Entry::Dir(ref dir) => dir.calculate_size_all_children(&Some(*storage)),
            };

            storage.set(c.clone(), entry);
        }

        total
    }

    pub fn get_load_children(&self) -> (Vec<Box<Entry>>, Vec<Box<Error>>) {
        let read_dir_results = match fs::read_dir(self.path_buf.as_path()) {
            Err(e) => return (vec![], vec![Box::new(e)]),
            Ok(v) => v,
        };

        let mut errors: Vec<Box<Error>> = vec![];
        let mut entries = vec![];
        for dir_entry in read_dir_results {
            let dir_entry = match dir_entry {
                Err(e) => {
                    errors.push(Box::new(e));
                    continue;
                }
                Ok(value) => value,
            };
            let entry = match Entry::new_with_parent(dir_entry.path(), Some(self.get_format_path()))
            {
                Err(e) => {
                    errors.push(e);
                    continue;
                }
                Ok(value) => value,
            };
            entries.push(Box::new(entry));
        }

        (entries, errors)
    }

    pub fn load_all_childen(&mut self) -> Vec<Box<Error>> {
        self.load_all_childen_with_storage(&None)
    }

    pub fn load_all_childen_with_storage(
        &mut self,
        storage: &Option<GenericStorage>,
    ) -> Vec<Box<Error>> {
        if self.children.len() != 0 {
            panic!("can only load children if have no children already exist");
        }
        // TODO: do something with errors
        let (children, _) = self.get_load_children();

        self.clone_children_to_current(&children);

        let queue = Injector::new();

        let mut file_entries = DirEntry::add_children_to_queue(children, &queue);
        while let Some(entry) = file_entries.pop() {
            DirEntry::store_entry(&storage, entry.get_format_path(), *entry);
        }

        let mut stealers = vec![];
        let mut workers = vec![];
        // pre-populating stealers and workers
        for _ in 0..num_cpus::get() {
            let w = Worker::new_fifo();
            let s = w.stealer();
            stealers.push(s);
            workers.push(w);
        }
        let counter = Arc::new(AtomicIsize::new(0));
        crossbeam::scope(|s| {
            for _ in 0..num_cpus::get() {
                let queue = &queue;
                let worker = workers.pop().unwrap();
                let stealers = &stealers;
                let counter = counter.clone();
                let storage = &storage;
                s.spawn(move |_| {
                    let backoff = crossbeam::utils::Backoff::new();
                    loop {
                        let task = DirEntry::find_task(&worker, &queue, &stealers);
                        match task {
                            // some buffer of time between stopping processing and empty queue
                            // expectation that if the queue is empty there is no more to process
                            // however, this might not be the case if there is a delay somewhere
                            // TODO: better sync method for workers
                            None => backoff.snooze(),
                            Some(mut task) => {
                                counter.fetch_add(1, Ordering::SeqCst);

                                if let Entry::Dir(ref mut d) = *task {
                                    let (children, errors) = d.get_load_children();
                                    d.clone_children_to_current(&children);
                                    // TODO: do something with errors
                                    if errors.len() > 0 {}
                                    let mut file_entries =
                                        DirEntry::add_children_to_queue(children, &queue);
                                    while let Some(entry) = file_entries.pop() {
                                        DirEntry::store_entry(
                                            &storage,
                                            entry.get_format_path(),
                                            *entry,
                                        );
                                    }
                                }
                                DirEntry::store_entry(&storage, task.get_format_path(), *task);

                                counter.fetch_add(-1, Ordering::SeqCst);
                            }
                        };
                        if counter.load(Ordering::SeqCst) <= 0
                            && queue.is_empty()
                            && worker.is_empty()
                        {
                            break;
                        }
                    }
                });
            }
        })
        .unwrap();

        // TODO: return errors
        vec![]
    }

    fn clone_children_to_current(&mut self, children: &Vec<Box<Entry>>) {
        for child in children.iter() {
            self.children.push(child.get_format_path());
        }
    }

    fn store_entry(storage: &Option<GenericStorage>, key: String, entry: Entry) {
        if let Some(storage) = storage {
            storage.set(key, entry);
        }
    }

    fn add_children_to_queue(
        mut children: Vec<Box<Entry>>,
        queue: &Injector<Box<Entry>>,
    ) -> Vec<Box<Entry>> {
        let mut file_entries = vec![];

        while let Some(child) = children.pop() {
            match *child {
                Entry::Dir(_) => queue.push(child),
                Entry::File(_) => file_entries.push(child),
            }
        }

        file_entries
    }

    fn find_task<T>(local: &Worker<T>, global: &Injector<T>, stealers: &[Stealer<T>]) -> Option<T> {
        // Pop a task from the local queue, if not empty.
        local.pop().or_else(|| {
            // Otherwise, we need to look for a task elsewhere.
            iter::repeat_with(|| {
                // Try stealing a batch of tasks from the global queue.
                global
                    .steal_batch_and_pop(local)
                    // .steal()
                    // Or try stealing a task from one of the other threads.
                    .or_else(|| stealers.iter().map(|s| s.steal()).collect())
            })
            // Loop while no task was stolen and any steal operation needs to be retried.
            .find(|s| !s.is_retry())
            // Extract the stolen task, if there is one.
            .and_then(|s| s.success())
        })
    }
}
