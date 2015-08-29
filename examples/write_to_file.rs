extern crate archive;

use archive::*;


fn main(){

    let mut arc = Writer::open_file("write.tar.gz", Format::TarGz).unwrap();

    let mut entry = arc.new_entry();
    entry.set_path("./pew.txt");
    entry.set_permissions(0o644);
    println!("{:?}", entry.save_file_by_path("foo.txt"));

}
