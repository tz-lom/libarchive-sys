extern crate archive;

use archive::*;
use std::fs::File;

fn main() {

    let f = File::open("archive.tar").unwrap();

    let mut a = Reader::open_stream(f).unwrap();
    let mut i = a.entries();
    loop {
        match i.next() {
            Some(e) => println!("{:?}", e.path()),
            None => { break }
        }
    }

    println!("the end");
}
