extern crate crossbeam;
extern crate num_cpus;

use crossbeam::deque::{Injector, Stealer, Worker};

use std::error::Error;
use std::fs;
use std::io;
use std::iter;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
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
    children: Vec<Arc<Mutex<Entry>>>,
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
            children: vec![],
            parent: Box::new(None),
        })
    }

    pub fn get_size_all_children(&self) -> u64 {
        self.size_all_children
    }

    pub fn load_childen(&mut self) -> Vec<Box<Error>> {
        match fs::read_dir(self.path_buf.as_path()) {
            Err(e) => return vec![Box::new(e)],
            Ok(v) => v,
        }
        .map(|entry| match Entry::new(entry?.path()) {
            Err(e) => Err(e),
            Ok(v) => {
                let v_ref = &v;
                match v_ref {
                    Entry::File(f) => self.size_all_children += f.get_size(),
                    _ => (),
                };
                self.children.push(Arc::new(Mutex::new(v)));
                Ok(())
            }
        })
        .filter_map(|x| x.err())
        .collect()
    }

    pub fn load_all_childen(&mut self) -> Vec<Box<Error>> {
        if self.children.len() != 0 {
            panic!("can only load children if have no children already exist");
        }
        // TODO: do something with errors
        let _ = self.load_childen();

        let queue = Injector::new();
        self.add_children_to_queue(&queue);
        let mut stealers = vec![];
        let mut workers = vec![];
        // pre-populating stealers and workers
        for _ in 0..num_cpus::get() {
            let w: Worker<Arc<Mutex<Entry>>> = Worker::new_fifo();
            let s = w.stealer();
            stealers.push(s);
            workers.push(w);
        }
        let counter = Mutex::new(0);
        crossbeam::scope(|s| {
            for _ in 0..num_cpus::get() {
                let queue = &queue;
                let worker = workers.pop().unwrap();
                let stealers = &stealers;
                let counter = &counter;
                s.spawn(move |_| {
                    loop {
                        *counter.lock().unwrap() += 1;
                        let task = DirEntry::find_task(&worker, &queue, &stealers);
                        match task {
                            // some buffer of time between stopping processing and empty queue
                            // expectation that if the queue is empty there is no more to process
                            // however, this might not be the case if there is a delay somewhere
                            // TODO: better sync method for workers
                            None => (),
                            Some(task) => {
                                let task = Arc::clone(&task);
                                let mut entry = task.lock().unwrap();
                                if let Entry::Dir(ref mut d) = *entry {
                                    let errors = d.load_childen();
                                    // TODO: do something with errors
                                    if errors.len() > 0 {}
                                    d.add_children_to_queue(&queue);
                                }
                            }
                        };
                        *counter.lock().unwrap() -= 1;
                        if *counter.lock().unwrap() == 0 && queue.is_empty() {
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

    fn add_children_to_queue(&self, queue: &Injector<Arc<Mutex<Entry>>>) {
        self.children.iter().for_each(|c| {
            let entry = c.lock().unwrap();
            if let Entry::Dir(_) = *entry {
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
                    // .steal_batch_and_pop(local)
                    .steal()
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
