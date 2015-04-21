
#![feature(libc)]

//for debug purpose
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![feature(trace_macros)]
#![feature(concat_idents)]



mod ffi;

use ffi::archive::*;

use std::ptr;

use std::ffi::CString;
use std::ffi::CStr;

use std::rc::Rc;
use std::io::Read;

extern crate time;
use time::Timespec;

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

    pub fn open_memory(self, memory: &mut [u8]) -> Result<Self, &'static str> {
        unsafe {
            if archive_read_open_memory(self.handler.h, *memory as *mut c_void, memory.len() as u64)==ARCHIVE_OK {
                Ok(self)
            } else {
                Err("Noway")
            }
        }
    }

    pub fn open_stream(self, source: &mut Read) -> Result<Self, &'static str> {
        unsafe {

            if archive_read_open2()==ARCHIVE_OK {
                Ok(self)
            } else {
                Err("Failed to create")
            }
        }
    }

    pub fn next_header<'s>(&'s self) -> Result<ArchiveEntryReader, &'static str> {
        unsafe {
            let mut entry: *mut Struct_archive_entry = ptr::null_mut();
            if archive_read_next_header(self.handler.h, &mut entry)==ARCHIVE_OK {
                Ok( ArchiveEntryReader { entry: entry, handler: self.handler.clone() } )
            } else {
                Err("Ok something ends")
            }
        }
    }
}

pub struct ArchiveEntryReader {
    entry: *mut Struct_archive_entry,
    handler: Rc<ReaderHandler>
}

macro_rules! get_time {
    ( $fname:ident, $apiname:ident) => {
        pub fn $fname(&self) -> Timespec {
            unsafe {
                let sec = (concat_idents!(archive_entry_, $apiname))(self.entry);
                let nsec = (concat_idents!(archive_entry_, $apiname, _nsec))(self.entry);
                Timespec::new(sec, nsec as i32)
            }
        }
    };
}

unsafe fn wrap_to_string(ptr: *const c_char) -> String {
    let path = CStr::from_ptr(ptr);
    String::from(std::str::from_utf8(path.to_bytes()).unwrap())
}

impl ArchiveEntryReader {
    pub fn pathname(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_pathname(self.entry))
        }
    }

    pub fn sourcepath(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_sourcepath(self.entry))
        }
    }

    pub fn archive(&self) -> Reader {
        Reader { handler: self.handler.clone() }
    }

    get_time!(access_time, atime);
    get_time!(creation_time, birthtime);
    get_time!(inode_change_time, ctime);
    get_time!(modification_time, mtime);
}

