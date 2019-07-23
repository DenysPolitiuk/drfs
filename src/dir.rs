extern crate crossbeam;
extern crate num_cpus;

use crossbeam::deque::{Injector, Stealer, Worker};

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::iter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::SystemTime;

use crate::{Entry, GenericError, GenericStorage};

#[derive(Debug, Clone)]
pub struct DirEntry {
    name: String,
    path_buf: Box<PathBuf>,
    size: u64,
    size_all_children: u64,
    last_access_time: Result<SystemTime, Arc<io::Error>>,
    last_modified_time: Result<SystemTime, Arc<io::Error>>,
    creation_time: Result<SystemTime, Arc<io::Error>>,
    children: Vec<String>,
    parent: Option<String>,
}

impl DirEntry {
    pub fn new<P: AsRef<Path> + AsRef<OsStr>>(
        path: P,
        parent: Option<String>,
    ) -> Result<DirEntry, GenericError> {
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
            size: metadata.len(),
            size_all_children: 0,
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
            children: vec![],
            parent,
        })
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_size_all_children(&self) -> u64 {
        self.size_all_children
    }

    pub fn get_format_path(&self) -> String {
        format!("{}", self.path_buf.display())
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_children(&self) -> Vec<String> {
        self.children.iter().map(|c| c.clone()).collect()
    }

    pub fn get_children_len(&self) -> usize {
        self.children.len()
    }

    pub fn count_entries_multi(&self, storage: &Option<&GenericStorage>) -> usize {
        let storage = match storage {
            // if no storage can only know about it's own children
            None => return self.children.len(),
            Some(v) => v,
        };

        let (queue, mut workers, stealers) =
            DirEntry::create_queue_workers_stealers(num_cpus::get());

        DirEntry::add_generic_to_queue(&self.children, &queue);

        let counter = Arc::new(AtomicIsize::new(0));
        let total_entries = Arc::new(AtomicUsize::new(0));
        crossbeam::scope(|s| {
            for _ in 0..num_cpus::get() {
                let queue = &queue;
                let worker = workers.pop().unwrap();
                let stealers = &stealers;
                let counter = counter.clone();
                let total_entries = total_entries.clone();

                s.spawn(move |_| {
                    let backoff = crossbeam::utils::Backoff::new();
                    loop {
                        let task = DirEntry::find_task(&worker, &queue, &stealers);
                        match task {
                            None => backoff.snooze(),
                            Some(task) => {
                                counter.fetch_add(1, Ordering::SeqCst);

                                if let Some(entry) = storage.get(&task) {
                                    total_entries.fetch_add(1, Ordering::SeqCst);
                                    if let Entry::Dir(ref dir) = entry {
                                        DirEntry::add_generic_to_queue(&dir.children, &queue);
                                    }
                                };

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

        total_entries.load(Ordering::Relaxed)
    }

    pub fn count_entries(&self, storage: &Option<&GenericStorage>) -> usize {
        let storage = match storage {
            // if no storage can only know about it's own children
            None => return self.children.len(),
            Some(v) => v,
        };

        let mut counter = 0;

        let mut queue = vec![];

        for c in &self.children {
            queue.push(c.clone());
        }

        while let Some(c) = queue.pop() {
            let mut entry = match storage.get(&c) {
                None => continue,
                Some(v) => v,
            };

            counter += 1;
            if let Entry::Dir(ref mut dir) = entry {
                queue.append(&mut dir.children);
            }
        }

        counter
    }

    pub fn calculate_size_all_children_multi(&self, storage: &Option<&GenericStorage>) -> u64 {
        let storage = match storage {
            // if no storage not able to know size of children
            None => return 0,
            Some(v) => v,
        };

        let (queue, mut workers, stealers) =
            DirEntry::create_queue_workers_stealers(num_cpus::get());

        DirEntry::add_generic_to_queue(&self.children, &queue);

        let counter = Arc::new(AtomicIsize::new(0));
        let total_size = Arc::new(AtomicUsize::new(0));
        crossbeam::scope(|s| {
            for _ in 0..num_cpus::get() {
                let queue = &queue;
                let worker = workers.pop().unwrap();
                let stealers = &stealers;
                let counter = counter.clone();
                let total_size = total_size.clone();

                s.spawn(move |_| {
                    let backoff = crossbeam::utils::Backoff::new();
                    loop {
                        let task = DirEntry::find_task(&worker, &queue, &stealers);
                        match task {
                            None => backoff.snooze(),
                            Some(task) => {
                                counter.fetch_add(1, Ordering::SeqCst);

                                if let Some(entry) = storage.get(&task) {
                                    total_size
                                        .fetch_add(entry.get_size() as usize, Ordering::SeqCst);
                                    if let Entry::Dir(ref dir) = entry {
                                        DirEntry::add_generic_to_queue(&dir.children, &queue);
                                    }
                                };

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

        total_size.load(Ordering::Relaxed) as u64
    }

    pub fn calculate_size_all_children(&self, storage: &Option<&GenericStorage>) -> u64 {
        let storage = match storage {
            // if no storage not able to know size of children
            None => return 0,
            Some(v) => v,
        };

        let mut total = 0;

        let mut queue = vec![];

        for c in &self.children {
            queue.push(c.clone());
        }

        while let Some(c) = queue.pop() {
            let mut entry = match storage.get(&c) {
                None => continue,
                Some(v) => v,
            };

            total += entry.get_size();
            if let Entry::Dir(ref mut dir) = entry {
                queue.append(&mut dir.children);
            }
        }

        total
    }

    pub fn get_load_children(&self) -> (Vec<Box<Entry>>, Vec<GenericError>) {
        let read_dir_results = match fs::read_dir(self.path_buf.as_path()) {
            Err(e) => return (vec![], vec![Box::new(e)]),
            Ok(v) => v,
        };

        let mut errors: Vec<GenericError> = vec![];
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

    pub fn load_all_children_with_storage(
        &mut self,
        storage: &Option<GenericStorage>,
    ) -> Vec<GenericError> {
        if self.children.len() != 0 {
            panic!("can only load children if have no children already exist");
        }

        let mut all_errors = vec![];

        let (children, mut errors) = self.get_load_children();

        if errors.len() > 0 {
            all_errors.append(&mut errors);
        }

        self.clone_children_to_current(&children);

        let (queue, mut workers, stealers) =
            DirEntry::create_queue_workers_stealers(num_cpus::get());

        let mut file_entries = DirEntry::add_children_to_queue(children, &queue);
        while let Some(entry) = file_entries.pop() {
            DirEntry::store_entry(&storage, entry.get_format_path(), *entry);
        }

        let counter = Arc::new(AtomicIsize::new(0));
        let all_errors_ref = &mut all_errors;
        crossbeam::scope(|s| {
            let (tx, rx) = mpsc::channel();
            s.spawn(move |_| loop {
                let error = match rx.recv().unwrap() {
                    None => break,
                    Some(v) => v,
                };

                all_errors_ref.push(error);
            });

            let mut handlers = vec![];
            for _ in 0..num_cpus::get() {
                let queue = &queue;
                let worker = workers.pop().unwrap();
                let stealers = &stealers;
                let counter = counter.clone();
                let storage = &storage;
                let tx = tx.clone();
                let handle = s.spawn(move |_| {
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
                                    let (children, mut errors) = d.get_load_children();
                                    d.clone_children_to_current(&children);
                                    if errors.len() > 0 {
                                        while let Some(error) = errors.pop() {
                                            tx.send(Some(error)).unwrap();
                                        }
                                    }
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
                handlers.push(handle);
            }
            for handle in handlers {
                handle.join().unwrap();
            }
            tx.send(None).unwrap();
        })
        .unwrap();

        all_errors
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

    fn add_generic_to_queue<T: Clone>(to_add: &Vec<T>, queue: &Injector<T>) {
        for entry in to_add.iter() {
            queue.push(entry.clone());
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

    fn create_queue_workers_stealers<T>(
        number: usize,
    ) -> (Injector<T>, Vec<Worker<T>>, Vec<Stealer<T>>) {
        let queue = Injector::new();
        let mut stealers = vec![];
        let mut workers = vec![];

        for _ in 0..number {
            let w = Worker::new_fifo();
            let s = w.stealer();
            stealers.push(s);
            workers.push(w);
        }
        (queue, workers, stealers)
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
