use super::mb_share_mem::*;
use crate::mb_rpcs::*;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;
use std::sync::Mutex;

pub trait MBPtrReader: Read {
    fn read_slice<T: Sized + Copy>(&mut self, data: &mut [T]);
    fn try_read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) -> usize;
    fn read_sized<T: Sized>(&mut self, data: &mut T);
}

pub trait MBPtrWriter: Write {
    fn write_slice<T: Sized + Copy>(&mut self, data: &[T]);
    fn try_write_slice<T: Sized + Copy>(&mut self, data: &[T]) -> usize;
    fn write_sized<T: Sized>(&mut self, data: &T);
}

pub trait MBPtrResolver {
    type READER: MBPtrReader;
    type WRITER: MBPtrWriter;
    fn reader<T: Sized>(&self, ptr: *const T) -> Self::READER;
    fn writer<T: Sized>(&self, ptr: *mut T) -> Self::WRITER;
    fn read_slice<T: Sized + Copy>(&self, ptr: *const T, data: &mut [T]) {
        self.reader(ptr).read_slice(data)
    }
    fn try_read_slice<T: Sized + Copy>(&self, ptr: *const T, data: &mut [T]) -> usize {
        self.reader(ptr).try_read_slice(data)
    }
    fn read_sized<T: Sized>(&self, ptr: *const T, data: &mut T) {
        self.reader(ptr).read_sized(data)
    }
    fn write_slice<T: Sized + Copy>(&self, ptr: *mut T, data: &[T]) {
        self.writer(ptr).write_slice(data)
    }
    fn try_write_slice<T: Sized + Copy>(&self, ptr: *mut T, data: &[T]) -> usize {
        self.writer(ptr).try_write_slice(data)
    }
    fn write_sized<T: Sized>(&self, ptr: *mut T, data: &T) {
        self.writer(ptr).write_sized(data)
    }
    fn read_str(&self, str_args: &MBStringArgs) -> Result<String, String> {
        let str_len = str_args.len as usize;
        let raw_ptr = str_args.ptr as *const usize;
        let mut buf_reader = BufReader::new(self.reader(raw_ptr as *const u8));
        let mut buf = vec![0u8; str_len];
        buf_reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
        Ok(String::from_utf8(buf).map_err(|e| e.to_string())?)
    }
    fn read_c_str(&self, ptr: *const u8) -> Result<String, String> {
        let mut buf_reader = BufReader::new(self.reader(ptr).take(4096));
        let mut buf = vec![];
        buf_reader
            .read_until(b'\0', &mut buf)
            .map_err(|e| e.to_string())?;
        if buf[buf.len() - 1] == 0 {
            buf = buf[..buf.len() - 1].to_vec();
        }
        let string = String::from_utf8(buf).map_err(|e| e.to_string())?;
        Ok(string)
    }
}

pub struct MBLocalPtrReader {
    ptr: *const u8,
}
impl MBLocalPtrReader {
    fn new(ptr: *const u8) -> MBLocalPtrReader {
        MBLocalPtrReader { ptr }
    }
}
impl MBPtrReader for MBLocalPtrReader {
    fn read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) {
        self.try_read_slice(data);
    }
    fn try_read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) -> usize {
        unsafe {
            std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut T, data.len())
                .copy_from_slice(std::slice::from_raw_parts(self.ptr as *const T, data.len()))
        };
        std::mem::size_of::<T>() * data.len()
    }
    fn read_sized<T: Sized>(&mut self, data: &mut T) {
        unsafe {
            std::slice::from_raw_parts_mut(data as *mut T as *mut u8, std::mem::size_of::<T>())
                .copy_from_slice(std::slice::from_raw_parts(
                    self.ptr as *const T as *const u8,
                    std::mem::size_of::<T>(),
                ))
        }
    }
}

impl Read for MBLocalPtrReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len();
        unsafe {
            std::slice::from_raw_parts_mut(buf.as_mut_ptr(), len)
                .copy_from_slice(std::slice::from_raw_parts(self.ptr, len))
        }
        Ok(len)
    }
}

pub struct MBLocalPtrWriter {
    ptr: *mut u8,
}
impl MBLocalPtrWriter {
    fn new(ptr: *mut u8) -> MBLocalPtrWriter {
        MBLocalPtrWriter { ptr }
    }
}
impl MBPtrWriter for MBLocalPtrWriter {
    fn write_slice<T: Sized + Copy>(&mut self, data: &[T]) {
        self.try_write_slice(data);
    }
    fn try_write_slice<T: Sized + Copy>(&mut self, data: &[T]) -> usize {
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr as *mut T, data.len())
                .copy_from_slice(std::slice::from_raw_parts(data.as_ptr(), data.len()))
        };
        std::mem::size_of::<T>() * data.len()
    }
    fn write_sized<T: Sized>(&mut self, data: &T) {
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr, std::mem::size_of::<T>()).copy_from_slice(
                std::slice::from_raw_parts(data as *const T as *const u8, std::mem::size_of::<T>()),
            )
        }
    }
}

impl Write for MBLocalPtrWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        unsafe {
            std::slice::from_raw_parts_mut(self.ptr, len)
                .copy_from_slice(std::slice::from_raw_parts(buf.as_ptr(), len))
        }
        Ok(len)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct MBLocalPtrResolver;
impl MBPtrResolver for MBLocalPtrResolver {
    type READER = MBLocalPtrReader;
    type WRITER = MBLocalPtrWriter;
    fn reader<T: Sized>(&self, ptr: *const T) -> Self::READER {
        MBLocalPtrReader::new(ptr as *const u8)
    }
    fn writer<T: Sized>(&self, ptr: *mut T) -> Self::WRITER {
        MBLocalPtrWriter::new(ptr as *mut u8)
    }
}

pub struct MBSMPtrReaderWrtier<SM: MBShareMem> {
    ptr: MBPtrT,
    sm: Arc<Mutex<SM>>,
}
impl<SM: MBShareMem> MBSMPtrReaderWrtier<SM> {
    fn new(ptr: MBPtrT, sm: &Arc<Mutex<SM>>) -> MBSMPtrReaderWrtier<SM> {
        MBSMPtrReaderWrtier {
            ptr,
            sm: sm.clone(),
        }
    }
}
impl<SM: MBShareMem> MBPtrReader for MBSMPtrReaderWrtier<SM> {
    fn read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) {
        self.sm.lock().unwrap().read_slice(self.ptr, data);
    }
    fn try_read_slice<T: Sized + Copy>(&mut self, data: &mut [T]) -> usize {
        self.sm.lock().unwrap().try_read_slice(self.ptr, data)
    }
    fn read_sized<T: Sized>(&mut self, data: &mut T) {
        self.sm.lock().unwrap().read_sized(self.ptr, data);
    }
}

impl<SM: MBShareMem> Read for MBSMPtrReaderWrtier<SM> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        Ok(self.sm.lock().unwrap().read(self.ptr, buf))
    }
}

impl<SM: MBShareMem> MBPtrWriter for MBSMPtrReaderWrtier<SM> {
    fn write_slice<T: Sized + Copy>(&mut self, data: &[T]) {
        self.sm.lock().unwrap().write_slice(self.ptr, data);
    }
    fn try_write_slice<T: Sized + Copy>(&mut self, data: &[T]) -> usize {
        self.sm.lock().unwrap().try_write_slice(self.ptr, data)
    }
    fn write_sized<T: Sized>(&mut self, data: &T) {
        self.sm.lock().unwrap().write_sized(self.ptr, data);
    }
}

impl<SM: MBShareMem> Write for MBSMPtrReaderWrtier<SM> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(self.sm.lock().unwrap().write(self.ptr, buf))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct MBSMPtrResolver<SM: MBShareMem> {
    sm: Arc<Mutex<SM>>,
}
impl<SM: MBShareMem> MBSMPtrResolver<SM> {
    pub fn new(sm: &Arc<Mutex<SM>>) -> MBSMPtrResolver<SM> {
        MBSMPtrResolver { sm: sm.clone() }
    }
}
impl<SM: MBShareMem> MBPtrResolver for MBSMPtrResolver<SM> {
    type READER = MBSMPtrReaderWrtier<SM>;
    type WRITER = MBSMPtrReaderWrtier<SM>;
    fn reader<T: Sized>(&self, ptr: *const T) -> Self::READER {
        MBSMPtrReaderWrtier::new(ptr as MBPtrT, &self.sm)
    }
    fn writer<T: Sized>(&self, ptr: *mut T) -> Self::WRITER {
        MBSMPtrReaderWrtier::new(ptr as MBPtrT, &self.sm)
    }
}
