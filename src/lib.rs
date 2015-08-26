extern crate libc;


pub mod ffi;
use ffi::*;

use std::rc::Rc;
use std::ffi::CString;
use std::io::{Seek, Read, Write};
use std::error::Error;
use std::any::Any;

#[derive(Debug)]
pub enum ArchiveError {
    AllocationError,
    InitializationError,
    Ok,
    Warn,
    Failed,
    Retry,
    Eof,
    Fatal
}

struct ArchiveHandle {
    handle: *mut ffi::Struct_archive,
    reader: Option<Box<Read>>,
//    seeker: Option<Box<Seek>>,
    buffer: Vec<u8>
}

impl ArchiveHandle {
    fn read_bytes(&mut self) -> std::io::Result<usize> {
        self.reader.as_mut().unwrap().read(&mut self.buffer[..])
    }
}


impl Drop for ArchiveHandle {
    fn drop(&mut self){
        unsafe {
            println!("Drop archive");
            archive_read_free(self.handle);
        }
    }
}

fn code_to_error(code: ::libc::c_int) -> ArchiveError {
    match code {
        ARCHIVE_OK     =>  ArchiveError::Ok,
        ARCHIVE_WARN   =>  ArchiveError::Warn,
        ARCHIVE_FAILED => ArchiveError::Failed,
        ARCHIVE_RETRY  => ArchiveError::Retry,
        ARCHIVE_EOF    => ArchiveError::Eof,
        ARCHIVE_FATAL  => ArchiveError::Fatal,
        _   => unreachable!()
    }
}


pub struct Reader {
    arc: Rc<Box<ArchiveHandle>>
}

impl Drop for Reader {
    fn drop(&mut self){
        println!("Drop reader");
    }
}

extern "C" fn arch_read(arch: *mut Struct_archive, _client_data: *mut ::libc::c_void, _buffer: *mut *const ::libc::c_void) -> ::libc::ssize_t {
    unsafe {
        // use client_data as pointer to ReadContainer struct
        let rc: &mut ArchiveHandle = &mut *(_client_data as *mut ArchiveHandle);
        *_buffer = rc.buffer.as_mut_ptr() as *mut ::libc::c_void;

        if rc.reader.is_none() {
            return -1;
        }
        let size = rc.read_bytes();

        if size.is_err() {
            let err = size.unwrap_err();
            let descr = CString::new(err.description()).unwrap();
            archive_set_error(arch, err.raw_os_error().unwrap_or(0), descr.as_ptr());
            return -1;
        }
        return size.unwrap() as ::libc::ssize_t;
    }
}

#[allow(unused_variables)]
extern "C" fn arch_close(arch: *mut Struct_archive, _client_data: *mut ::libc::c_void) -> ::libc::c_int {
    return ARCHIVE_OK;
}

impl Reader {
    pub fn open_file(file_name: &str, buffer_size: u64 ) -> Result<Reader, ArchiveError> {
        let fname = CString::new(file_name).unwrap();
        unsafe {
            let hnd = archive_read_new();
            if hnd.is_null() {
                return Err(ArchiveError::AllocationError);
            }
            let res = archive_read_support_filter_all(hnd);
            if res != ARCHIVE_OK {
                archive_read_free(hnd);
                return Err(ArchiveError::InitializationError);
            }
            let res = archive_read_support_format_all(hnd);
            if res != ARCHIVE_OK {
                archive_read_free(hnd);
                return Err(ArchiveError::InitializationError);
            }
            let res = archive_read_support_compression_all(hnd);
            if res != ARCHIVE_OK {
                archive_read_free(hnd);
                return Err(ArchiveError::InitializationError);
            }

            let r = ArchiveHandle { handle: hnd, reader: None, buffer: Vec::new() };
            let res = archive_read_open_filename(r.handle, fname.as_ptr(), buffer_size);
            if res==ARCHIVE_OK {
                Ok( Reader { arc: Rc::new(Box::new(r)) } )
            } else {
                archive_read_free(hnd);
                Err(code_to_error(res))
            }
        }
    }

    pub fn open_stream<T: Any+Read>(source: T) -> Result<Self, ArchiveError> {
        unsafe {
            let hnd = archive_read_new();
            if hnd.is_null() {
                return Err(ArchiveError::AllocationError);
            }
            let res = archive_read_support_filter_all(hnd);
            if res != ARCHIVE_OK {
                archive_read_free(hnd);
                return Err(ArchiveError::InitializationError);
            }
            let res = archive_read_support_format_all(hnd);
            if res != ARCHIVE_OK {
                archive_read_free(hnd);
                return Err(ArchiveError::InitializationError);
            }
            let res = archive_read_support_compression_all(hnd);
            if res != ARCHIVE_OK {
                archive_read_free(hnd);
                return Err(ArchiveError::InitializationError);
            }

            let mut r = ArchiveHandle { handle: hnd, reader: Some(Box::new(source)), buffer: Vec::with_capacity(8192) };
            for _ in 0..8192 {
                r.buffer.push(0);
            }

            let mut b = Box::new(r);
            let raw = &mut *b as *mut ArchiveHandle;

            let res = archive_read_open(
                        hnd,
                        raw as *mut ::libc::c_void,
                        None,
                        Some(arch_read),
                        Some(arch_close));

            if res==ARCHIVE_OK {
                Ok( Reader { arc: Rc::new(b) } )
            } else {
                Err(code_to_error(res))
            }
        }
    }

    pub fn entries(&mut self) -> FastReadIterator {
        FastReadIterator {
            arc: self.arc.clone(),
            entry: ArchiveEntryReader{ entry: std::ptr::null_mut(), owned: false, arc: self.arc.clone() }
         }
    }
}
/*
impl<'a, T> IntoIterator for &'a mut Reader {
    type Item = &'a ArchiveEntryReader
    type IntoIter = FastReadIterator<'a>

    fn into_iter(self) -> FastReadIterator<'a> {
        FastReadIterator { arc: self.arc.clone(),  }
    }
}
*/

pub struct ArchiveEntryReader {
    entry: *mut Struct_archive_entry,
    owned: bool,
    arc: Rc<Box<ArchiveHandle>>
}

unsafe fn wrap_to_string(ptr: *const ::libc::c_char) -> String {
    let path = std::ffi::CStr::from_ptr(ptr);
    String::from(std::str::from_utf8(path.to_bytes()).unwrap())
}

impl Drop for ArchiveEntryReader {
    fn drop(&mut self){
        if self.owned {
            unsafe {
                println!("Drop entry");
                archive_entry_free(self.entry);
            }
        }
    }
}

impl ArchiveEntryReader {
    pub fn path(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_pathname(self.entry))
        }
    }

    pub fn user_name(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_uname(self.entry))
        }
    }

    pub fn group_name(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_gname(self.entry))
        }
    }
}

pub struct FastReadIterator {
    arc: Rc<Box<ArchiveHandle>>,
    entry: ArchiveEntryReader
}

impl FastReadIterator {
    pub fn next<'a>(&'a mut self) -> Option<&'a ArchiveEntryReader> {
        unsafe {
            let res = archive_read_next_header((*self.arc).handle, &mut self.entry.entry);
            if res==ARCHIVE_OK {
                Some( &self.entry )
            } else {
                None
            }
        }
    }
}
