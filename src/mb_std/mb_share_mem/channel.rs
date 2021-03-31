use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use super::share_mem::*;
use crate::mb_channel::*;
#[derive(Debug)]
#[repr(C)]
pub struct MBQueueShareMem<SM: MBShareMem, T: Sized + Default + Debug> {
    base: MBPtrT,
    mem: Arc<Mutex<SM>>,
    cur_p_entry: T,
    cur_c_entry: T,
}
impl<SM: MBShareMem, T: Sized + Default + Debug> MBQueueShareMem<SM, T> {
    pub fn new(base: MBPtrT, mem: &Arc<Mutex<SM>>) -> MBQueueShareMem<SM, T> {
        MBQueueShareMem {
            base,
            mem: mem.clone(),
            cur_p_entry: T::default(),
            cur_c_entry: T::default(),
        }
    }
    fn idx_p_offset() -> MBPtrT {
        std::mem::size_of::<u32>() as MBPtrT
    }

    fn idx_c_offset() -> MBPtrT {
        Self::idx_p_offset() + std::mem::size_of::<u32>() as MBPtrT
    }

    fn entry_offset(idx: usize) -> MBPtrT {
        Self::idx_c_offset()
            + (std::mem::size_of::<u32>() + std::mem::size_of::<T>() * idx) as MBPtrT
    }

    fn idx_p(&self) -> u32 {
        let mut data: u32 = 0;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.base + Self::idx_p_offset(), &mut data);
        data
    }
    fn idx_c(&self) -> u32 {
        let mut data: u32 = 0;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.base + Self::idx_c_offset(), &mut data);
        data
    }
    fn flush_p_entry(&mut self) {
        let cur_p = self.idx_p_masked() as usize;
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.base + Self::entry_offset(cur_p), &self.cur_p_entry);
    }
    fn load_p_entry(&mut self) {
        let cur_p = self.idx_p_masked() as usize;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.base + Self::entry_offset(cur_p), &mut self.cur_p_entry);
    }
    fn load_c_entry(&mut self) {
        let cur_c = self.idx_c_masked() as usize;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.base + Self::entry_offset(cur_c), &mut self.cur_c_entry);
    }
}

impl<SM: MBShareMem, T: Sized + Default + Debug> MBQueueIf<T> for MBQueueShareMem<SM, T> {
    fn idx_p_masked(&self) -> u32 {
        idx_masked(self.idx_p())
    }
    fn idx_c_masked(&self) -> u32 {
        idx_masked(self.idx_c())
    }
    fn idx_p_flag(&self) -> bool {
        idx_flag(self.idx_p())
    }
    fn idx_c_flag(&self) -> bool {
        idx_flag(self.idx_c())
    }
    fn cur_p_entry_mut(&mut self) -> &mut T {
        self.load_p_entry();
        &mut self.cur_p_entry
    }
    fn cur_c_entry(&mut self) -> &T {
        self.load_c_entry();
        &self.cur_c_entry
    }
    fn advance_p(&mut self) {
        let next_p = self.idx_p().wrapping_add(1);
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.base + Self::idx_p_offset(), &next_p);
    }
    fn advance_c(&mut self) {
        let next_c = self.idx_c().wrapping_add(1);
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.base + Self::idx_c_offset(), &next_c);
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct MBChannelShareMem<SM: MBShareMem> {
    base: MBPtrT,
    mem: Arc<Mutex<SM>>,
    req_queue: MBQueueShareMem<SM, MBReqEntry>,
    resp_queue: MBQueueShareMem<SM, MBRespEntry>,
}

impl<SM: MBShareMem> MBChannelShareMem<SM> {
    pub fn new(base: MBPtrT, mem: &Arc<Mutex<SM>>) -> MBChannelShareMem<SM> {
        let req_queue = MBQueueShareMem::<SM, MBReqEntry>::new(
            base + ((std::mem::size_of::<u32>() + std::mem::size_of::<MBState>()) as MBPtrT),
            mem,
        );
        let resp_queue = MBQueueShareMem::<SM, MBRespEntry>::new(
            req_queue.base + std::mem::size_of::<MBQueue<MBReqEntry>>() as MBPtrT,
            mem,
        );
        //clear share memory
        let req_queue_default = MBQueue::<MBReqEntry>::default();
        mem.lock()
            .unwrap()
            .write_sized(req_queue.base, &req_queue_default);
        let resp_queue_default = MBQueue::<MBRespEntry>::default();
        mem.lock()
            .unwrap()
            .write_sized(resp_queue.base, &resp_queue_default);
        MBChannelShareMem {
            base,
            mem: mem.clone(),
            req_queue,
            resp_queue,
        }
    }
    pub fn with_elf(file: &str, mem: &Arc<Mutex<SM>>, load: bool) -> MBChannelShareMem<SM> {
        use xmas_elf::ElfFile;
        let mut base: MBPtrT = 0;
        let f = |elf: &ElfFile, _: &str| -> Result<(), String> {
            if let Some(s) = elf.find_section_by_name(".mailbox") {
                base = s.address() as MBPtrT;
                Ok(())
            } else {
                Err("Can't get \".mailbox\" section!".to_string())
            }
        };
        if load {
            mem.lock().unwrap().load_elf_with(file, f).unwrap();
        } else {
            use super::utils::process_elf;
            process_elf(file, f).unwrap();
        }
        MBChannelShareMem::new(base, mem)
    }
}

impl<SM: MBShareMem> MBChannelIf for MBChannelShareMem<SM> {
    fn req_can_get(&self) -> bool {
        !self.req_queue.empty()
    }
    fn req_can_put(&self) -> bool {
        !self.req_queue.full()
    }
    fn resp_can_get(&self) -> bool {
        !self.resp_queue.empty()
    }
    fn resp_can_put(&self) -> bool {
        !self.resp_queue.full()
    }
    fn put_req<REQ: Copy, M: MBRpc<REQ = REQ>>(&mut self, master: &M, req: REQ) {
        master.put_req(req, self.req_queue.cur_p_entry_mut());
        self.req_queue.flush_p_entry();
        self.req_queue.advance_p();
    }
    fn get_req(&mut self) -> MBReqEntry {
        let entry = *self.req_queue.cur_c_entry();
        self.req_queue.advance_c();
        entry
    }
    fn get_resp<RESP, M: MBRpc<RESP = RESP>>(&mut self, master: &M) -> RESP {
        let ret = master.get_resp(self.resp_queue.cur_c_entry());
        self.resp_queue.advance_c();
        ret
    }
    fn put_resp(&mut self, resp: MBRespEntry) {
        *self.resp_queue.cur_p_entry_mut() = resp;
        self.resp_queue.flush_p_entry();
        self.resp_queue.advance_p();
    }
}
