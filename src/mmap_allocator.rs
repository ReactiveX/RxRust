// Copyright (C) 2015 <Rick Richardson r@12sidedtech.com>
//
// This software may be modified and distributed under the terms
// of the MIT license.  See the LICENSE file for details.

use std::mem;
use std::ops::Drop;
use std::old_io::FilePermission;
use std::fmt;
use std::sync::atomic::{Ordering, AtomicUint};
use std::path::Path;
use nix::sys::{mman, stat};
use nix::{fcntl, unistd};
use nix::sys::stat::{Mode, S_IRWXU, S_IRWXG, S_IRWXO };
use libc::{c_int, c_void};

//when rust has allocator traits, this won't be necessary
use iobuf::Allocator;

lazy_static! {
    static ref ALIGN: usize =  mem::size_of_val(&0us);
    static ref MAGIC: usize = 0x42424242;
    static ref MODE_ALL : Mode = S_IRWXU | S_IRWXG | S_IRWXO;
}

impl Allocator for MappedRegion {
    fn allocate(&self, size: usize, align: usize) -> *mut u8 {
        self.allocate(size, align)
    }

    fn deallocate(&self, _: *mut u8, _: usize, _: usize) {
        // NO DISASSEMBLE!  JOHNNY FIVE ALIVE!
    }
}


pub struct MappedRegion {
    addr: *const c_void,
    total_size: u64,
    fd: c_int,
    header: *mut MMapHeader,
    count: AtomicUint
}

unsafe impl Send for MappedRegion {}
unsafe impl Sync for MappedRegion {}

pub struct MMapHeader {
    magic: usize,
    current: AtomicUint,
    total_size: u64
}

impl fmt::Show for MMapHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MMapHeader[current: {:?}, total_size: {:?}]", self.current.load(Ordering::Relaxed), self.total_size)
    }
}

impl MappedRegion {
    pub fn new(basepath: &str, total_size: u64) -> Result<MappedRegion, String> {
        let fpath = Path::new(basepath);
        let fd = fcntl::open(&fpath, fcntl::O_CREAT | fcntl::O_RDWR, *MODE_ALL).unwrap();
        unistd::ftruncate(fd, total_size as i64).unwrap();
        let ptr = mman::mmap(0 as *mut c_void, total_size, mman::PROT_READ | mman::PROT_WRITE, mman::MAP_SHARED, fd, 0).unwrap();
        match mman::madvise(ptr as *const c_void, total_size, mman::MADV_SEQUENTIAL) {
            Ok(..) => {},
            Err(e) => { error!("Failed to advise mmap to alloc {:?} bytes at {:?} - error: {:?}", total_size, ptr, e);
                        mman::munmap(ptr, total_size).unwrap();
                        unistd::close(fd).unwrap(); }
        }
        let offset = ((mem::size_of::<MMapHeader>() - 1) | (*ALIGN -1)) + 1;
        let headerptr : *mut MMapHeader = unsafe { mem::transmute(ptr) };
        unsafe { *headerptr = MMapHeader {magic: *MAGIC, current: AtomicUint::new(offset), total_size: total_size } };
        Ok(MappedRegion{addr : ptr as *const c_void,
                        total_size : total_size,
                        fd: fd,
                        count: AtomicUint::new(0),
                        header: headerptr})
    }

    pub fn load(basepath: &str) -> Result<MappedRegion, String> {
        let fpath = Path::new(basepath);
        let fd = fcntl::open(&fpath, fcntl::O_CREAT | fcntl::O_RDWR, *MODE_ALL).unwrap();
        let fstat = stat::fstat(fd).unwrap();
        let ptr = mman::mmap(0 as *mut c_void, fstat.st_size as u64, mman::PROT_READ | mman::PROT_WRITE, mman::MAP_SHARED, fd, 0).unwrap();
        let headerptr : *mut MMapHeader = unsafe { mem::transmute(ptr) };
        let ref mut header = unsafe { &(*headerptr) };
        debug!("Mmap File loaded: {:?}", header);

        if  header.magic != *MAGIC &&
            header.total_size != fstat.st_size as u64 { // quick sanity check
            mman::munmap(ptr, fstat.st_size as u64 ).unwrap();
            unistd::close(fd).unwrap();
            Err(format!("Failed to load data file, {:?}. Reported sizes do not match: file size: {:?}, header size {:?}", basepath, fstat.st_size, header.total_size))
        }
        else {
            Ok(MappedRegion{addr : ptr as *const c_void,
                            total_size : header.total_size,
                            fd: fd,
                            count: AtomicUint::new(0),
                            header: headerptr })
        }
    }
    /*
    pub fn alt_new(basepath: &str, count: usize, size: size_t, align: usize) -> Result<MappedRegion, &str> {
        let fpath = Path::new(format!("{:?}-{:?}", basepath, count));
        fcntl::open(&fpath, fcntl::O_CREAT | fcntl::O_RDWR, FilePermission::all()).map(
          |fd| Destructed::new(fd, Box::new(|f| {unistd::close(*f);})).and_then(
          |fd| { unistd::ftruncate(*fd.inner(), size as i64).map(|_| fd )}).and_then(
          |fd| { mman::mmap(0 as *mut c_void, size, mman::PROT_READ | mman::PROT_WRITE, mman::MAP_SHARED, *fd.inner(), 0).map(
                 |ptr| (fd, Destructed::new(ptr, Box::new(|p| { mman::munmap(*p, size);})) )}).and_then(
          |(fd,ptr)| { mman::madvise(*ptr.inner() as *const c_void, size, mman::MADV_SEQUENTIAL).map(|_| (fd, ptr)) }).map(
          |(fd,ptr)| { ptr.release(); fd.release();
                       MappedRegion{addr : *ptr.inner() as *const c_void, total_size : size, current: AtomicUint::new(*ptr.inner() as usize), fd: *fd.inner(), align: align}
                     }).map_err(|e| e.desc())
    }
    */

    /// returns the number of allocations since this object was created
    /// for statistical purposes, does not start from beginning of the
    /// journal file, only the instantiation of MappedRegion by load or new
    pub fn num_allocated(&self) -> usize {
       self.count.load(Ordering::Relaxed)
    }
    //current is guaranteed to start off as aligned, so we'll ensure it stays that way
    //get the current value to return, then calculate the next value to be supplied by
    //the next call to this function
    pub fn allocate(&self, size: usize, align: usize) -> *mut u8 {
        if align != *ALIGN { panic!("User requested alignment of {:?} but we only have {:?}", align, *ALIGN); }
        let ref mut header = unsafe { &(*self.header) };
        let mut offset : usize;
        loop { // attempt to fetch the next available slot, if it is taken, as evidenced by the CAS, then try again
            offset = header.current.load(Ordering::SeqCst);
            let newval = (((offset + size) - 1) | (*ALIGN - 1)) + 1;
            let oldval = header.current.compare_and_swap(offset, newval, Ordering::SeqCst);
            if offset == oldval { break }
        }
        if offset > self.total_size as usize {
            error!("we have gone past our allocated file space for the allocator : offset {:?}", offset);
            return unsafe { mem::transmute::<usize, *mut u8>(0)}
        }
        self.count.fetch_add(1, Ordering::SeqCst);
        unsafe { mem::transmute((self.addr as *mut u8).offset(offset as isize)) }
    }
}

impl Drop for MappedRegion {
    fn drop(&mut self) {
        if mman::msync(self.addr as *const c_void, self.total_size, mman::MS_SYNC).is_err() {}
        if mman::munmap(self.addr as *mut c_void, self.total_size).is_err() {}
        if unistd::close(self.fd).is_err() {}
    }
}

#[cfg(test)]

mod test {

use std::old_io::fs::mkdir_recursive;
use super::MappedRegion;
use std::path::posix::Path;
use std::old_io::FilePermission;
use std::mem;
use super::ALIGN;
struct TestObject {
    v1 : isize,
    v2 : isize,
    v3 : isize,
    v4 : isize,
    v5 : isize,
    v6 : u16,
    v7 : u8,
}



#[test]
fn mmap_allocator_basic() {
    use std::old_io::fs;
    let two_jigabytes = 1024 * 1024 * 1024 * 2;

    mkdir_recursive(&"target/data".parse().unwrap(), FilePermission::from_bits(0o775).unwrap());
    println!("TestObject is {:?} bytes or {:?} rounded", mem::size_of::<TestObject>(),((mem::size_of::<TestObject>() - 1) | (*ALIGN - 1)) + 1);
    {
        let region = MappedRegion::new("./target/data/test_alloc.db", two_jigabytes ).unwrap();

        for i in range(0, 100is) {
            let foo : *mut TestObject = unsafe { mem::transmute(region.allocate(mem::size_of::<TestObject>(), 8)) };
            unsafe { *foo = TestObject { v1: i, v2: i, v3: i, v4: i, v5: i, v6: i as u16, v7: i as u8 } };
        }
    }

    {
        let region = MappedRegion::load("./target/data/test_alloc.db").unwrap();
        for i in range(0, 100is) {
            let foo : *mut TestObject = unsafe { mem::transmute(region.allocate(mem::size_of::<TestObject>(), 8)) };
            unsafe { *foo = TestObject { v1: i*i, v2: i*i, v3: i*i, v4: i*i, v5: i*i, v6: (i*i) as u16, v7: (i*i) as u8 } };
        }
        assert!(region.total_size == two_jigabytes);
    }
    fs::unlink(&Path::new("./target/data/test_alloc.db"));
}

#[test]
fn mmap_allocator_boundaries() {
    use std::old_io::fs;
    mkdir_recursive(&"target/data".parse().unwrap(), FilePermission::from_bits(0o775).unwrap());

    let one_k = 1024;
    {
        let region = MappedRegion::new("./target/data/test_alloc2.db", one_k).unwrap();
        for i in range(0, one_k / mem::size_of::<TestObject>() as u64) {
            let foo : *mut TestObject = unsafe { mem::transmute(region.allocate(mem::size_of::<TestObject>(), 8)) };
            assert!(foo as usize != 0);
        }
        let foo : *mut TestObject = unsafe { mem::transmute(region.allocate(mem::size_of::<TestObject>(), 8)) };
        assert!(foo as usize == 0);
    }
        let region = MappedRegion::load("./target/data/test_alloc2.db").unwrap();
        let foo : *mut TestObject = unsafe { mem::transmute(region.allocate(mem::size_of::<TestObject>(), 8)) };
        assert!(foo as usize == 0);

    fs::unlink(&Path::new("./target/data/test_alloc2.db"));
}

}
