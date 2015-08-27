extern crate archive;

use archive::*;

fn main() {
    let mut a = Reader::open_file("archive.tar").unwrap();

    let mut i = a.entries();
    loop {
        match i.next() {
            Some(e) => println!("{:?}", e.path()),
            None => { break }
        }
    }

    println!("the end");
}
