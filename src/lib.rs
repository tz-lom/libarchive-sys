
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
#[derive(Debug)]
pub enum ArchiveExtractFlag {
    Owner,
    Perm,
    Time,
    No_Overwrite,
    Unlink,
    Acl,
    Fflags,
    Xattr,
    Secure_Symlinks,
    Secure_Nodotdot,
    No_Autodir,
    No_Overwrite_Newer,
    Sparse,
    Mac_Metadata,
    No_Hfs_Compression,
    Hfs_Compression_Forced,
    Secure_Noabsolutepaths
}

pub enum ArchiveFormat {
    _7Zip,
    Ar_Bsd,
    Ar_Svr4,
    Cpio,
    Cpio_newc,
    Gnutar,
    Iso9600,
    Mtree,
    Mtree_Classic,
    Pax,
    Pax_Restricted,
    Shar,
    Shar_Dump,
    Ustar,
    V7tar,
    Xar,
    Zip
}

pub enum ArchiveFilter {
  Bzip2,
  Compress,
  Gzip,
  Lzip,
  Lzma,
  None,
  // TODO : Program(&str)
  Xz
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

fn flags_to_code(flags : Vec<ArchiveExtractFlag>) -> c_int {
    let mut flags_code : c_int = 0;
    for flag in flags.into_iter() {
      let flag_code : c_int = match flag {
          ArchiveExtractFlag::Owner => ARCHIVE_EXTRACT_OWNER,
          ArchiveExtractFlag::Perm => ARCHIVE_EXTRACT_PERM,
          ArchiveExtractFlag::Time => ARCHIVE_EXTRACT_TIME,
          ArchiveExtractFlag::No_Overwrite => ARCHIVE_EXTRACT_NO_OVERWRITE,
          ArchiveExtractFlag::Unlink => ARCHIVE_EXTRACT_UNLINK,
          ArchiveExtractFlag::Acl => ARCHIVE_EXTRACT_ACL,
          ArchiveExtractFlag::Fflags => ARCHIVE_EXTRACT_FFLAGS,
          ArchiveExtractFlag::Xattr => ARCHIVE_EXTRACT_XATTR,
          ArchiveExtractFlag::Secure_Symlinks => ARCHIVE_EXTRACT_SECURE_SYMLINKS,
          ArchiveExtractFlag::Secure_Nodotdot => ARCHIVE_EXTRACT_SECURE_NODOTDOT,
          ArchiveExtractFlag::No_Autodir => ARCHIVE_EXTRACT_NO_AUTODIR,
          ArchiveExtractFlag::No_Overwrite_Newer => ARCHIVE_EXTRACT_NO_OVERWRITE_NEWER,
          ArchiveExtractFlag::Sparse => ARCHIVE_EXTRACT_SPARSE,
          ArchiveExtractFlag::Mac_Metadata => ARCHIVE_EXTRACT_MAC_METADATA,
          ArchiveExtractFlag::No_Hfs_Compression => ARCHIVE_EXTRACT_NO_HFS_COMPRESSION,
          ArchiveExtractFlag::Hfs_Compression_Forced => ARCHIVE_EXTRACT_HFS_COMPRESSION_FORCED,
          ArchiveExtractFlag::Secure_Noabsolutepaths => ARCHIVE_EXTRACT_SECURE_NOABSOLUTEPATHS
      };
      flags_code |= flag_code;
    }
    flags_code
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
    pub fn support_format_raw(self) -> Self {
        unsafe {
            archive_read_support_format_raw(*self.handler);
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

    pub fn read_data<'s>(&'s self, size : size_t) -> Result<Vec<u8>, ArchiveError> {
        unsafe {
          let mut chunk_vec = Vec::with_capacity(size as usize);
          let chunk_ptr = chunk_vec.as_mut_ptr();
          let res = archive_read_data(*self.handler, chunk_ptr as *mut c_void, size) as i32;
          if (res==ARCHIVE_FATAL) || (res==ARCHIVE_WARN) || (res==ARCHIVE_RETRY) {
            Err(code_to_error(res))
          } else if res==0 {
            Err(code_to_error(ARCHIVE_EOF))
          } else {
            chunk_vec.set_len(size as usize);
            Ok(chunk_vec)
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
	handler: Rc<*mut Struct_archive>,
  outUsed : Rc<*mut size_t>
}

impl Drop for Writer {
	fn drop(&mut self) {
		if Rc::is_unique(&self.handler) {
			unsafe { 
        archive_write_free(*self.handler); 
      }
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
        let mut init_used: Box<size_t> = Box::new(0);
        let outUsed: *mut size_t = &mut *init_used;
				Ok(Writer { handler: Rc::new(h), outUsed: Rc::new(outUsed)})
			}
		}
	}
  pub fn add_filter(self, filter : ArchiveFilter) -> Self {
    unsafe {
      match filter {
        ArchiveFilter::Bzip2 => archive_write_add_filter_bzip2(*self.handler),
        ArchiveFilter::Compress => archive_write_add_filter_compress(*self.handler),
        ArchiveFilter::Gzip => archive_write_add_filter_gzip(*self.handler),
        ArchiveFilter::Lzip => archive_write_add_filter_lzip(*self.handler),
        ArchiveFilter::Lzma => archive_write_add_filter_lzma(*self.handler),
        ArchiveFilter::None => archive_write_add_filter_none(*self.handler),
        // TODO : Program(&str)
        ArchiveFilter::Xz => archive_write_add_filter_xz(*self.handler)
      };
    }
    self
  }

  pub fn set_format(self, format : ArchiveFormat) -> Self {
    unsafe {
      match format {
        ArchiveFormat::_7Zip => archive_write_set_format_7zip(*self.handler),
        ArchiveFormat::Ar_Bsd => archive_write_set_format_ar_bsd(*self.handler),
        ArchiveFormat::Ar_Svr4 => archive_write_set_format_ar_svr4(*self.handler),
        ArchiveFormat::Cpio => archive_write_set_format_cpio(*self.handler),
        ArchiveFormat::Cpio_newc => archive_write_set_format_cpio_newc(*self.handler),
        ArchiveFormat::Gnutar => archive_write_set_format_gnutar(*self.handler),
        ArchiveFormat::Iso9600 => archive_write_set_format_iso9660(*self.handler),
        ArchiveFormat::Mtree => archive_write_set_format_mtree(*self.handler),
        ArchiveFormat::Mtree_Classic => archive_write_set_format_mtree_classic(*self.handler),
        ArchiveFormat::Pax => archive_write_set_format_pax(*self.handler),
        ArchiveFormat::Pax_Restricted => archive_write_set_format_pax_restricted(*self.handler),
        ArchiveFormat::Shar => archive_write_set_format_shar(*self.handler),
        ArchiveFormat::Shar_Dump => archive_write_set_format_shar_dump(*self.handler),
        ArchiveFormat::Ustar => archive_write_set_format_ustar(*self.handler),
        ArchiveFormat::V7tar => archive_write_set_format_v7tar(*self.handler),
        ArchiveFormat::Xar => archive_write_set_format_xar(*self.handler),
        ArchiveFormat::Zip => archive_write_set_format_zip(*self.handler),
      };
    }
    self
  }

  pub fn open_filename(&mut self, fileName: &str) -> Result<&mut Self, ArchiveError> {
      let fname = CString::new(fileName).unwrap();
      unsafe {
          let res = archive_write_open_filename(*self.handler, fname.as_ptr());
          if res==ARCHIVE_OK {
              Ok(self)
          } else {
              Err(code_to_error(res))
          }
      }
  }

  pub fn open_memory(&mut self, memory: &mut [u8]) -> Result<&mut Self, ArchiveError> {
      unsafe {
          let memptr: *mut u8 = &mut memory[0];
          let res = archive_write_open_memory(*self.handler, memptr as *mut c_void, memory.len() as u64, *self.outUsed);
          if res==ARCHIVE_OK {
              Ok(self)
          } else {
              Err(code_to_error(res))
          }
      }
  }

  pub fn write_header(&mut self, entry: ArchiveEntryReader) -> Result<&mut Self, ArchiveError> {
      unsafe {
        let res = archive_write_header(*self.handler, entry.entry);
        if res==ARCHIVE_OK {
            Ok(self)
        } else {
            Err(code_to_error(res))
        }
      }
  }

  pub fn write_header_new(&mut self, pathname : &str) -> Result<&mut Self, ArchiveError> {
      unsafe {
        let new_entry = archive_entry_new();
        let c_pathname = CString::new(pathname).unwrap();
        archive_entry_set_pathname(new_entry, c_pathname.as_ptr());
        let new_entryreader = ArchiveEntryReader { entry: new_entry, handler: self.handler.clone() };
        self.write_header(new_entryreader)
      }
  }

  pub fn write_data(&mut self, data: Vec<u8>) -> Result<&mut Self, ArchiveError> {
      unsafe {
        let data_len = data.len();
        let data_bytes = CString::from_vec_unchecked(data);
        // TODO: How to handle errors here?
        archive_write_data(*self.handler, data_bytes.as_ptr() as *mut c_void, data_len as u64);
      }
      Ok(self)
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

    pub fn extract_to(self, path : &str, flags : Vec<ArchiveExtractFlag>) -> Result<Self, ArchiveError> {
        let extract_path = CString::new(path).unwrap();
        unsafe {
            archive_entry_set_pathname(self.entry, extract_path.as_ptr());
            self.extract(flags)
        }
    }
    pub fn extract(self,flags : Vec<ArchiveExtractFlag>) -> Result<Self, ArchiveError> {        
        unsafe {
          let res = archive_read_extract(*self.handler, self.entry, flags_to_code(flags));
          if res==ARCHIVE_OK {
              Ok(self)
          } else {
            Err(code_to_error(res))
          }
        }
    }


    get_time!(access_time, atime);
    get_time!(creation_time, birthtime);
    get_time!(inode_change_time, ctime);
    get_time!(modification_time, mtime);
}

