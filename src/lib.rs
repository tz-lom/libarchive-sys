
#![feature(libc)]

//for debug purpose
#![allow(dead_code)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![feature(trace_macros)]
#![feature(concat_idents)]
#![feature(box_raw)]
#![feature(rc_unique)]


mod ffi;
use ffi::archive::*;

use std::ptr;
use std::ffi::CString;
use std::ffi::CStr;
use std::rc::Rc;
use std::io::{Read, Seek};
use std::error::Error;
use std::any::Any;


extern crate time;
use time::Timespec;


#[allow(raw_pointer_derive)]
#[derive(PartialEq, Clone)]
pub struct Reader {
    handler: Rc<*mut Struct_archive>
}

#[derive(Debug)]
pub struct AllocationError;
#[derive(Debug)]
pub enum ArchiveError {
    Ok,
    Warn,
    Failed,
    Retry,
    Eof,
    Fatal
}
/*
impl fmt::Debug for AllocationError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("AllocationError").finish()
    }
}

impl fmt::Debug for AllocationError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.debug_struct("AllocationError").finish()
    }
}*/


fn code_to_error(code: c_int) -> ArchiveError {
    match code {
        ARCHIVE_OK => { return ArchiveError::Ok; }
        ARCHIVE_WARN => { return ArchiveError::Warn; }
        ARCHIVE_FAILED => { return ArchiveError::Failed; }
        ARCHIVE_RETRY => { return ArchiveError::Retry; }
        ARCHIVE_EOF => { return ArchiveError::Eof; }
        ARCHIVE_FATAL => { return ArchiveError::Fatal; }
        _ => { panic!(); }
    }
}

struct ReadContainer {
    reader: Box<Read>,
    buffer: Vec<u8>,
    seeker: Option<Box<Seek>>
}

impl ReadContainer {
    fn read_bytes(&mut self) -> std::io::Result<usize> {
        self.reader.read(&mut self.buffer[..])
    }
}

extern "C" fn arch_read(arch: *mut Struct_archive, _client_data: *mut c_void, _buffer: *mut *mut c_void) -> ssize_t {
    unsafe {
        // use client_data as pointer to ReadContainer struct
        let mut rc = Box::from_raw(_client_data as *mut ReadContainer);
        *_buffer = rc.buffer.as_mut_ptr() as *mut c_void;
        let size = rc.read_bytes();
        Box::into_raw(rc);

        if size.is_err() {
            let err = size.unwrap_err();
            let descr = CString::new(err.description()).unwrap();
            archive_set_error(arch, err.raw_os_error().unwrap_or(0), descr.as_ptr());
            return -1;
        }
        return size.unwrap() as ssize_t;
    }
}

#[allow(unused_variables)]
extern "C" fn arch_close(arch: *mut Struct_archive, _client_data: *mut c_void) -> c_int {
    unsafe {
        let rc = Box::from_raw(_client_data as *mut ReadContainer);
        return ARCHIVE_OK;
    }
}

extern "C" fn arch_skip(_: *mut Struct_archive, _client_data: *mut c_void, request: int64_t) -> int64_t {
    unsafe {
        let mut rc = Box::from_raw(_client_data as *mut ReadContainer);

        // we can't return error code here, but if we return 0 normal read will be called, where error code will be set
        if rc.seeker.is_none() {
            Box::into_raw(rc);
            return 0;
        }
        let size = rc.seeker.as_mut().unwrap().seek(std::io::SeekFrom::Current(request)).unwrap_or(0);

        Box::into_raw(rc);
        return size as int64_t;
    }
}

impl Reader {
    pub fn new() -> Result<Reader, AllocationError> {
        unsafe {
            let h = archive_read_new();

            if h.is_null() {
                Err(AllocationError)
            } else {
                Ok(Reader { handler: Rc::new(h) })

            }
        }
    }

    pub fn support_filter_all(self) -> Self {
        unsafe {
            archive_read_support_filter_all(*self.handler);
        }
        self
    }

    pub fn support_format_all(self) -> Self {
        unsafe {
            archive_read_support_format_all(*self.handler);
        }
        self
    }

    pub fn open_filename(self, fileName: &str, bufferSize: u64 ) -> Result<Self, ArchiveError> {
        let fname = CString::new(fileName).unwrap();
        unsafe {
            let res = archive_read_open_filename(*self.handler, fname.as_ptr(), bufferSize);
            if res==ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn open_memory(self, memory: &mut [u8]) -> Result<Self, ArchiveError> {
        unsafe {
            let memptr: *mut u8 = &mut memory[0];
            let res = archive_read_open_memory(*self.handler, memptr as *mut c_void, memory.len() as u64);
            if res==ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn open_stream<T: Any+Read>(self, source: T) -> Result<Self, ArchiveError> {
        unsafe {
            let mut rc_unboxed =  ReadContainer { reader: Box::new(source), buffer: Vec::with_capacity(8192), seeker: None};
            for _ in 0..8192 {
                rc_unboxed.buffer.push(0);
            }
            let rc = Box::new( rc_unboxed );

            let res = archive_read_open(
                        *self.handler,
                        Box::into_raw(rc) as *mut c_void,
                        ptr::null_mut(),
                        arch_read,
                        arch_close);
            if res==ARCHIVE_OK {
                Ok(self)
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn next_header<'s>(&'s self) -> Result<ArchiveEntryReader, ArchiveError> {
        unsafe {
            let mut entry: *mut Struct_archive_entry = ptr::null_mut();
            let res = archive_read_next_header(*self.handler, &mut entry);
            if res==ARCHIVE_OK {
                Ok( ArchiveEntryReader { entry: entry, handler: self.handler.clone() } )
            } else {
                Err(code_to_error(res))
            }
        }
    }
}

impl Drop for Reader {
	fn drop(&mut self) {
		if Rc::is_unique(&self.handler) {
			unsafe { archive_read_free(*self.handler); }
		}
	}
}

#[allow(raw_pointer_derive)]
#[derive(PartialEq, Clone)]
pub struct Writer {
	handler: Rc<*mut Struct_archive>
}

impl Drop for Writer {
	fn drop(&mut self) {
		if Rc::is_unique(&self.handler) {
			unsafe { archive_write_free(*self.handler); }
		}
	}
}

impl Writer {
	pub fn new() -> Result<Writer, AllocationError> {
		unsafe {
			let h = archive_write_new();
			if h.is_null() {
				Err(AllocationError)
			} else {
				Ok(Writer { handler: Rc::new(h) })
			}
		}
	}
}

#[allow(raw_pointer_derive)]
#[derive(PartialEq, Clone)]
pub struct WriterToDisk {
	handler: Rc<*mut Struct_archive>
}

impl WriterToDisk {
	pub fn new() -> Result<WriterToDisk, AllocationError> {
		unsafe {
			let h = archive_write_disk_new();
			if h.is_null() {
					Err(AllocationError)
			} else {
					Ok(WriterToDisk { handler: Rc::new(h) })
			}
		}
	}
}

impl Drop for WriterToDisk {
	fn drop(&mut self) {
		if Rc::is_unique(&self.handler) {
			unsafe { archive_write_free(*self.handler); }
		}
	}
}

pub struct ArchiveEntryReader {
    entry: *mut Struct_archive_entry,
    handler: Rc<*mut Struct_archive>
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

