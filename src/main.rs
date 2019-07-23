use clap::{App, Arg};

use drfs::ui::ui;
use drfs::EntryWrapper;

use std::env;
use std::time::Instant;

// TODO:
//
// * Given a folder, traverse through it and
//      * Find all files
//      * Find all folders
//      * Collect metadata
//          * File size
//          * Last access time
//          * Last modified time
//          * Creation time
//          * Owner (?)
//          * Group (?)
//          * Extension
//          * Mime type (?)
//  * Store results in a collection
//  * Optionally store to permanent storage
//      * Optionally load from permanent storage
//  * Optionally use TUI to go through results
fn main() {
    let matches = App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("target")
                .short("t")
                .long("target")
                .takes_value(true)
                .help("target to process"),
        )
        .arg(
            Arg::with_name("tui")
                .short("T")
                .long("tui")
                .help("launch an interactive TUI"),
        )
        .arg(
            Arg::with_name("loops")
                .short("l")
                .long("loops")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .help("don't output found errors"),
        )
        .get_matches();

    let target_name = matches
        .value_of("target")
        .or(env::current_dir().unwrap().to_str())
        .unwrap()
        .to_owned();
    let loops = matches
        .value_of("loops")
        .or(Some("1"))
        .unwrap()
        .parse::<u32>()
        .expect("unable to parse loops");
    let quiet = matches.is_present("quiet");
    let is_tui = matches.is_present("tui");

    if is_tui {
        if let Err(e) = ui::run() {
            println!("Got error in tui run : {}", e);
        }
        return;
    }

    let mut total_load_children = 0;
    let mut total_count_entries = 0;
    let mut total_calculate_size = 0;

    for i in 0..loops {
        println!("\nTry #{}", i + 1);

        let mut entry = match EntryWrapper::new_with_memstorage(&target_name) {
            Err(e) => panic!("{}", e),
            Ok(v) => v,
        };

        total_load_children += execute_with_measure_execution_time(|| {
            let errors = entry.load_all_children();
            if !quiet {
                for error in errors {
                    println!("{}", error);
                }
            }
        });

        println!("target is : {}", target_name);

        total_count_entries += execute_with_measure_execution_time(|| {
            println!("total number of entries : {}", entry.count_entries());
        });

        total_calculate_size += execute_with_measure_execution_time(|| {
            let size = entry.calculate_size();
            println!("total size in bytes is : {}", size);

            let (converted_size, size_name) = bytes_to_other(size as f64);
            println!("converted size is {} {}", converted_size, size_name);
        });
    }

    println!("Average over {} iterations is\n\tload children : {} ms\n\tcount entries : {} ms\n\tcalculation size : {} ms", loops,
             total_load_children as f64 / loops as f64, total_count_entries as f64 / loops as f64, total_calculate_size as f64 / loops as f64);
}

fn execute_with_measure_execution_time<F: FnOnce()>(closure: F) -> u128 {
    let start = Instant::now();
    closure();
    let duration = start.elapsed();
    println!("Took {} ms to execute", duration.as_millis());
    duration.as_millis()
}

fn bytes_to_other(bytes: f64) -> (f64, String) {
    let (converted_size, depth) = _bytes_to_other(bytes, 0);
    (converted_size, _depth_to_word(depth))
}

fn _bytes_to_other(bytes: f64, depth: u32) -> (f64, u32) {
    if bytes > 1.0 {
        return _bytes_to_other(bytes / 1024.0, depth + 1);
    }
    if depth < 1 {
        (bytes, depth)
    } else {
        (bytes * 1024.0, depth - 1)
    }
}

fn _depth_to_word(depth: u32) -> String {
    match depth {
        0 => "B".to_string(),
        1 => "KB".to_string(),
        2 => "MB".to_string(),
        3 => "GB".to_string(),
        4 => "TB".to_string(),
        _ => format!("more than TB, depth {}", depth),
    }
}
