extern crate archive;

use archive::*;
use std::fs::File;


fn main(){

    let mut arc = Writer::open_file("write.tar.gz", Format::TarGz).unwrap();

    let mut entry = WriteEntry::new();
    entry.set_path("bar.txt");
    entry.set_permissions(0o644); // not really necessary
    println!("{:?}", arc.add_entry_stream(&mut entry, File::open("foo.txt").unwrap() ));
    println!("@{:?}", arc.add_path("foo.txt"));
    println!("#{:?}", arc.add_path_with_callback(".",
        |e| {
            if e.path() != "./foo.txt" {  // keep in mind that current directory symbol is not removed from path
                false  // return flag if add entry to archive
            } else {
                e.set_path("baz.txt");
                true
            }
        }
        ));


}
