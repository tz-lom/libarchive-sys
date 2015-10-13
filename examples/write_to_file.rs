extern crate archive;

use archive::*;
use std::fs::File;


fn main(){

    let mut arc = Writer::open_file("write.tar.gz", Format::TarGz).unwrap();

    let mut entry = WriteEntry::new();
    entry.set_path("pew.txt");
    entry.set_permissions(0o644);
    entry.stub();
    println!("{:?}", arc.write_entry_stream(&mut entry, File::open("foo.txt").unwrap() ));
    println!("@{:?}", arc.write_file("foo.txt"));


}
