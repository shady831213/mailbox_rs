#[cfg(feature = "ptr32")]
pub type MBPtrT = u32;
#[cfg(feature = "ptr64")]
pub type MBPtrT = u64;
#[cfg(feature = "ptrhost")]
pub type MBPtrT = usize;

#[derive(Debug, Copy, Clone)]
#[repr(u32)]
pub enum MBAction {
    IDLE = 0,
    EXIT = 1,
    PRINT = 2,
    CPRINT = 3,
    MEMMOVE = 4,
    MEMSET = 5,
    MEMCMP = 6,
    SVCALL = 7,
    OTHER = 0x80000000,
}

impl Default for MBAction {
    fn default() -> Self {
        MBAction::IDLE
    }
}

#[derive(Debug)]
#[repr(u32)]
pub enum MBState {
    INIT = 0,
    PREADY = 1,
    CREADY = 2,
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

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct MBReqEntry {
    pub action: MBAction,
    pub words: u32,
    pub args: [MBPtrT; MB_MAX_ARGS],
}

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct MBRespEntry {
    pub words: u32,
    pub rets: MBPtrT,
}

pub trait MBRpc {
    type REQ;
    type RESP;
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry);
    fn get_resp(&self, entry: &MBRespEntry) -> Self::RESP;
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

#[derive(Default, Debug)]
#[repr(C)]
pub struct MBQueue<T> {
    id: u32,
    idx_p: u32,
    idx_c: u32,
    queue: [T; MB_MAX_ENTRIES],
}

impl<T> MBQueueIf<T> for MBQueue<T> {
    fn idx_p_masked(&self) -> u32 {
        idx_masked(self.idx_p)
    }
    fn idx_c_masked(&self) -> u32 {
        idx_masked(self.idx_c)
    }
    fn idx_p_flag(&self) -> bool {
        idx_flag(self.idx_p)
    }
    fn idx_c_flag(&self) -> bool {
        idx_flag(self.idx_c)
    }
    fn cur_p_entry_mut(&mut self) -> &mut T {
        &mut self.queue[self.idx_p_masked() as usize]
    }
    fn cur_c_entry(&mut self) -> &T {
        &self.queue[self.idx_c_masked() as usize]
    }
    fn advance_p(&mut self) {
        self.idx_p = self.idx_p.wrapping_add(1);
    }
    fn advance_c(&mut self) {
        self.idx_c = self.idx_c.wrapping_add(1);
    }
}

pub trait MBChannelIf {
    fn req_can_get(&self) -> bool;
    fn req_can_put(&self) -> bool;
    fn resp_can_get(&self) -> bool;
    fn resp_can_put(&self) -> bool;
    fn put_req<REQ: Copy, M: MBRpc<REQ = REQ>>(&mut self, master: &M, req: REQ);
    fn get_req(&mut self) -> MBReqEntry;
    fn get_resp<RESP, M: MBRpc<RESP = RESP>>(&mut self, master: &M) -> RESP;
    fn put_resp(&mut self, resp: MBRespEntry);
}
#[derive(Default, Debug)]
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
