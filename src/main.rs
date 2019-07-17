use clap::{App, Arg};

use drfs::Entry;

use std::env;

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
        .get_matches();

    let target_name = matches
        .value_of("target")
        .or(env::current_dir().unwrap().to_str())
        .unwrap()
        .to_owned();
    let mut entry = match Entry::new(&target_name) {
        Err(e) => panic!("{}", e),
        Ok(v) => v,
    };
    let e = &mut entry;
    if let Entry::Dir(e) = e {
        e.load_all_childen();
    }
    println!("target is : {}", target_name);
    println!("total number of entries : {}", entry.count_entries());
    println!("total size in bytes is : {}", entry.calculate_size());
}
