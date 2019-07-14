use drfs::Entry;

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
    let mut entry = Entry::new("/home/boris/qemu-machines").unwrap();
    let e = &mut entry;
    if let Entry::Dir(e) = e {
        e.load_all_childen();
        // let errors = e.load_childen();
        // for err in errors {
        // println!("Error in load children : {}", err);
        // }
    }
    // let entry = entry;
    // println!("{:?}", entry);
}
