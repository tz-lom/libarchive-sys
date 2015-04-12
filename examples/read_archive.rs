extern crate Archive;

use Archive::*;

fn main() {
    let a = Reader::new().unwrap()
        .support_filter_all()
        .support_format_all()
        .open_filename("archive.tar", 10240).unwrap();

    loop {
        match a.next_header() {
            Ok(e) => println!("{:?}", e.pathname()),
            Err(_) => { break }
        }
    }

    println!("the end");
}