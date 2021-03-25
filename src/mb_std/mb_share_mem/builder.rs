extern crate yaml_rust;
use crate::mb_channel::*;
use crate::mb_std::*;
use async_std::future::Future;
use futures::future::join_all;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
pub use yaml_rust::{Yaml, YamlLoader};
pub struct MBShareMemSpaceBuilder<M: MBShareMemBlock, P: MBShareMemParser<MemType = M>> {
    docs: Vec<Yaml>,
    parser: P,
    shared: HashMap<String, Arc<Mutex<M>>>,
    spaces: HashMap<String, Arc<Mutex<MBShareMemSpace<M>>>>,
}
impl<M: MBShareMemBlock, P: MBShareMemParser<MemType = M>> MBShareMemSpaceBuilder<M, P> {
    pub fn new(file: &str) -> Result<MBShareMemSpaceBuilder<M, P>, String> {
        let file_expand = shellexpand::full(file)
            .map_err(|e| e.to_string())?
            .to_string();
        let s = fs::read_to_string(file_expand).map_err(|e| e.to_string())?;
        Self::from_str(&s)
    }

    pub fn from_str(s: &str) -> Result<MBShareMemSpaceBuilder<M, P>, String> {
        Ok(MBShareMemSpaceBuilder {
            docs: YamlLoader::load_from_str(s).map_err(|e| e.to_string())?,
            parser: P::default(),
            shared: HashMap::new(),
            spaces: HashMap::new(),
        })
    }

    pub fn build_shared(mut self) -> Result<MBShareMemSpaceBuilder<M, P>, String> {
        if let Yaml::Hash(ref mems) = self.docs[0]["shared"] {
            for (key, mem) in mems.iter() {
                let k = key.as_str().unwrap();
                self.shared.insert(
                    k.to_string(),
                    Arc::new(Mutex::new(self.parser.parse(k, mem)?)),
                );
            }
        }
        Ok(self)
    }

    pub fn build_spaces(
        mut self,
    ) -> Result<HashMap<String, Arc<Mutex<MBShareMemSpace<M>>>>, String> {
        if let Yaml::Hash(ref spaces) = self.docs[0]["space"] {
            for (key, space) in spaces.iter() {
                let k = key.as_str().unwrap();
                let s = space
                    .as_vec()
                    .ok_or(format!("{:?}: mem space should be array!", k))?;
                let mut mem_space = MBShareMemSpace::<M>::new();
                for y in s.iter() {
                    match y {
                        Yaml::Hash(m) => {
                            let (name, v) = m.front().unwrap();
                            let n = name.as_str().unwrap();
                            mem_space
                                .add_mem(&Arc::new(Mutex::new(self.parser.parse(n, v)?)))
                                .map_err(|_| {
                                    format!("{:?}: {:?} is overlapped with other memory!", k, n)
                                })?;
                        }
                        Yaml::String(m) => mem_space
                            .add_mem(
                                self.shared
                                    .get(m)
                                    .ok_or(format!("{:?}: Can't get shared mem {:?}!", k, m))?,
                            )
                            .map_err(|_| {
                                format!("{:?}: {:?} is overlapped with other memory!", k, m)
                            })?,
                        _ => return Err(format!("{:?}: Invalid type {:?}!", k, y)),
                    }
                }
                self.spaces
                    .insert(k.to_string(), Arc::new(Mutex::new(mem_space)));
            }
            Ok(self.spaces)
        } else {
            Err("No space found in memory cfg file!".to_string())
        }
    }
}

pub trait MBShareMemParser: Default {
    type MemType: MBShareMemBlock;
    fn parse(&self, key: &str, doc: &Yaml) -> Result<Self::MemType, String>;
}

pub struct MBChannelShareMemSys<SM: MBShareMem> {
    chs: HashMap<String, Arc<Mutex<MBAsyncChannel<MBChannelShareMem<SM>>>>>,
}
impl<SM: MBShareMem> MBChannelShareMemSys<SM> {
    fn new() -> MBChannelShareMemSys<SM> {
        MBChannelShareMemSys {
            chs: HashMap::new(),
        }
    }

    pub fn wake<F: Fn() + 'static>(&self, tick: F) -> impl Future<Output = ()> + '_ {
        async move {
            loop {
                let wakers = self
                    .chs
                    .values()
                    .map(|ch| MBAsyncWake::new(ch))
                    .collect::<Vec<_>>();
                join_all(wakers).await;
                async_std::task::yield_now().await;
                tick();
            }
        }
    }

    pub fn serve<F: Fn(&MBSMServer<SM>)>(
        &self,
        space_map: &HashMap<String, Arc<Mutex<SM>>>,
        server_callback: F,
    ) -> impl Future<Output = Vec<()>> + '_ {
        let futures = self
            .chs
            .iter()
            .map(|ch| {
                let server = MBSMServer::new(ch.0, space_map.get(ch.0).unwrap());
                server_callback(&server);
                let receiver = MBAsyncReceiver::new(ch.1);
                async move {
                    loop {
                        let req = receiver.recv_req().await;
                        let mut resp = server.do_cmd(&req).await;
                        if let Some(r) = resp.take() {
                            receiver.send_resp(r).await;
                        }
                    }
                }
            })
            .collect::<Vec<_>>();
        join_all(futures)
    }
}
pub struct MBChannelShareMemBuilder<SM: MBShareMem> {
    docs: Vec<Yaml>,
    sys: MBChannelShareMemSys<SM>,
}

impl<SM: MBShareMem> MBChannelShareMemBuilder<SM> {
    pub fn new(file: &str) -> Result<MBChannelShareMemBuilder<SM>, String> {
        let file_expand = shellexpand::full(file)
            .map_err(|e| e.to_string())?
            .to_string();
        let s = fs::read_to_string(file_expand).map_err(|e| e.to_string())?;
        Self::from_str(&s)
    }
    pub fn from_str(s: &str) -> Result<MBChannelShareMemBuilder<SM>, String> {
        Ok(MBChannelShareMemBuilder {
            docs: YamlLoader::load_from_str(s).map_err(|e| e.to_string())?,
            sys: MBChannelShareMemSys::<SM>::new(),
        })
    }

    pub fn build(
        mut self,
        space_map: &HashMap<String, Arc<Mutex<SM>>>,
    ) -> Result<MBChannelShareMemSys<SM>, String> {
        if let Yaml::Hash(ref chs) = self.docs[0] {
            for (key, ch) in chs.iter() {
                let k = key.as_str().unwrap();
                let elf = ch["elf"].as_str();
                let base = ch["base"].as_i64();
                let space_k = ch["space"]
                    .as_str()
                    .ok_or(format!("{:?}: No space found!", k))?;
                let space = space_map.get(space_k).ok_or(format!(
                    "{:?}: space {:?} not found in current map!",
                    k, space_k
                ))?;
                let ch = if let Some(e) = elf {
                    MBChannelShareMem::with_elf(e, space)
                } else if let Some(b) = base {
                    MBChannelShareMem::new(b as MBPtrT, space)
                } else {
                    return Err(format!("{:?}: Neither found elf nor base!", k));
                };
                self.sys
                    .chs
                    .insert(k.to_string(), Arc::new(Mutex::new(MBAsyncChannel::new(ch))));
            }
            Ok(self.sys)
        } else {
            Err("No channels found in mailbox cfg file!".to_string())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[derive(Debug)]
    struct MyShareMem {
        name: String,
        base: MBPtrT,
        size: MBPtrT,
    }
    impl MBShareMemBlock for MyShareMem {
        fn base(&self) -> MBPtrT {
            self.base
        }
        fn size(&self) -> MBPtrT {
            self.size
        }
    }
    impl MBShareMem for MyShareMem {
        fn write(&mut self, _addr: MBPtrT, _data: &[u8]) -> usize {
            0
        }
        fn read(&self, _addr: MBPtrT, _data: &mut [u8]) -> usize {
            0
        }
    }

    #[derive(Default)]
    struct MyParser;
    impl MBShareMemParser for MyParser {
        type MemType = MyShareMem;
        fn parse(&self, key: &str, doc: &Yaml) -> Result<Self::MemType, String> {
            Ok(MyShareMem {
                name: key.to_string(),
                base: doc["base"]
                    .as_i64()
                    .ok_or("base should be integer!".to_string())? as MBPtrT,
                size: doc["size"]
                    .as_i64()
                    .ok_or("size should be integer!".to_string())? as MBPtrT,
            })
        }
    }
    const SM_YAML: &'static str = "
    shared:
        global:
            base: 0x80000000
            size: 0x10000000
        global2:
            base: 0x90000000
            size: 0x10000000
    space:
        core0:
            - ilm:
                base: 0
                size: 4096
            - dlm:
                base: 4096
                size: 16384
            - global
            - global2
        core1:
            - dlm:
                base: 4096
                size: 16384
            - global
        core2:
            - dlm:
                base: 4096
                size: 16384
            - global
    ";
    #[test]
    fn sm_yaml_test() {
        let spaces = MBShareMemSpaceBuilder::<MyShareMem, MyParser>::from_str(SM_YAML)
            .unwrap()
            .build_shared()
            .unwrap()
            .build_spaces()
            .unwrap();
        println!("space:{:?}", spaces.keys());
    }

    #[test]
    fn ch_yaml_test() {
        let s = "
            core0:
                space: core0
                base: 0x1000
            core1:
                space: core1
                base: 0x1000
            core2:
                space: core2
                base: 0x1000
        ";
        let spaces = MBShareMemSpaceBuilder::<MyShareMem, MyParser>::from_str(SM_YAML)
            .unwrap()
            .build_shared()
            .unwrap()
            .build_spaces()
            .unwrap();
        MBChannelShareMemBuilder::<MBShareMemSpace<MyShareMem>>::from_str(s)
            .unwrap()
            .build(&spaces)
            .unwrap();
    }
}
