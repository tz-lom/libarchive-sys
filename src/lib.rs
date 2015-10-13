extern crate libc;
#[macro_use]
extern crate bitflags;


pub mod ffi;
pub use ffi::extract_flags::*;
use ffi::*;

use std::rc::Rc;
use std::ffi::CString;
use std::io::{Read, Write, Cursor};
use std::error::Error as StdError;
use std::any::Any;
use std::path::Path;
use std::fs::File;

#[derive(Debug)]
pub enum Error {
    Allocation,
    Initialization,
    Open,
    EntryNotWritten,
    EntryWrittenPartly,
    EntryExtractedPartly,
    NotAFile
}

pub trait ArchiveHandle {
    unsafe fn archive_handle(&self) -> *mut ffi::Struct_archive;

    fn error_string(&self) -> String {
        unsafe {
            wrap_to_string(archive_error_string(self.archive_handle()))
        }
    }
}

unsafe fn wrap_to_string(ptr: *const ::libc::c_char) -> String {
    if ptr.is_null() {
        return String::new()
    }
    let path = std::ffi::CStr::from_ptr(ptr);
    String::from(std::str::from_utf8(path.to_bytes()).unwrap())
}

pub trait Entry {
    unsafe fn entry_handle(&self) -> *mut ffi::Struct_archive_entry;

    fn is_file(&self) -> bool {
        unsafe {
            archive_entry_filetype(self.entry_handle()) as u32 == ffi::AE_IFREG
        }
    }

    fn path(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_pathname(self.entry_handle()))
        }
    }

    fn user_name(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_uname(self.entry_handle()))
        }
    }

    fn group_name(&self) -> String {
        unsafe {
            wrap_to_string(archive_entry_gname(self.entry_handle()))
        }
    }

    fn set_path(&mut self, path: &str) {
        unsafe {
            archive_entry_update_pathname_utf8(self.entry_handle(), CString::new(path).unwrap().as_ptr());
        }
    }

    fn set_permissions(&mut self, perm: u16) {
        unsafe {
            archive_entry_set_perm(self.entry_handle(), perm);
        }
    }

    fn stub(&mut self) {
        unsafe {
            archive_entry_set_filetype(self.entry_handle(), AE_IFREG);
        }
    }
}

pub struct ReaderEntry {
    handle: *mut Struct_archive_entry,
    archive: *mut Struct_archive
}

impl ArchiveHandle for ReaderEntry {
    unsafe fn archive_handle(&self) -> *mut ffi::Struct_archive {
        self.archive
    }
}

impl Entry for ReaderEntry {
    unsafe fn entry_handle(&self) -> *mut ffi::Struct_archive_entry {
        self.handle
    }
}

impl ReaderEntry {
    pub fn extract(self, flags: ExtractFlags) -> bool{
        unsafe {
            match archive_read_extract(self.archive, self.handle, flags.bits()) {
                ARCHIVE_OK | ARCHIVE_WARN => true,
                _ => false
            }
        }
    }
}

struct ReaderFromStream {
    reader: Box<Read>,
    buffer: Vec<u8>
}

impl ReaderFromStream {
    fn read_bytes(&mut self) -> std::io::Result<usize> {
        self.reader.read(&mut self.buffer[..])
    }
}

pub struct Reader {
    handle: *mut ffi::Struct_archive,
    entry: ReaderEntry,
    reader: Option<Box<ReaderFromStream>>
}

impl Drop for Reader {
    fn drop(&mut self) {
        unsafe {
            archive_read_free(self.handle);
        }
    }
}

impl ArchiveHandle for Reader {
    unsafe fn archive_handle(&self) -> *mut ffi::Struct_archive {
        self.handle
    }
}

unsafe fn allow_all_formats(hnd: *mut ffi::Struct_archive) -> Result<(), Error > {
    let res = archive_read_support_filter_all(hnd);
    if res != ARCHIVE_OK {
        archive_read_free(hnd);
        return Err(Error::Initialization);
    }
    let res = archive_read_support_format_all(hnd);
    if res != ARCHIVE_OK {
        archive_read_free(hnd);
        return Err(Error::Initialization);
    }
    let res = archive_read_support_compression_all(hnd);
    if res != ARCHIVE_OK {
        archive_read_free(hnd);
        return Err(Error::Initialization);
    }
    Ok({})
}


extern "C" fn arch_read(arch: *mut Struct_archive, _client_data: *mut ::libc::c_void, _buffer: *mut *const ::libc::c_void) -> ::libc::ssize_t {
    unsafe {
        // use client_data as pointer to ReadContainer struct
        let rd: &mut ReaderFromStream = &mut *(_client_data as *mut ReaderFromStream);
        *_buffer = rd.buffer.as_mut_ptr() as *mut ::libc::c_void;

        let size = rd.read_bytes();

        if size.is_err() {
            let err = size.unwrap_err();
            let descr = CString::new(err.description()).unwrap();
            archive_set_error(arch, err.raw_os_error().unwrap_or(0), descr.as_ptr());
            return -1;
        }
        return size.unwrap() as ::libc::ssize_t;
    }
}

impl Reader {
    pub fn open_file<P: AsRef<Path>>(file: P) -> Result<Reader, Error> {
        let fname = CString::new(file.as_ref().to_string_lossy().as_bytes()).unwrap();
        unsafe {
            let hnd = archive_read_new();
            if hnd.is_null() {
                return Err(Error::Allocation);
            }

            try!(allow_all_formats(hnd));

            let res = archive_read_open_filename(hnd, fname.as_ptr(), 10240);
            if res==ARCHIVE_OK {
                Ok( Reader {
                        handle: hnd,
                        reader: None,
                        entry: ReaderEntry {
                            handle: std::ptr::null_mut(),
                            archive: hnd
                        }
                } )
            } else {
                archive_read_free(hnd);
                Err(Error::Open)
            }
        }
    }

    pub fn open_stream<T: Any+Read>(source: T) -> Result<Self, Error> {
        unsafe {
            let hnd = archive_read_new();
            if hnd.is_null() {
                return Err(Error::Allocation);
            }

            try!(allow_all_formats(hnd));


            let r = ReaderFromStream {
                reader: Box::new(source),
                buffer: vec![0; 8192]
                };
            let mut rfs = Box::new(r);
            let raw = &mut *rfs as *mut ReaderFromStream;

            let res = archive_read_open(
                        hnd,
                        raw as *mut ::libc::c_void,
                        None,
                        Some(arch_read),
                        None);

            if res==ARCHIVE_OK {
                Ok( Reader {
                    handle: hnd,
                    entry: ReaderEntry {
                        handle: std::ptr::null_mut(),
                        archive: hnd
                    },
                    reader: Some(rfs)
                } )
            } else {
                archive_read_free(hnd);
                Err(Error::Open)
            }
        }
    }

    pub fn next<'a>(&'a mut self) -> Option<&'a mut ReaderEntry> {
        unsafe {
            let res = archive_read_next_header(self.handle, &mut self.entry.handle);
            if res==ARCHIVE_OK {
                Some( &mut self.entry )
            } else {
                None
            }
        }
    }
}

pub struct WriteEntry {
    handle: *mut Struct_archive_entry
}

impl Entry for WriteEntry {
    unsafe fn entry_handle(&self) -> *mut ffi::Struct_archive_entry {
        self.handle
    }
}

impl Drop for WriteEntry {
    fn drop(&mut self) {
        unsafe {
            archive_entry_free(self.handle);
        }
    }
}

impl WriteEntry {
    pub fn new() -> WriteEntry {
        unsafe {
            WriteEntry { handle: archive_entry_new() }
        }
    }

    pub fn clone(entry: &Entry) -> WriteEntry {
        unsafe {
            WriteEntry { handle: archive_entry_clone(entry.entry_handle()) }
        }
    }

    pub fn reset(&mut self) {
        unsafe {
            archive_entry_clear(self.handle);
        }
    }
}

pub struct Writer {
    handle: *mut Struct_archive,
    resolver: *mut Struct_archive_entry_linkresolver,
    fsreader: *mut Struct_archive
}

pub enum Format {
    Tar,
    TarGz,
    TarXz,
    Zip
}


unsafe fn set_format(hnd: *mut ffi::Struct_archive, format: Format) -> Result<(), Error> {
    use Format::*;

    let res = try!(match format {
        Tar  => Ok(archive_write_add_filter_none(hnd)),
        TarXz => Ok(archive_write_add_filter_xz(hnd)),
        TarGz => Ok(archive_write_add_filter_gzip(hnd)),
        Zip => Ok(ARCHIVE_OK)
    });
    if res!=ARCHIVE_OK {
        return Err(Error::Initialization)
    }

    let res = try!(match format {
        Tar | TarGz | TarXz => Ok(archive_write_set_format_ustar(hnd)),
        Zip => Ok(archive_write_set_format_zip(hnd))
    });
    if res!=ARCHIVE_OK {
        return Err(Error::Initialization)
    }

    Ok({})
}

impl ArchiveHandle for Writer {
    unsafe fn archive_handle(&self) -> *mut ffi::Struct_archive {
        self.handle
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        unsafe {
            if !self.resolver.is_null() && !self.fsreader.is_null() {
                let mut sparse: *mut Struct_archive_entry = std::ptr::null_mut();
                let mut entry = std::ptr::null_mut::<Struct_archive_entry>();
                archive_entry_linkify(self.resolver, &mut entry, &mut sparse);
                if !sparse.is_null() {
                    ll_write_archive_entry(self.fsreader, self.handle, sparse);
                }
                archive_entry_linkresolver_free(self.resolver)
            }

            if !self.fsreader.is_null() {
                archive_read_free(self.fsreader);
            }

            archive_write_free(self.handle);
        }
    }
}

fn ll_write_archive_entry(from: *mut Struct_archive, to: *mut Struct_archive, entry: *mut Struct_archive_entry) -> bool {
    unsafe {
        if archive_write_header(to, entry) != ARCHIVE_OK {
            return false;
        }

        if archive_entry_size(entry)>0 {

            let mut buffer = std::ptr::null::<::libc::c_void>();
            let mut sz = 0;
            let mut offset = 0;
            let mut progress = 0;

            let nullbuff = [0; 4096];

            while archive_read_data_block(from, &mut buffer as *mut *const ::libc::c_void, &mut sz, &mut offset)==ARCHIVE_OK {

                while offset>progress {
                    let wsz = archive_write_data(to,
                        nullbuff.as_ptr() as *mut ::libc::c_void,
                        std::cmp::min(nullbuff.len() as u64, offset as u64)
                        );
                    if wsz<0 {
                        return false;
                    } else {
                        progress += wsz;
                    }
                }

                let wsz = archive_write_data(to, buffer, sz);
                if wsz != sz as i64 {
                    return false;
                } else {
                    progress += wsz;
                }
            }
        }
        archive_write_finish_entry(to);
        true
    }
}


impl Writer {
    pub fn open_file<P: AsRef<Path>>(file: P, format: Format) -> Result<Writer, Error> {
        let fname = CString::new(file.as_ref().to_string_lossy().as_bytes()).unwrap();
        unsafe {
            let hnd = archive_write_new();
            if hnd.is_null() {
                return Err(Error::Allocation);
            }

            match set_format(hnd, format) {
                Err(e) => {
                    archive_write_free(hnd);
                    return Err(e);
                },
                _ => {}
            }

            let res = archive_write_open_filename(hnd, fname.as_ptr());
            if res==ARCHIVE_OK {
                Ok( Writer {
                        handle: hnd,
                        resolver: std::ptr::null_mut(),
                        fsreader: std::ptr::null_mut()
                    } )
            } else {
                archive_write_free(hnd);
                Err(Error::Open)
            }
        }
    }

    pub fn write_entry_stream<T: Read>(&mut self, entry: &mut Entry, mut stream: T) -> bool {
        unsafe {
            let mut buffer = Vec::new();
            match stream.read_to_end(&mut buffer) {
                Ok(size) => {
                    archive_entry_set_size(entry.entry_handle(), size as i64);
                    if archive_write_header(self.handle, entry.entry_handle()) != ARCHIVE_OK {
                        return false;
                    }

                    let mut written = 0;
                    while  written < size {
                        let wsz = archive_write_data(self.handle, buffer.as_mut_ptr() as *const ::libc::c_void, (size) as u64);
                        if wsz != size as i64{
                            return false;
                        }
                        written += wsz as usize;
                    }

                    archive_write_finish_entry(self.handle);

                    true

                }
                Err(_) => false
            }

        }
    }

    pub fn write_archive_entry<E: ArchiveHandle+Entry>(&mut self, entry: &mut E) -> bool {
        unsafe {
            ll_write_archive_entry(entry.archive_handle(), self.handle, entry.entry_handle())
        }
    }

    fn init_disk_reader(&mut self) {
        unsafe {
            if self.fsreader.is_null() {
                self.fsreader = archive_read_disk_new();
                archive_read_disk_set_standard_lookup(self.fsreader);
                self.resolver = archive_entry_linkresolver_new();
                archive_entry_linkresolver_set_strategy(self.resolver, archive_format(self.handle));
            }
        }
    }

    pub fn write_file<P: AsRef<Path>>(&mut self, path: P) -> bool {
        self.init_disk_reader();
        unsafe {
            if archive_read_disk_open(self.fsreader, CString::new(path.as_ref().to_string_lossy().as_bytes()).unwrap().as_ptr()) != ARCHIVE_OK {
                return false;
            }

            let mut entry = archive_entry_new();

            loop {
                let r = archive_read_next_header2(self.fsreader, entry);

                match r {
                    ARCHIVE_EOF => break,
                    ARCHIVE_OK => {
                        archive_read_disk_descend(self.fsreader);

                        let mut sparse : *mut Struct_archive_entry = std::ptr::null_mut();
                        archive_entry_linkify(self.resolver, &mut entry, &mut sparse);

                        if ! entry.is_null() {

                            if ! ll_write_archive_entry(self.fsreader, self.handle, entry) {
                                archive_entry_free(entry);
                                return false;
                            }
                        }

                        if ! sparse.is_null() {
                            if ! ll_write_archive_entry(self.fsreader, self.handle, sparse) {
                                archive_entry_free(entry);
                                return false;
                            }
                        }
                    },
                    _ => {
                        archive_read_close(self.fsreader);
                        return false;
                    }
                }
            }
            archive_read_close(self.fsreader);
            true
        }
    }
}
