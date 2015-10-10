extern crate archive;

use archive::*;

fn main() {
    let mut a = Reader::open_file("archive.tar").unwrap();

    loop {
        match a.next() {
            Some(e) => println!("{:?}", e.path()),
            None => { break }
        }
    }
    println!("-------------");
}
