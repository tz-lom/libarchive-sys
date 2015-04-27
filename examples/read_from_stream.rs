extern crate Archive;

use Archive::*;
use std::fs::File;

fn main() {

    let f = File::open("archive.tar").unwrap();

    let a = Reader::new().unwrap()
    .support_filter_all()
    .support_format_all()
    .open_stream(f).unwrap();

    loop {
        match a.next_header() {
                Ok(e) => println!("{:?}", e.pathname()),
                Err(_) => { break }
            }
    }

    println!("the end");
}