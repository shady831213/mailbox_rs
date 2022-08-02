use super::mb_ptr_resolver::*;
use crate::mb_rpcs::*;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub trait MBFile: Read + Seek + Write + Send {}

pub trait MBFileOpener: Sync + Send {
    fn open(&self, path: &str, flags: u32) -> std::io::Result<MBFileType>;
}

pub struct MBNormalFileOpener;
impl MBFileOpener for MBNormalFileOpener {
    fn open(&self, path: &str, flags: u32) -> std::io::Result<MBFileType> {
        Ok(MBFileType::Normal(mb_open_file(path, flags)?))
    }
}

pub fn mb_open_file(path: &str, flags: u32) -> std::io::Result<File> {
    Ok(File::options()
        .create(flags & (MB_FILE_WRITE | MB_FILE_APPEND | MB_FILE_TRUNC) != 0)
        .write(flags & (MB_FILE_WRITE | MB_FILE_APPEND | MB_FILE_TRUNC) != 0)
        .append(flags & MB_FILE_APPEND != 0)
        .truncate(flags & MB_FILE_TRUNC != 0)
        .read(flags & MB_FILE_READ != 0)
        .open(path)?)
}

pub fn into_io_error<E: std::string::ToString>(kind: std::io::ErrorKind, e: E) -> std::io::Error {
    std::io::Error::new(kind, e.to_string())
}

pub enum MBFileType {
    Normal(File),
    Virtual(Box<dyn MBFile>),
    Special(Box<dyn MBFile>),
}

impl Read for MBFileType {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            MBFileType::Normal(f) => f.read(buf),
            MBFileType::Virtual(f) => f.read(buf),
            MBFileType::Special(f) => f.read(buf),
        }
    }
}

impl Write for MBFileType {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            MBFileType::Normal(f) => f.write(buf),
            MBFileType::Virtual(f) => f.write(buf),
            MBFileType::Special(f) => f.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            MBFileType::Normal(f) => f.flush(),
            MBFileType::Virtual(f) => f.flush(),
            MBFileType::Special(f) => f.flush(),
        }
    }
}

impl Seek for MBFileType {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            MBFileType::Normal(f) => f.seek(pos),
            MBFileType::Virtual(f) => f.seek(pos),
            MBFileType::Special(f) => f.seek(pos),
        }
    }
}

impl MBFile for MBFileType {}

struct MBFsInner {
    file_fd_cnt: u32,
    opened_files: BTreeMap<u32, MBFileType>,
}
impl MBFsInner {
    fn insert(&mut self, file: MBFileType) -> std::io::Result<u32> {
        let fd = self.file_fd_cnt;
        self.opened_files.insert(fd, file).map_or(Ok(()), |_| {
            Err(into_io_error(
                std::io::ErrorKind::AlreadyExists,
                "file has been opened!",
            ))
        })?;
        self.file_fd_cnt = self.file_fd_cnt.wrapping_add(1);
        Ok(fd)
    }
    fn reomve(&mut self, fd: &u32) -> std::io::Result<()> {
        self.opened_files.remove(fd).ok_or(into_io_error(
            std::io::ErrorKind::NotFound,
            format!("invalid fd {}!", fd),
        ))?;
        Ok(())
    }
    fn read(&mut self, fd: &u32, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let file = self.opened_files.get_mut(fd).ok_or(into_io_error(
            std::io::ErrorKind::NotFound,
            format!("invalid fd {}!", fd),
        ))?;
        if buf.len() == 0 {
            file.read_to_end(buf)
        } else {
            file.read(buf)
        }
    }
    fn write(&mut self, fd: &u32, buf: &[u8]) -> std::io::Result<usize> {
        let file = self.opened_files.get_mut(fd).ok_or(into_io_error(
            std::io::ErrorKind::NotFound,
            format!("invalid fd {}!", fd),
        ))?;
        file.write(buf)
    }
    fn seek(&mut self, fd: &u32, pos: u64) -> std::io::Result<u64> {
        let file = self.opened_files.get_mut(fd).ok_or(into_io_error(
            std::io::ErrorKind::NotFound,
            format!("invalid fd {}!", fd),
        ))?;
        file.seek(SeekFrom::Start(pos))
    }
}

pub struct MBFs {
    root: PathBuf,
    virtual_openers: HashMap<String, Box<dyn MBFileOpener>>,
    special_openers: HashMap<String, Box<dyn MBFileOpener>>,
    normal_opener: MBNormalFileOpener,
    inner: Mutex<MBFsInner>,
}

impl MBFs {
    pub fn new(path: &str) -> std::io::Result<MBFs> {
        Self::with_special_and_virtual(path, |_| Ok(()), |_| Ok(()))
    }

    pub fn with_special<
        F: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
    >(
        path: &str,
        f: F,
    ) -> std::io::Result<MBFs> {
        Self::with_special_and_virtual(path, f, |_| Ok(()))
    }

    pub fn with_virtual<
        F: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
    >(
        path: &str,
        f: F,
    ) -> std::io::Result<MBFs> {
        Self::with_special_and_virtual(path, |_| Ok(()), f)
    }

    pub fn with_special_and_virtual<
        F1: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
        F2: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
    >(
        path: &str,
        mut special_f: F1,
        mut virtual_f: F2,
    ) -> std::io::Result<MBFs> {
        let path_expand = shellexpand::full(path)
            .map_err(|e| into_io_error(std::io::ErrorKind::NotFound, e))?
            .to_string();
        let root = PathBuf::from(&path_expand).canonicalize()?;
        if !root.is_dir() {
            Err(into_io_error(
                std::io::ErrorKind::NotFound,
                format!("{} is not valid directory!", path_expand),
            ))
        } else {
            let mut special_openers: HashMap<String, Box<dyn MBFileOpener>> = HashMap::new();
            special_f(&mut special_openers)
                .map_err(|e| into_io_error(std::io::ErrorKind::Other, e))?;
            let mut virtual_openers: HashMap<String, Box<dyn MBFileOpener>> = HashMap::new();
            virtual_f(&mut virtual_openers)
                .map_err(|e| into_io_error(std::io::ErrorKind::Other, e))?;
            Ok(MBFs {
                root,
                virtual_openers,
                special_openers,
                normal_opener: MBNormalFileOpener,
                inner: Mutex::new(MBFsInner {
                    file_fd_cnt: 10,
                    opened_files: BTreeMap::new(),
                }),
            })
        }
    }

    fn open_file<P: AsRef<Path>>(&self, path: P, flags: u32) -> std::io::Result<MBFileType> {
        let path_string = shellexpand::full(path.as_ref().to_str().unwrap())
            .map_err(|e| into_io_error(std::io::ErrorKind::NotFound, e))?
            .to_string();
        let path_expand = Path::new(path_string.as_str());
        if path_expand.has_root() || path_expand.starts_with("..") {
            return Err(into_io_error(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "{} maybe out of root {}!",
                    path_expand.display(),
                    self.root.display()
                ),
            ));
        }
        let file_path = self.root.join(&path_expand);
        if path_expand.starts_with("virtual") {
            if let Some(opener) = self.virtual_openers.get(
                path_expand
                    .strip_prefix("virtual/")
                    .unwrap()
                    .to_str()
                    .unwrap(),
            ) {
                return opener.open(file_path.to_str().unwrap(), flags);
            } else {
                return Err(into_io_error(
                    std::io::ErrorKind::NotFound,
                    format!("{} is invalid!", file_path.display()),
                ));
            }
        }
        if let Some(ext) = file_path.extension() {
            if let Some(opener) = self.special_openers.get(ext.to_str().unwrap()) {
                return opener.open(file_path.to_str().unwrap(), flags);
            }
        }
        self.normal_opener.open(file_path.to_str().unwrap(), flags)
    }

    pub fn open<P: AsRef<Path>>(&self, path: P, flags: u32) -> Result<u32, String> {
        let err_handler = |e: String| {
            format!(
                "file:{}, {}",
                self.root.join(path.as_ref()).to_str().unwrap(),
                e
            )
        };
        let file = self
            .open_file(path.as_ref(), flags)
            .map_err(|e| err_handler(e.to_string()))?;
        self.inner
            .lock()
            .unwrap()
            .insert(file)
            .map_err(|e| err_handler(e.to_string()))
    }

    pub fn close(&self, fd: u32) -> std::io::Result<()> {
        self.inner.lock().unwrap().reomve(&fd)
    }
    pub fn read<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>(
        &self,
        r: &R,
        fd: u32,
        ptr: *mut u8,
        len: usize,
    ) -> std::io::Result<usize> {
        let mut buf = vec![0u8; len];
        let ret_len = self.inner.lock().unwrap().read(&fd, &mut buf)?;
        r.write_slice(ptr, &buf[..ret_len]);
        Ok(ret_len)
    }
    pub fn write<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>(
        &self,
        r: &R,
        fd: u32,
        ptr: *const u8,
        len: usize,
    ) -> std::io::Result<usize> {
        let mut buf = vec![0u8; len];
        let len = r.try_read_slice(ptr, &mut buf);
        self.inner.lock().unwrap().write(&fd, &buf[..len])
    }
    pub fn seek(&self, fd: u32, pos: u64) -> std::io::Result<u64> {
        self.inner.lock().unwrap().seek(&fd, pos)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
    use std::io::{BufReader, Read, Seek, SeekFrom, Write};
    #[test]
    fn mb_fs_basic_test() {
        let fs = MBFs::new("resources").unwrap();
        let fd = fs.open("test", MB_FILE_WRITE | MB_FILE_READ).unwrap();
        let resolver = MBLocalPtrResolver::default();
        let data = b"Hello World!";
        let wlen = fs.write(&resolver, fd, data.as_ptr(), data.len()).unwrap();
        println!("wlen = {}", wlen);
        assert_eq!(wlen, data.len());
        let mut buf = vec![0u8; wlen];
        let pos = fs.seek(fd, 0).unwrap();
        assert_eq!(pos, 0);
        let rlen = fs.read(&resolver, fd, buf.as_mut_ptr(), buf.len()).unwrap();
        fs.close(fd).unwrap();
        println!("rlen = {}", rlen);
        assert_eq!(rlen, wlen);
        assert_eq!(data[..], buf[..]);
        println!("content = {}", String::from_utf8(buf).unwrap());
    }

    struct TGReadFile(BufReader<File>);
    impl TGReadFile {
        const fn vec_size() -> usize {
            std::mem::size_of::<u32>()
        }
        const fn vec_num(buf_size: usize) -> usize {
            buf_size / Self::vec_size()
        }
    }
    impl Read for TGReadFile {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let line_num = Self::vec_num(buf.len());
            let mut pos: usize = 0;
            for i in 0..line_num {
                let mut content_buf = String::new();
                if self.0.read_line(&mut content_buf)? == 0 {
                    return Ok(0);
                }
                let content_buf = content_buf.trim().strip_prefix("0x").ok_or(into_io_error(
                    std::io::ErrorKind::InvalidData,
                    format!("line {}: {} is not valid hex data!", i, content_buf),
                ))?;
                let data = u32::from_str_radix(content_buf, 16)
                    .map_err(|e| into_io_error(std::io::ErrorKind::InvalidData, e))?
                    .to_le_bytes();
                buf[pos..pos + Self::vec_size()].copy_from_slice(&data);
                pos += Self::vec_size();
            }
            Ok(line_num * Self::vec_size())
        }
    }
    impl Write for TGReadFile {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.get_ref().write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.get_ref().flush()
        }
    }
    impl Seek for TGReadFile {
        fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
            Err(into_io_error(
                std::io::ErrorKind::PermissionDenied,
                "not support seek!",
            ))
        }
    }
    impl MBFile for TGReadFile {}

    struct TGWriteFile(File);
    impl TGWriteFile {
        const fn vec_size() -> usize {
            std::mem::size_of::<u32>()
        }
        const fn vec_num(buf_size: usize) -> usize {
            buf_size / Self::vec_size()
        }
    }
    impl Read for TGWriteFile {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.0.read(buf)
        }
    }
    impl Write for TGWriteFile {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let line_num = Self::vec_num(buf.len());
            let mut pos: usize = 0;
            for _ in 0..line_num {
                let mut data = [0u8; Self::vec_size()];
                data.copy_from_slice(&buf[pos..pos + Self::vec_size()]);
                writeln!(&mut self.0, "{:#x}", u32::from_le_bytes(data))?;
                pos += Self::vec_size();
            }
            Ok(line_num * Self::vec_size())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }
    impl Seek for TGWriteFile {
        fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
            Err(into_io_error(
                std::io::ErrorKind::PermissionDenied,
                "not support seek!",
            ))
        }
    }
    impl MBFile for TGWriteFile {}
    struct TGFileOpener;
    impl MBFileOpener for TGFileOpener {
        fn open(&self, path: &str, flags: u32) -> std::io::Result<MBFileType> {
            if flags & MB_FILE_READ != 0 {
                Ok(MBFileType::Special(Box::new(TGReadFile(BufReader::new(
                    mb_open_file(path, MB_FILE_READ)?,
                )))))
            } else {
                Ok(MBFileType::Special(Box::new(TGWriteFile(mb_open_file(
                    path,
                    MB_FILE_WRITE | MB_FILE_TRUNC,
                )?))))
            }
        }
    }

    #[test]
    fn mb_fs_special_file_test() {
        let fs = MBFs::with_special("resources", |table| {
            table
                .insert("tg".to_string(), Box::new(TGFileOpener))
                .map_or(Ok(()), |_| Err("tg exists!".to_string()))
        })
        .unwrap();
        let fd = fs.open("test.tg", MB_FILE_WRITE).unwrap();
        let resolver = MBLocalPtrResolver::default();
        let data: Vec<u32> = vec![0x12345678, 0x5a5a5a5a, 0xa5a5a5a5, 0xdeadbeef];
        let wlen = fs
            .write(&resolver, fd, data.as_ptr() as *const u8, data.len() * 4)
            .unwrap();
        println!("wlen = {}", wlen);
        assert_eq!(wlen, data.len() * 4);
        let data2: Vec<u32> = vec![0x87654321];
        let wlen = fs
            .write(&resolver, fd, data2.as_ptr() as *const u8, data2.len() * 4)
            .unwrap();
        println!("wlen = {}", wlen);
        assert_eq!(wlen, data2.len() * 4);
        fs.close(fd).unwrap();
        let fd = fs.open("test.tg", MB_FILE_READ).unwrap();
        let mut result: u32 = 0;
        for d in data.iter() {
            let rlen = fs
                .read(&resolver, fd, &mut result as *mut u32 as *mut u8, 4)
                .unwrap();
            assert_eq!(rlen, 4);
            println!("result = {:#x}", result);
            assert_eq!(result, *d);
        }
        let rlen = fs
            .read(&resolver, fd, &mut result as *mut u32 as *mut u8, 4)
            .unwrap();
        assert_eq!(rlen, 4);
        println!("result = {:#x}", result);
        assert_eq!(result, data2[0]);
        let rlen = fs
            .read(&resolver, fd, &mut result as *mut u32 as *mut u8, 4)
            .unwrap();
        println!("rlen = {}", rlen);
        let rlen = fs
            .read(&resolver, fd, &mut result as *mut u32 as *mut u8, 4)
            .unwrap();
        println!("rlen = {}", rlen);
        assert_eq!(rlen, 0);
        fs.close(fd).unwrap();
    }

    struct VirtFile {
        buffer: Vec<u8>,
        pos: usize,
    }
    impl Read for VirtFile {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let len = if self.buffer.len() - self.pos < buf.len() {
                self.buffer.len() - self.pos
            } else {
                buf.len()
            };
            buf[..len].copy_from_slice(&self.buffer[self.pos..self.pos + len]);
            self.pos += len;
            Ok(len)
        }
    }
    impl Write for VirtFile {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if buf.len() == 0 {
                return Ok(0);
            }
            self.buffer = [&self.buffer[..self.pos], &buf[..]].concat();
            self.pos += buf.len();
            println!("write:buffer = {:#x?}", self.buffer);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Seek for VirtFile {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            if let SeekFrom::Start(p) = pos {
                self.pos = if p as usize > self.buffer.len() {
                    self.buffer.len()
                } else {
                    p as usize
                };
                Ok(self.pos as u64)
            } else {
                Err(into_io_error(
                    std::io::ErrorKind::InvalidInput,
                    "not support seek current and end!",
                ))
            }
        }
    }
    impl MBFile for VirtFile {}
    struct VirtFileOpener;
    impl MBFileOpener for VirtFileOpener {
        fn open(&self, _path: &str, _flags: u32) -> std::io::Result<MBFileType> {
            Ok(MBFileType::Virtual(Box::new(VirtFile {
                buffer: vec![],
                pos: 0,
            })))
        }
    }

    #[test]
    fn mb_fs_virtual_test() {
        let fs = MBFs::with_virtual("resources", |table| {
            table
                .insert("virt/virt".to_string(), Box::new(VirtFileOpener))
                .map_or(Ok(()), |_| Err("virt exists!".to_string()))
        })
        .unwrap();
        let fd = fs
            .open("virtual/virt/virt", MB_FILE_WRITE | MB_FILE_READ)
            .unwrap();
        let resolver = MBLocalPtrResolver::default();
        let data = b"Hello World!";
        let wlen = fs.write(&resolver, fd, data.as_ptr(), data.len()).unwrap();
        println!("wlen = {}", wlen);
        assert_eq!(wlen, data.len());
        let mut buf = vec![0u8; wlen];
        let pos = fs.seek(fd, 0).unwrap();
        assert_eq!(pos, 0);
        let rlen = fs.read(&resolver, fd, buf.as_mut_ptr(), buf.len()).unwrap();
        fs.close(fd).unwrap();
        println!("rlen = {}", rlen);
        assert_eq!(rlen, wlen);
        assert_eq!(data[..], buf[..]);
        println!("content = {}", String::from_utf8(buf).unwrap());
    }
}
