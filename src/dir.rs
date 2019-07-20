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
use std::sync::RwLock;
use std::time::SystemTime;

use crate::Entry;

#[derive(Debug)]
pub struct DirEntry {
    name: String,
    path_buf: Box<PathBuf>,
    size_all_children: u64,
    last_access_time: Result<SystemTime, io::Error>,
    last_modified_time: Result<SystemTime, io::Error>,
    creation_time: Result<SystemTime, io::Error>,
    children: RwLock<Vec<Arc<Entry>>>,
    parent: Box<Option<DirEntry>>,
}

impl DirEntry {
    pub fn new<P: AsRef<Path> + std::convert::AsRef<std::ffi::OsStr>>(
        path: P,
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
            children: RwLock::new(vec![]),
            parent: Box::new(None),
        })
    }

    pub fn get_size_all_children(&self) -> u64 {
        self.size_all_children
    }

    pub fn count_entries(&self) -> u64 {
        let mut counter = 0;

        let children = &*self.children.read().unwrap();
        for c in &*children {
            match **c {
                Entry::File(_) => counter += 1,
                Entry::Dir(ref dir) => {
                    counter += dir.count_entries();
                }
            }
        }

        counter
    }

    pub fn calculate_size_all_children(&self) -> u64 {
        let mut total = 0;
        let children = &*self.children.read().unwrap();
        for c in &*children {
            total += match **c {
                Entry::File(ref f) => f.get_size(),
                Entry::Dir(ref dir) => dir.calculate_size_all_children(),
            };
        }
        total
    }

    pub fn get_load_children(&self) -> (Vec<Arc<Entry>>, Vec<Box<Error>>) {
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
            let entry = match Entry::new(dir_entry.path()) {
                Err(e) => {
                    errors.push(e);
                    continue;
                }
                Ok(value) => value,
            };
            entries.push(Arc::new(entry));
        }

        (entries, errors)
    }

    pub fn load_all_childen(&mut self) -> Vec<Box<Error>> {
        if self.children.read().unwrap().len() != 0 {
            panic!("can only load children if have no children already exist");
        }
        // TODO: do something with errors
        let (children, _) = self.get_load_children();
        *self.children.write().unwrap() = children;

        let queue = Injector::new();
        self.add_children_to_queue(&queue);
        let mut stealers = vec![];
        let mut workers = vec![];
        // pre-populating stealers and workers
        for _ in 0..num_cpus::get() {
            let w: Worker<Arc<Entry>> = Worker::new_fifo();
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
                            Some(task) => {
                                counter.fetch_add(1, Ordering::SeqCst);

                                if let Entry::Dir(ref d) = *task {
                                    let (children, errors) = d.get_load_children();
                                    // TODO: do something with errors
                                    if errors.len() > 0 {}
                                    *d.children.write().unwrap() = children;
                                    d.add_children_to_queue(&queue);
                                }

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

    fn add_children_to_queue(&self, queue: &Injector<Arc<Entry>>) {
        self.children.read().unwrap().iter().for_each(|c| {
            if let Entry::Dir(_) = **c {
                queue.push(Arc::clone(c));
            }
        });
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
