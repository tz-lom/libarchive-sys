extern crate archive;

use archive::*;
use std::fs::File;

fn main() {

    let f = File::open("archive.tar").unwrap();

    let mut a = Reader::open_stream(f).unwrap();
    loop {
        match a.next() {
            Some(e) => println!("{:?}", e.path()),
            None => { break }
        }
    }

    println!("-------------");
}
