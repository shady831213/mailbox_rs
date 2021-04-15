use async_std::future::Future;
use async_std::task::Context;
use async_std::task::Poll;
use async_std::task::Waker;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use crate::mb_channel::*;
use crate::mb_rpcs::*;

pub struct MBAsyncChannel<CH: MBChannelIf> {
    channel: CH,
    c_waker: Option<Waker>,
    s_waker: Option<Waker>,
}

impl<CH: MBChannelIf> MBAsyncChannel<CH> {
    pub fn new(ch: CH) -> MBAsyncChannel<CH> {
        MBAsyncChannel {
            channel: ch,
            c_waker: None,
            s_waker: None,
        }
    }
}

pub struct MBAsyncSender<CH: MBChannelIf>(Arc<Mutex<MBAsyncChannel<CH>>>);

impl<CH: MBChannelIf> MBAsyncSender<CH> {
    pub fn new(ch: &Arc<Mutex<MBAsyncChannel<CH>>>) -> MBAsyncSender<CH> {
        MBAsyncSender(ch.clone())
    }
    fn try_send<REQ: Copy, RPC: MBRpc<REQ = REQ>>(
        &self,
        rpc: &RPC,
        req: REQ,
        cx: &mut Context,
    ) -> Poll<()> {
        let mut ch = self.0.lock().unwrap();
        if !ch.channel.req_can_put() {
            ch.c_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        ch.channel.put_req(rpc, req);
        if let Some(w) = ch.s_waker.take() {
            w.wake();
        }
        Poll::Ready(())
    }
    fn try_recv<RESP, RPC: MBRpc<RESP = RESP>>(&self, rpc: &RPC, cx: &mut Context) -> Poll<RESP> {
        let mut ch = self.0.lock().unwrap();
        if !ch.channel.resp_can_get() {
            ch.c_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let ret = ch.channel.get_resp(rpc);
        if let Some(w) = ch.s_waker.take() {
            w.wake();
        }
        Poll::Ready(ret)
    }

    pub fn send_req<'a, REQ: 'a + Copy, RPC: 'a + MBRpc<REQ = REQ>>(
        &'a self,
        rpc: &'a RPC,
        req: REQ,
    ) -> impl Future<Output = ()> + 'a {
        let req_fut = MBAsyncSenderReq {
            sender: self,
            rpc: rpc,
            data: req,
        };
        async {
            req_fut.await;
            async_std::task::yield_now().await;
        }
    }
    pub fn recv_resp<'a, RESP: 'a, RPC: 'a + MBRpc<RESP = RESP>>(
        &'a self,
        rpc: &'a RPC,
    ) -> impl Future<Output = RESP> + 'a {
        let resp_fut = MBAsyncSenderResp {
            sender: self,
            rpc: rpc,
            _marker: PhantomData,
        };
        async {
            let resp = resp_fut.await;
            async_std::task::yield_now().await;
            resp
        }
    }
}

struct MBAsyncSenderReq<'a, REQ, RPC, CH: MBChannelIf> {
    sender: &'a MBAsyncSender<CH>,
    rpc: &'a RPC,
    data: REQ,
}

impl<'a, REQ: Copy, RPC: MBRpc<REQ = REQ>, CH: MBChannelIf> Future
    for MBAsyncSenderReq<'a, REQ, RPC, CH>
{
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let ret = self.sender.try_send(self.rpc, self.data, cx);
        ret
    }
}

struct MBAsyncSenderResp<'a, RESP, RPC, CH: MBChannelIf> {
    sender: &'a MBAsyncSender<CH>,
    rpc: &'a RPC,
    _marker: PhantomData<RESP>,
}

impl<'a, RESP, RPC: MBRpc<RESP = RESP>, CH: MBChannelIf> Future
    for MBAsyncSenderResp<'a, RESP, RPC, CH>
{
    type Output = RESP;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.sender.try_recv(self.rpc, cx)
    }
}

pub struct MBAsyncReceiver<CH: MBChannelIf>(Arc<Mutex<MBAsyncChannel<CH>>>);

impl<CH: MBChannelIf> MBAsyncReceiver<CH> {
    pub fn new(ch: &Arc<Mutex<MBAsyncChannel<CH>>>) -> MBAsyncReceiver<CH> {
        MBAsyncReceiver(ch.clone())
    }
    fn try_recv(&self, cx: &mut Context) -> Poll<MBReqEntry> {
        let mut ch = self.0.lock().unwrap();
        if !ch.channel.req_can_get() {
            ch.s_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        let req = ch.channel.get_req();
        if let Some(w) = ch.c_waker.take() {
            w.wake();
        }
        Poll::Ready(req)
    }

    fn try_send(&self, resp: MBRespEntry, cx: &mut Context) -> Poll<()> {
        let mut ch = self.0.lock().unwrap();
        if !ch.channel.resp_can_put() {
            ch.s_waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        ch.channel.put_resp(resp);
        if let Some(w) = ch.c_waker.take() {
            w.wake();
        }
        Poll::Ready(())
    }

    pub fn recv_req<'a>(&'a self) -> impl Future<Output = MBReqEntry> + 'a {
        let req_fut = MBAsyncReceiverReq { receiver: self };
        req_fut
    }

    pub fn send_resp<'a>(&'a self, resp: MBRespEntry) -> impl Future<Output = ()> + 'a {
        let resp_fut = MBAsyncReceiverResp {
            receiver: self,
            resp,
        };
        resp_fut
    }
}

struct MBAsyncReceiverReq<'a, CH: MBChannelIf> {
    receiver: &'a MBAsyncReceiver<CH>,
}

impl<'a, CH: MBChannelIf> Future for MBAsyncReceiverReq<'a, CH> {
    type Output = MBReqEntry;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.receiver.try_recv(cx)
    }
}

struct MBAsyncReceiverResp<'a, CH: MBChannelIf> {
    receiver: &'a MBAsyncReceiver<CH>,
    resp: MBRespEntry,
}

impl<'a, CH: MBChannelIf> Future for MBAsyncReceiverResp<'a, CH> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.receiver.try_send(self.resp, cx)
    }
}

pub struct MBAsyncWake<'a, CH: MBChannelIf>(&'a Arc<Mutex<MBAsyncChannel<CH>>>);

impl<'a, CH: MBChannelIf> Future for MBAsyncWake<'a, CH> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<Self::Output> {
        let mut ch = self.0.lock().unwrap();
        if let Some(w) = ch.s_waker.take() {
            w.wake();
        }
        if let Some(w) = ch.c_waker.take() {
            w.wake();
        }
        Poll::Ready(())
    }
}

impl<'a, CH: MBChannelIf> MBAsyncWake<'a, CH> {
    pub fn new(ch: &'a Arc<Mutex<MBAsyncChannel<CH>>>) -> MBAsyncWake<'a, CH> {
        MBAsyncWake(ch)
    }
}
