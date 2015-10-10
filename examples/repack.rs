extern crate archive;

use archive::*;

fn main(){

    let mut to = Writer::open_file("repack.zip", Format::Zip).unwrap();

    let mut from = Reader::open_file("test.tar").unwrap();

    loop {
        match from.next() {
            Some(e) => {
                println!("{:?} {:?}", to.write_archive_entry(e), to.error_string());
            },
            None => { break }
        }
    }
}
