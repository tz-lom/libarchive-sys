
#![feature(libc)]

//for debug purpose
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]


mod ffi;

use ffi::archive::*;

use std::ptr;

use std::ffi::CString;
use std::ffi::CStr;

use std::rc::Rc;

struct ReaderHandler {
    h: *mut Struct_archive
}

impl Drop for ReaderHandler {
    fn drop(&mut self) {
        unsafe {
            println!("Dropped\n");
            archive_read_free(self.h);
        }
    }
}

pub struct Reader {
    handler: Rc<ReaderHandler>
}

impl PartialEq for Reader {
    fn eq(&self, other: &Reader) -> bool {
        self.handler.h == other.handler.h
    }
}

impl Eq for Reader {}

impl Reader {
    pub fn new() -> Result<Reader, &'static str> {
        unsafe {
            let h = archive_read_new();

            if h.is_null() {
                Err("Failed to allocate archive struct")
            } else {
                Ok(Reader { handler: Rc::new(ReaderHandler { h:h }) })

            }
        }
    }

    pub fn support_filter_all(self) -> Self {
        unsafe {
            archive_read_support_filter_all(self.handler.h);
        }
        self
    }

    pub fn support_format_all(self) -> Self {
        unsafe {
            archive_read_support_format_all(self.handler.h);
        }
        self
    }

    pub fn open_filename(self, fileName: &str, bufferSize: u64 ) -> Result<Self, &'static str> {
        let fname = CString::new(fileName).unwrap();
        unsafe {
            if archive_read_open_filename(self.handler.h, fname.as_ptr(), bufferSize)==ARCHIVE_OK {
                Ok(self)
            } else {
                Err("Can't open file")
            }
        }
    }

    pub fn next_header<'s>(&'s self) -> Result<ArchiveEntry, &'static str> {
        unsafe {
            let mut entry: *mut Struct_archive_entry = ptr::null_mut();
            if archive_read_next_header(self.handler.h, &mut entry)==ARCHIVE_OK {
                Ok( ArchiveEntry { entry: entry, handler: self.handler.clone() } )
            } else {
                Err("Ok something ends")
            }
        }
    }
}

pub struct ArchiveEntry {
    entry: *mut Struct_archive_entry,
    handler: Rc<ReaderHandler>
}

impl ArchiveEntry {
    pub fn pathname(&self) -> String {
        unsafe {
            let path = CStr::from_ptr(archive_entry_pathname(self.entry));
            let S = std::str::from_utf8(path.to_bytes()).unwrap();
            String::from(S)
        }
    }

    pub fn sourcepath(&self) -> String {
        unsafe {
            let path = CStr::from_ptr(archive_entry_sourcepath(self.entry));
            let S = std::str::from_utf8(path.to_bytes()).unwrap();
            String::from(S)
        }
    }

    pub fn archive(&self) -> Reader {
        Reader { handler: self.handler.clone() }
    }
}

