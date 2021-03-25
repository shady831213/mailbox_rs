pub extern crate async_std;
pub extern crate futures;
pub extern crate shellexpand;
pub extern crate xmas_elf;
mod mb_async_channel;
mod mb_rpcs;
mod mb_server;
mod mb_share_mem;
pub use mb_async_channel::*;
pub use mb_rpcs::*;
pub use mb_server::*;
pub use mb_share_mem::*;
#[cfg(test)]
mod tests {
    use super::mb_async_channel::*;
    use super::mb_rpcs::*;
    use super::mb_server::*;
    use crate::mb_channel::*;
    use async_std::future::Future;
    use async_std::task::Context;
    use async_std::task::Poll;
    use std::sync::Arc;
    use std::sync::Mutex;
    #[no_mangle]
    extern "C" fn __mb_exit(code: MBPtrT) {
        println!("EXIT {}!", code)
    }

    #[test]
    fn mb_std_basic() {
        let channel = Arc::new(Mutex::new(MBAsyncChannel::new(MBChannel::default())));
        let server = MBLocalServer::new("server");
        let sender = MBAsyncSender::new(&channel);
        let receiver = MBAsyncReceiver::new(&channel);
        async_std::task::block_on(async {
            let c = async_std::task::spawn(async move {
                for i in 0..20 {
                    mb_exit(&sender, i as u32).await;
                    let msg = format!("abc {}!\n", i);
                    mb_print(&sender, &msg).await;
                    println!("Print done!");
                }
            });
            async_std::task::spawn(async move {
                loop {
                    let req = receiver.recv_req().await;
                    let mut resp = server.do_cmd(&req).await;
                    if let Some(r) = resp.take() {
                        receiver.send_resp(r).await;
                    }
                }
            });
            c.await;
        })
    }

    use super::mb_share_mem::*;
    struct ShareMem {
        base: usize,
        mem: Vec<u8>,
    }
    impl ShareMem {
        fn new(base: usize, size: usize) -> ShareMem {
            ShareMem {
                base,
                mem: vec![0; size],
            }
        }
    }
    impl MBShareMem for ShareMem {
        fn write(&mut self, addr: MBPtrT, data: &[u8]) -> usize {
            let offset = addr as usize - self.base;
            let len = if offset + data.len() > self.mem.len() {
                offset + data.len() - self.mem.len()
            } else {
                data.len()
            };
            self.mem[offset..offset + len].copy_from_slice(&data[..len]);
            len
        }
        fn read(&self, addr: MBPtrT, data: &mut [u8]) -> usize {
            let offset = addr as usize - self.base;
            let len = if offset + data.len() > self.mem.len() {
                offset + data.len() - self.mem.len()
            } else {
                data.len()
            };
            &mut data[..len].copy_from_slice(&self.mem[offset..offset + len]);
            len
        }
    }

    #[test]
    fn mb_std_share_mem() {
        let share_mem = Arc::new(Mutex::new(ShareMem::new(0, 1024)));
        let channel = Arc::new(Mutex::new(MBAsyncChannel::new(MBChannelShareMem::new(
            0, &share_mem,
        ))));
        let server = MBLocalServer::new("server");
        let sender = MBAsyncSender::new(&channel);
        let receiver = MBAsyncReceiver::new(&channel);
        async_std::task::block_on(async {
            let c = async_std::task::spawn(async move {
                for i in 0..20 {
                    mb_exit(&sender, i as u32).await;
                    println!("send req");
                    let msg = format!("abc {}!\n", i);
                    mb_print(&sender, &msg).await;
                    println!("Print done!");
                }
            });
            async_std::task::spawn(async move {
                loop {
                    let req = receiver.recv_req().await;
                    let mut resp = server.do_cmd(&req).await;
                    if let Some(r) = resp.take() {
                        receiver.send_resp(r).await;
                    }
                }
            });
            c.await;
        })
    }

    #[test]
    fn mb_cprint_test() {
        let channel = Arc::new(Mutex::new(MBAsyncChannel::new(MBChannel::default())));
        let server = MBLocalServer::new("server");
        let sender = MBAsyncSender::new(&channel);
        let receiver = MBAsyncReceiver::new(&channel);
        async_std::task::block_on(async {
            let c = async_std::task::spawn(async move {
                for i in 0..20 {
                    let file = "mb_cprint_test\0";
                    let pos = line!();
                    let fmt_str =
                        "mb_cprint_test \\ \\% %d, %x, %f, %s, %d, %d, %d, %d, %d, %d!\n\0";
                    let s = "my s\0";
                    let args: Vec<MBPtrT> = vec![
                        i as MBPtrT,
                        0xdead as MBPtrT,
                        1.2345_f32.to_bits() as MBPtrT,
                        s.as_ptr() as MBPtrT,
                        4,
                        5,
                        6,
                        7,
                        8,
                        9,
                    ];
                    println!("Print begin!");
                    mb_cprint(&sender, &fmt_str, &file, pos, &args).await;
                    println!("Print done!");
                }
                mb_print(&sender, "done!\n").await;
            });
            async_std::task::spawn(async move {
                loop {
                    let req = receiver.recv_req().await;
                    let mut resp = server.do_cmd(&req).await;
                    if let Some(r) = resp.take() {
                        receiver.send_resp(r).await;
                    }
                }
            });
            c.await;
        })
    }

    struct MyCustomRPC;
    impl<RA: MBPtrReader, R: MBPtrResolver<READER = RA>> MBAsyncRPC<RA, R> for MyCustomRPC {
        fn poll_cmd(
            &self,
            server_name: &str,
            _r: &R,
            req: &MBReqEntry,
            _cx: &mut Context,
        ) -> Poll<Option<MBRespEntry>> {
            println!("{} this is MyCustomRPC code:{}!", server_name, req.args[1]);
            let mut resp = MBRespEntry::default();
            resp.words = 1;
            resp.rets = req.args[1];
            Poll::Ready(Some(resp))
        }
    }
    impl<RA: MBPtrReader, R: MBPtrResolver<READER = RA>> CustomAsycRPC<RA, R> for MyCustomRPC {
        fn is_me(&self, action: u32) -> bool {
            action == 0x8
        }
    }
    impl MBRpc for MyCustomRPC {
        type REQ = u32;
        type RESP = u32;
        fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
            entry.words = 2;
            entry.action = MBAction::OTHER;
            entry.args[0] = 8;
            entry.args[1] = req as MBPtrT;
        }
        fn get_resp(&self, resp: &MBRespEntry) -> Self::RESP {
            resp.rets as u32
        }
    }
    fn mb_custom<CH: MBChannelIf>(
        sender: &MBAsyncSender<CH>,
        code: u32,
    ) -> impl Future<Output = u32> + '_ {
        async move {
            sender.send_req(&MyCustomRPC, code).await;
            sender.recv_resp(&MyCustomRPC).await
        }
    }
    #[test]
    fn custom_rpc_test() {
        let channel = Arc::new(Mutex::new(MBAsyncChannel::new(MBChannel::default())));
        let server = MBLocalServer::new("server");
        server.add_cmd(MyCustomRPC);
        let sender = MBAsyncSender::new(&channel);
        let receiver = MBAsyncReceiver::new(&channel);
        async_std::task::block_on(async {
            let c = async_std::task::spawn(async move {
                for i in 0..20 {
                    println!("mb_custom:{}", mb_custom(&sender, i as u32).await);
                }
            });
            async_std::task::spawn(async move {
                loop {
                    let req = receiver.recv_req().await;
                    let mut resp = server.do_cmd(&req).await;
                    if let Some(r) = resp.take() {
                        receiver.send_resp(r).await;
                    }
                }
            });
            c.await;
        })
    }
}
