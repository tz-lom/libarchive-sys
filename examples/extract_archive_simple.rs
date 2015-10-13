extern crate archive;

use archive::*;

fn main() {
    let mut a = Reader::open_file("test.tar").unwrap();

    match a.next() {
        Some(e) => {
            e.extract(ARCHIVE_EXTRACT_DEFAULT);
        },
        None => {
        }
    }

    println!("the end");
}
