use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use super::share_mem::*;
use crate::mb_channel::*;
use crate::mb_rpcs::*;
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
        MB_CACHE_LINE.map_or(Self::entry_offset(MB_MAX_ENTRIES), |cache_line| {
            Self::entry_offset(MB_MAX_ENTRIES).next_multiple_of(cache_line as MBPtrT)
        })
    }

    fn entry_offset(idx: usize) -> MBPtrT {
        Self::idx_p_offset()
            + (std::mem::size_of::<u32>() + std::mem::size_of::<T>() * idx) as MBPtrT
    }

    fn idx_p(&self) -> u32 {
        let mut data: u32 = 0;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.idx_p_ptr(), &mut data);
        data
    }
    fn idx_c(&self) -> u32 {
        let mut data: u32 = 0;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.idx_c_ptr(), &mut data);
        data
    }

    fn idx_p_ptr(&self) -> MBPtrT {
        self.base + Self::idx_p_offset()
    }

    fn idx_c_ptr(&self) -> MBPtrT {
        self.base + Self::idx_c_offset()
    }

    fn clr_p(&self) {
        let next_p = 0;
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.idx_p_ptr(), &next_p);
    }
    fn clr_c(&self) {
        let next_c = 0;
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.idx_c_ptr(), &next_c);
    }
    fn flush_p_entry(&mut self) -> MBPtrT {
        let ptr = self.p_ptr();
        self.mem.lock().unwrap().write_sized(ptr, &self.cur_p_entry);
        ptr
    }

    fn p_ptr(&self) -> MBPtrT {
        let cur_p = self.idx_p_masked() as usize;
        self.base + Self::entry_offset(cur_p)
    }
    fn load_c_entry(&mut self) {
        let ptr = self.c_ptr();
        self.mem
            .lock()
            .unwrap()
            .read_sized(ptr, &mut self.cur_c_entry);
    }

    fn c_ptr(&self) -> MBPtrT {
        let cur_c = self.idx_c_masked() as usize;
        self.base + Self::entry_offset(cur_c)
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
            .write_sized(self.idx_p_ptr(), &next_p);
    }
    fn advance_c(&mut self) {
        let next_c = self.idx_c().wrapping_add(1);
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.idx_c_ptr(), &next_c);
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
            base + MB_CACHE_LINE.map_or(
                std::mem::size_of::<u32>() + std::mem::size_of::<MBState>(),
                |cache_line| {
                    (std::mem::size_of::<u32>() + std::mem::size_of::<MBState>())
                        .next_multiple_of(cache_line)
                },
            ) as MBPtrT,
            mem,
        );
        let resp_queue = MBQueueShareMem::<SM, MBRespEntry>::new(
            req_queue.base
                + MB_CACHE_LINE.map_or(std::mem::size_of::<MBQueue<MBReqEntry>>(), |cache_line| {
                    (std::mem::size_of::<MBQueue<MBReqEntry>>()).next_multiple_of(cache_line)
                }) as MBPtrT,
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
    pub fn with_elf(
        file: &str,
        mem: &Arc<Mutex<SM>>,
        load: bool,
        mb_id: usize,
    ) -> MBChannelShareMem<SM> {
        use xmas_elf::ElfFile;
        let mut mb_address: MBPtrT = 0;
        let f = |elf: &ElfFile, _: &str| -> Result<(), String> {
            if let Some(s) = elf.find_section_by_name(".mailbox") {
                let address = s.address()
                    + (MB_CACHE_LINE.map_or(std::mem::size_of::<MBChannel>(), |cache_line| {
                        std::mem::size_of::<MBChannel>().next_multiple_of(cache_line)
                    }) * mb_id) as u64;
                let sec_end = s.address() + s.size();
                if address + std::mem::size_of::<MBChannel>() as u64 > sec_end {
                    return Err(format!(
                        "mailbox id {} exceeds .mailbox section bound!",
                        mb_id
                    ));
                }
                mb_address = address as MBPtrT;
                Ok(())
            } else {
                Err("Can't get \".mailbox\" section!".to_string())
            }
        };
        if load {
            mem.lock().unwrap().load_elf_with(file, f).unwrap();
        } else {
            use crate::mb_std::utils::process_elf;
            process_elf(file, f).unwrap();
        }
        MBChannelShareMem::new(mb_address, mem)
    }

    fn state_offset(&self) -> MBPtrT {
        std::mem::size_of::<u32>() as MBPtrT
    }
}

impl<SM: MBShareMem> MBChannelIf for MBChannelShareMem<SM> {
    fn is_ready(&self) -> bool {
        let mut state: MBState = MBState::INIT;
        self.mem
            .lock()
            .unwrap()
            .read_sized(self.base + self.state_offset(), &mut state);
        state == MBState::READY
    }
    fn reset_req(&mut self) {
        let state = MBState::INIT;
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.base + self.state_offset(), &state);
        self.req_queue.clr_p();
        self.req_queue.clr_c();
        self.resp_queue.clr_p();
        self.resp_queue.clr_c();
    }
    fn reset_ready(&self) -> bool {
        self.req_queue.idx_p() == 0
            && self.req_queue.idx_c() == 0
            && self.resp_queue.idx_p() == 0
            && self.resp_queue.idx_c() == 0
    }
    fn reset_ack(&mut self) {
        let state = MBState::READY;
        self.mem
            .lock()
            .unwrap()
            .write_sized(self.base + self.state_offset(), &state);
    }

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
    fn put_req<REQ: Copy, M: MBRpc<REQ = REQ>>(&mut self, master: &M, req: REQ) -> MBPtrT {
        master.put_req(req, self.req_queue.cur_p_entry_mut());
        self.req_queue.flush_p_entry()
    }
    fn get_req(&mut self) -> MBReqEntry {
        *self.req_queue.cur_c_entry()
    }
    fn get_resp<RESP, M: MBRpc<RESP = RESP>>(&mut self, master: &M) -> RESP {
        master.get_resp(self.resp_queue.cur_c_entry())
    }
    fn put_resp(&mut self, resp: MBRespEntry) -> MBPtrT {
        *self.resp_queue.cur_p_entry_mut() = resp;
        self.resp_queue.flush_p_entry()
    }
    fn commit_req(&mut self) -> MBPtrT {
        self.req_queue.advance_p();
        self.req_queue.idx_p_ptr()
    }
    fn ack_req(&mut self) -> MBPtrT {
        self.req_queue.advance_c();
        self.req_queue.idx_c_ptr()
    }
    fn ack_resp(&mut self) -> MBPtrT {
        self.resp_queue.advance_c();
        self.resp_queue.idx_c_ptr()
    }
    fn commit_resp(&mut self) -> MBPtrT {
        self.resp_queue.advance_p();
        self.resp_queue.idx_p_ptr()
    }
}
