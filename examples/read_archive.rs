extern crate Archive;


fn main() {
	let a = Archive::Archive::new().unwrap();
	a.support_filter_all();
	a.support_format_all();
	let b = a.open_filename("archive.tar", 10240)
		.unwrap();


	loop {
		match b.next_header() {

			Ok(e) => println!("{:?}", e.pathname()),
			Err(e) => { println!("{:?}", e); break }

		}
	}
}