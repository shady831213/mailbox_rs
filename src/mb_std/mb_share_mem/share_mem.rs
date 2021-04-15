use crate::mb_rpcs::*;
use std::sync::Arc;
use std::sync::Mutex;

use crate::mb_std::utils::*;
use xmas_elf::program;
use xmas_elf::ElfFile;

pub struct MBShareMemSpace<M: MBShareMemBlock> {
    mems: Vec<Arc<Mutex<M>>>,
}
impl<M: MBShareMemBlock> MBShareMemSpace<M> {
    pub fn new() -> MBShareMemSpace<M> {
        MBShareMemSpace { mems: vec![] }
    }
    pub fn add_mem(&mut self, mem: &Arc<Mutex<M>>) -> Result<(), &'static str> {
        let m = mem.lock().unwrap();
        if self.find_mem_by_addr(m.base()).is_some()
            || self.find_mem_by_addr(m.end_addr()).is_some()
        {
            return Err("Overlapped!");
        }
        Ok(self.mems.push(mem.clone()))
    }
    fn find_mem_by_addr(&self, addr: MBPtrT) -> Option<&Arc<Mutex<M>>> {
        self.mems.iter().find(|m| {
            let mem = m.lock().unwrap();
            mem.in_range(addr)
        })
    }
}
impl<M: MBShareMemBlock> MBShareMem for MBShareMemSpace<M> {
    fn write(&mut self, addr: MBPtrT, data: &[u8]) -> usize {
        if let Some(m) = self.find_mem_by_addr(addr) {
            m.lock().unwrap().write(addr, data)
        } else {
            0
        }
    }
    fn read(&self, addr: MBPtrT, data: &mut [u8]) -> usize {
        if let Some(m) = self.find_mem_by_addr(addr) {
            m.lock().unwrap().read(addr, data)
        } else {
            0
        }
    }
}
pub trait MBShareMemBlock: MBShareMem {
    fn base(&self) -> MBPtrT;
    fn size(&self) -> MBPtrT;
    fn end_addr(&self) -> MBPtrT {
        (self.size() - 1) + self.base()
    }
    fn in_range(&self, addr: MBPtrT) -> bool {
        addr >= self.base() && addr <= self.end_addr()
    }
}
pub trait MBShareMem {
    fn write(&mut self, addr: MBPtrT, data: &[u8]) -> usize;
    fn read(&self, addr: MBPtrT, data: &mut [u8]) -> usize;
    fn write_sized<T: Sized>(&mut self, addr: MBPtrT, data: &T) {
        let len = self.write(addr, unsafe {
            std::slice::from_raw_parts((data as *const T) as *const u8, std::mem::size_of::<T>())
        });
        if len != std::mem::size_of::<T>() {
            panic!("write_sized @ {:#x} misatched!", addr)
        }
    }

    fn read_sized<T: Sized>(&self, addr: MBPtrT, data: &mut T) {
        let len = self.read(addr, unsafe {
            std::slice::from_raw_parts_mut((data as *mut T) as *mut u8, std::mem::size_of::<T>())
        });
        if len != std::mem::size_of::<T>() {
            panic!("read_sized @ {:#x} misatched!", addr)
        }
    }

    fn write_slice<T: Sized>(&mut self, addr: MBPtrT, data: &[T]) {
        let len = self.write(addr, unsafe {
            std::slice::from_raw_parts(
                (data.as_ptr() as *const T) as *const u8,
                std::mem::size_of::<T>() * data.len(),
            )
        });
        if len != std::mem::size_of::<T>() * data.len() {
            panic!("write_slice @ {:#x} misatched!", addr)
        }
    }

    fn read_slice<T: Sized>(&self, addr: MBPtrT, data: &mut [T]) {
        let len = self.read(addr, unsafe {
            std::slice::from_raw_parts_mut(
                (data.as_mut_ptr() as *mut T) as *mut u8,
                std::mem::size_of::<T>() * data.len(),
            )
        });
        if len != std::mem::size_of::<T>() * data.len() {
            panic!("read_slice @ {:#x} misatched!", addr)
        }
    }

    fn load_elf(&mut self, file: &str) -> Result<(), String> {
        self.load_elf_with(file, |_, _| Ok(()))
    }

    fn load_elf_with<F: FnMut(&ElfFile, &str) -> Result<(), String>>(
        &mut self,
        file: &str,
        mut f: F,
    ) -> Result<(), String> {
        process_elf(file, |elf, file| {
            f(elf, file)?;
            elf.program_iter().for_each(|p| {
                if let Ok(program::Type::Load) = p.get_type() {
                    if let Ok(program::SegmentData::Undefined(d)) = p.get_data(&elf) {
                        let addr = p.virtual_addr() as MBPtrT;
                        println!(
                            "load elf {} segment({}) @ {:#x} - {:#x}!",
                            file,
                            d.len() as MBPtrT,
                            addr,
                            addr + d.len() as MBPtrT
                        );
                        self.write(addr, d);
                    }
                }
            });
            Ok(())
        })
    }
}
