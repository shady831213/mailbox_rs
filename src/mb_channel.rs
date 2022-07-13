use crate::mb_rpcs::*;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[repr(u32)]
pub enum MBState {
    INIT = 0,
    READY = 1,
}

impl Default for MBState {
    fn default() -> Self {
        MBState::INIT
    }
}

pub const MB_MAX_ARGS: usize = 8;
pub const MB_MAX_ENTRIES: usize = 8;

pub const fn idx_masked(ptr: u32) -> u32 {
    ptr & ((1 << (MB_MAX_ENTRIES.trailing_zeros() as u32)) - 1)
}

pub const fn idx_flag(ptr: u32) -> bool {
    (ptr >> (MB_MAX_ENTRIES.trailing_zeros() as u32)) & 0x1 == 0
}

macro_rules! io_read32 {
    ($ptr:expr) => {
        unsafe { ($ptr as *const u32).read_volatile() }
    };
}
macro_rules! io_write32 {
    ($ptr:expr, $value:expr) => {
        unsafe { ($ptr as *mut u32).write_volatile($value as u32) }
    };
}

macro_rules! io_read_mbptr {
    ($ptr:expr) => {
        unsafe { ($ptr as *const MBPtrT).read_volatile() }
    };
}

macro_rules! io_write_mbptr {
    ($ptr:expr, $value:expr) => {
        unsafe { ($ptr as *mut MBPtrT).write_volatile($value as MBPtrT) }
    };
}

#[derive(Default, Debug, Copy)]
#[repr(C)]
pub struct MBReqEntry {
    pub action: MBAction,
    pub words: u32,
    pub args: [MBPtrT; MB_MAX_ARGS],
}

impl MBReqEntry {
    pub fn set_action(&mut self, v: MBAction) {
        io_write32!(&mut self.action as *mut MBAction, v)
    }
    pub fn set_words(&mut self, v: u32) {
        io_write32!(&mut self.words, v)
    }
    pub fn set_args(&mut self, i: usize, v: MBPtrT) {
        io_write_mbptr!(&mut self.args[i], v)
    }
}

impl Clone for MBReqEntry {
    fn clone(&self) -> Self {
        let mut entry = MBReqEntry {
            words: io_read32!(&self.words),
            action: MBAction::from(io_read32!(&self.action as *const MBAction)),
            args: [0; MB_MAX_ARGS],
        };
        for i in 0..MB_MAX_ARGS {
            let v = io_read_mbptr!(&self.args[i]);
            io_write_mbptr!(&mut entry.args[i], v);
        }
        entry
    }
}

#[derive(Default, Debug, Copy)]
#[repr(C)]
pub struct MBRespEntry {
    pub words: u32,
    pub rets: MBPtrT,
}

impl MBRespEntry {
    pub fn get_rets(&self) -> MBPtrT {
        io_read_mbptr!(&self.rets)
    }
}

impl Clone for MBRespEntry {
    fn clone(&self) -> Self {
        MBRespEntry {
            words: io_read32!(&self.words),
            rets: io_read_mbptr!(&self.rets),
        }
    }
}

pub trait MBQueueIf<T> {
    fn idx_p_masked(&self) -> u32;
    fn idx_c_masked(&self) -> u32;
    fn idx_p_flag(&self) -> bool;
    fn idx_c_flag(&self) -> bool;
    fn full(&self) -> bool {
        self.idx_c_masked() == self.idx_p_masked() && self.idx_c_flag() != self.idx_p_flag()
    }
    fn empty(&self) -> bool {
        self.idx_c_masked() == self.idx_p_masked() && self.idx_c_flag() == self.idx_p_flag()
    }
    fn cur_p_entry_mut(&mut self) -> &mut T;
    fn cur_c_entry(&mut self) -> &T;
    fn advance_p(&mut self);
    fn advance_c(&mut self);
}

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct MBQueue<T> {
    id: u32,
    idx_p: u32,
    idx_c: u32,
    queue: [T; MB_MAX_ENTRIES],
}

impl<T> MBQueueIf<T> for MBQueue<T> {
    fn idx_p_masked(&self) -> u32 {
        idx_masked(io_read32!(&self.idx_p))
    }
    fn idx_c_masked(&self) -> u32 {
        idx_masked(io_read32!(&self.idx_c))
    }
    fn idx_p_flag(&self) -> bool {
        idx_flag(io_read32!(&self.idx_p))
    }
    fn idx_c_flag(&self) -> bool {
        idx_flag(io_read32!(&self.idx_c))
    }
    fn cur_p_entry_mut(&mut self) -> &mut T {
        &mut self.queue[self.idx_p_masked() as usize]
    }
    fn cur_c_entry(&mut self) -> &T {
        &self.queue[self.idx_c_masked() as usize]
    }
    fn advance_p(&mut self) {
        let v = io_read32!(&self.idx_p).wrapping_add(1);
        io_write32!(&mut self.idx_p, v);
    }
    fn advance_c(&mut self) {
        let v = io_read32!(&self.idx_c).wrapping_add(1);
        io_write32!(&mut self.idx_c, v);
    }
}

pub trait MBChannelIf {
    fn reset_req(&mut self);
    fn reset_ack(&mut self);
    fn reset_ready(&self) -> bool;
    fn is_ready(&self) -> bool;
    fn req_can_get(&self) -> bool;
    fn req_can_put(&self) -> bool;
    fn resp_can_get(&self) -> bool;
    fn resp_can_put(&self) -> bool;
    fn put_req<REQ: Copy, M: MBRpc<REQ = REQ>>(&mut self, master: &M, req: REQ);
    fn get_req(&mut self) -> MBReqEntry;
    fn get_resp<RESP, M: MBRpc<RESP = RESP>>(&mut self, master: &M) -> RESP;
    fn put_resp(&mut self, resp: MBRespEntry);
}
#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct MBChannel {
    id: u32,
    state: MBState,
    req_queue: MBQueue<MBReqEntry>,
    resp_queue: MBQueue<MBRespEntry>,
}

impl MBChannel {
    pub const fn const_init() -> MBChannel {
        MBChannel {
            id: 0,
            state: MBState::INIT,
            req_queue: MBQueue::<MBReqEntry> {
                id: 0,
                idx_p: 0,
                idx_c: 0,
                queue: [MBReqEntry {
                    words: 0,
                    action: MBAction::IDLE,
                    args: [0; MB_MAX_ARGS],
                }; MB_MAX_ENTRIES],
            },
            resp_queue: MBQueue::<MBRespEntry> {
                id: 0,
                idx_p: 0,
                idx_c: 0,
                queue: [MBRespEntry { words: 0, rets: 0 }; MB_MAX_ENTRIES],
            },
        }
    }
}

impl MBChannelIf for MBChannel {
    fn is_ready(&self) -> bool {
        io_read32!(&self.state as *const MBState) == MBState::READY as u32
    }
    fn reset_req(&mut self) {
        io_write32!(&mut self.state as *mut MBState, MBState::INIT);
        io_write32!(&mut self.req_queue.idx_p, 0);
        io_write32!(&mut self.req_queue.idx_c, 0);
        io_write32!(&mut self.resp_queue.idx_p, 0);
        io_write32!(&mut self.resp_queue.idx_c, 0);
    }
    fn reset_ready(&self) -> bool {
        io_read32!(&self.req_queue.idx_p) == 0
            && io_read32!(&self.req_queue.idx_c) == 0
            && io_read32!(&self.resp_queue.idx_p) == 0
            && io_read32!(&self.resp_queue.idx_p) == 0
    }
    fn reset_ack(&mut self) {
        io_write32!(&mut self.state as *mut MBState, MBState::READY);
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
    fn put_req<REQ: Copy, M: MBRpc<REQ = REQ>>(&mut self, master: &M, req: REQ) {
        master.put_req(req, self.req_queue.cur_p_entry_mut());
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
        self.resp_queue.advance_p();
    }
}
