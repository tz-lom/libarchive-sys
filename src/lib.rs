extern crate libc;


pub mod ffi;
use ffi::*;

use std::rc::Rc;
use std::ffi::CString;
use std::io::{Read, Write};
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
    EntryWrittenPartly
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


impl Reader {
    pub fn open_file<P: AsRef<Path>>(file: P) -> Result<Reader, Error> {
        let fname = CString::new(file.as_ref().to_string_lossy().as_bytes()).unwrap();
        unsafe {
            let hnd = archive_read_new();
            if hnd.is_null() {
                return Err(Error::Allocation);
            }

            try!(allow_all_formats(hnd));

            let r = ArchiveHandle { handle: hnd, reader: None, buffer: Vec::new() };
            let res = archive_read_open_filename(r.handle, fname.as_ptr(), 10240);
            if res==ARCHIVE_OK {
                Ok( Reader { arc: Rc::new(Box::new(r)) } )
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

            let r = ArchiveHandle { handle: hnd, reader: Some(Box::new(source)), buffer: vec![0; 8192] };
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
                archive_read_free(hnd);
                Err(Error::Open)
            }
        }
    }

    pub fn entries(&mut self) -> FastReadIterator {
        FastReadIterator {
            arc: self.arc.clone(),
            entry: Entry{ entry: std::ptr::null_mut(), owned: false, arc: self.arc.clone() }
         }
    }
}
/*
impl<'a, T> IntoIterator for &'a mut Reader {
    type Item = &'a Entry
    type IntoIter = FastReadIterator<'a>

    fn into_iter(self) -> FastReadIterator<'a> {
        FastReadIterator { arc: self.arc.clone(),  }
    }
}
*/

pub struct Entry {
    entry: *mut Struct_archive_entry,
    owned: bool,
    arc: Rc<Box<ArchiveHandle>>
}

unsafe fn wrap_to_string(ptr: *const ::libc::c_char) -> String {
    let path = std::ffi::CStr::from_ptr(ptr);
    String::from(std::str::from_utf8(path.to_bytes()).unwrap())
}

impl Drop for Entry {
    fn drop(&mut self){
        if self.owned {
            unsafe {
                println!("Drop entry");
                archive_entry_free(self.entry);
            }
        }
    }
}

impl Entry {
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

    pub fn set_path(&mut self, path: &str) {
        unsafe {
            archive_entry_update_pathname_utf8(self.entry, CString::new(path).unwrap().as_ptr());
        }
    }

    pub fn save_file_by_path<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Error> {
        match File::open(path) {
            Ok(file) => self.save_file(file),
            Err(a) => { println!("oo {:?}", a); Err(Error::EntryNotWritten) }
        }
    }

    pub fn set_permissions(&mut self, perm: u16) {
        unsafe {
            archive_entry_set_perm(self.entry, perm);
        }
    }

    pub fn save_file(&mut self, file: File) -> Result<(), Error> {
        match file.metadata() {
            Ok(meta) => {
                unsafe {
                    if meta.file_type().is_dir() {
                        archive_entry_set_filetype(self.entry, AE_IFDIR);
                    }
                    if meta.file_type().is_symlink() {
                        archive_entry_set_filetype(self.entry, AE_IFLNK);
                    }
                    if meta.file_type().is_file() {
                        archive_entry_set_filetype(self.entry, AE_IFREG);

                    }
                }
                self.save_stream(file, meta.len())
            },
            Err(a) => { println!("{:?}", a); Err(Error::EntryNotWritten) }
        }
    }

    pub fn save_stream<T: Read>(&mut self, mut stream: T, size: u64) -> Result<(), Error> {
        unsafe {
            archive_entry_set_size(self.entry, size as i64);
            let ret = archive_write_header(self.arc.handle, self.entry);
            if ret!=ARCHIVE_OK {
                println!("le fu {:?}", ret);
                return Err(Error::EntryNotWritten);
            }
            // traverce data in blocks of 8K

            let mut buf:[u8; 8192] = [0; 8192];
            let mut written = 0;

            while written < size {
                match stream.read(&mut buf) {
                    Ok(0) => {
                        return Err(Error::EntryWrittenPartly);
                        },
                    Ok(sz) => {
                        let wsz = archive_write_data(self.arc.handle, buf.as_mut_ptr() as *const ::libc::c_void, sz as u64);
                        if wsz!= sz as i64 {
                            return Err(Error::EntryWrittenPartly);
                        }
                        written += sz as u64;
                    },
                    Err(_) => {
                        return Err(Error::EntryWrittenPartly);
                    }
                }
            }
            Ok({})
        }
    }

    //pub fn set_data_from_file(&mut self, )
}

pub struct FastReadIterator {
    arc: Rc<Box<ArchiveHandle>>,
    entry: Entry
}

impl FastReadIterator {
    pub fn next<'a>(&'a mut self) -> Option<&'a Entry> {
        unsafe {
            let res = archive_read_next_header(self.arc.handle, &mut self.entry.entry);
            if res==ARCHIVE_OK {
                Some( &self.entry )
            } else {
                None
            }
        }
    }
}


pub struct Writer {
    arc: Rc<Box<ArchiveHandle>>
}

pub enum Format {
    Tar,
    TarGz,
    TarXz
}

unsafe fn set_format(hnd: *mut ffi::Struct_archive, format: Format) -> Result<(), Error> {
    use Format::*;

    let res = try!(match format {
        Tar  => Ok(archive_write_add_filter_none(hnd)),
        TarXz => Ok(archive_write_add_filter_xz(hnd)),
        TarGz => Ok(archive_write_add_filter_gzip(hnd))
    });
    if res!=ARCHIVE_OK {
        return Err(Error::Initialization)
    }

    let res = try!(match format {
        Tar | TarGz | TarXz => Ok(archive_write_set_format_ustar(hnd)),
        });
    if res!=ARCHIVE_OK {
        return Err(Error::Initialization)
    }

    Ok({})
}

impl Writer {
    pub fn open_file<P: AsRef<Path>>(file: P, format: Format) -> Result<Writer, Error> {
        let fname = CString::new(file.as_ref().to_string_lossy().as_bytes()).unwrap();
        unsafe {
            let hnd = archive_write_new();
            if hnd.is_null() {
                return Err(Error::Allocation);
            }

            try!(set_format(hnd, format));

            let res = archive_write_open_filename(hnd, fname.as_ptr());
            if res==ARCHIVE_OK {
                Ok( Writer { arc: Rc::new( Box::new( ArchiveHandle { handle: hnd, reader: None, buffer: Vec::new() } ) ) } )
            } else {
                archive_write_free(hnd);
                Err(Error::Open)
            }
        }
    }

    pub fn new_entry(&mut self) -> Entry {
        unsafe {
            let e = archive_entry_new();
            Entry { entry: e, owned: true, arc: self.arc.clone() }
        }
    }
}
