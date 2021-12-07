extern crate yaml_rust;
use super::mb_fs::*;
use crate::mb_rpcs::*;
use crate::mb_std::*;
use async_std::future::Future;
use futures::future::join_all;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
pub use yaml_rust::{Yaml, YamlLoader};
pub fn get_yaml_with_ref<'a>(doc: &'a Yaml, key: &str) -> &'a Yaml {
    let result = &doc[key];
    match result {
        Yaml::BadValue => {
            let refer = &doc["<<"];
            match refer {
                Yaml::BadValue => result,
                _ => get_yaml_with_ref(refer, key),
            }
        }
        _ => result,
    }
}

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

    fn add_space(&self, mem_space: &mut MBShareMemSpace<M>, doc:&Vec<Yaml>) -> Result<(), String> {
        for y in doc.iter() {
            match y {
                Yaml::Hash(m) => {
                    let (name, v) = m.front().unwrap();
                    let n = name.as_str().unwrap();
                    mem_space
                        .add_mem(&Arc::new(Mutex::new(self.parser.parse(n, v)?)))
                        .map_err(|_| {
                            format!("{:?} is overlapped with other memory!", n)
                        })?;
                }
                Yaml::String(m) => mem_space
                    .add_mem(
                        self.shared
                            .get(m)
                            .ok_or(format!("Can't get shared mem {:?}!", m))?,
                    )
                    .map_err(|_| {
                        format!("{:?} is overlapped with other memory!", m)
                    })?,
                Yaml::Array(a) => self.add_space(mem_space, &a.to_vec())?,
                _ => return Err(format!("Invalid type {:?}!", y)),
            }
        }
        Ok(())
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
                self.add_space(&mut mem_space, &s).map_err(|e|{format!("{:?}: {:?}", k, e)})?;
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
    space_map: HashMap<String, Arc<Mutex<SM>>>,
    ch_space_map: HashMap<String, String>,
    fs: Arc<Option<MBFs>>,
}
impl<SM: MBShareMem> MBChannelShareMemSys<SM> {
    fn new(space_map: HashMap<String, Arc<Mutex<SM>>>) -> MBChannelShareMemSys<SM> {
        MBChannelShareMemSys {
            chs: HashMap::new(),
            space_map,
            ch_space_map: HashMap::new(),
            fs: Arc::new(None),
        }
    }
    pub fn get_space(&self, name: &str) -> Option<&Arc<Mutex<SM>>> {
        self.space_map.get(name)
    }
    pub fn get_ch_space_name(&self, ch_name: &str) -> Option<&str> {
        self.ch_space_map.get(ch_name).map(|s| s.as_str())
    }
    pub fn mailboxes(&self) -> &HashMap<String, Arc<Mutex<MBAsyncChannel<MBChannelShareMem<SM>>>>> {
        &self.chs
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
        server_callback: F,
    ) -> impl Future<Output = Vec<()>> + '_ {
        let futures = self
            .chs
            .iter()
            .map(|ch| {
                let server = MBSMServer::new(ch.0, &self.fs, self.space_map.get(ch.0).unwrap());
                server_callback(&server);
                let receiver = MBAsyncReceiver::new(ch.1);
                async move {
                    loop {
                        let req = receiver.recv_req().await;
                        match server.do_cmd(&req).await {
                            Ok(r) => receiver.send_resp(r).await,
                            Err(MBAsyncRPCError::Stop) => break,
                            Err(MBAsyncRPCError::Illegal(action)) => panic!("Illegal cmd {:?}", action),
                            _ => {} 
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
    pub fn new(
        file: &str,
        space_map: HashMap<String, Arc<Mutex<SM>>>,
    ) -> Result<MBChannelShareMemBuilder<SM>, String> {
        let file_expand = shellexpand::full(file)
            .map_err(|e| e.to_string())?
            .to_string();
        let s = fs::read_to_string(file_expand).map_err(|e| e.to_string())?;
        Self::from_str(&s, space_map)
    }
    pub fn from_str(
        s: &str,
        space_map: HashMap<String, Arc<Mutex<SM>>>,
    ) -> Result<MBChannelShareMemBuilder<SM>, String> {
        Ok(MBChannelShareMemBuilder {
            docs: YamlLoader::load_from_str(s).map_err(|e| e.to_string())?,
            sys: MBChannelShareMemSys::<SM>::new(space_map),
        })
    }

    pub fn cfg_channels(mut self) -> Result<MBChannelShareMemBuilder<SM>, String> {
        if let Yaml::Hash(ref chs) = self.docs[0] {
            for (key, ch) in chs.iter() {
                let k = key.as_str().unwrap();
                let elf = ch["elf"].as_str();
                let load = ch["load"].as_bool();
                let base = ch["base"].as_i64();
                let space_k = ch["space"]
                    .as_str()
                    .ok_or(format!("{:?}: No space found!", k))?;
                let space = self.sys.get_space(space_k).ok_or(format!(
                    "{:?}: space {:?} not found in current map!",
                    k, space_k
                ))?;
                let ch = if let Some(e) = elf {
                    if let Some(l) = load {
                        MBChannelShareMem::with_elf(e, space, l)
                    } else {
                        MBChannelShareMem::with_elf(e, space, true)
                    }
                } else if let Some(b) = base {
                    MBChannelShareMem::new(b as MBPtrT, space)
                } else {
                    return Err(format!("{:?}: Neither found elf nor base!", k));
                };
                self.sys
                    .chs
                    .insert(k.to_string(), Arc::new(Mutex::new(MBAsyncChannel::new(ch))));
                self.sys
                    .ch_space_map
                    .insert(k.to_string(), space_k.to_string());
            }
            Ok(self)
        } else {
            Err("No channels found in mailbox cfg file!".to_string())
        }
    }
    pub fn fs(mut self, root: &str) -> Result<MBChannelShareMemBuilder<SM>, String> {
        self.sys.fs = Arc::new(Some(MBFs::new(root).map_err(|e| e.to_string())?));
        Ok(self)
    }

    pub fn fs_with_special_and_virtual<
        F1: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
        F2: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
    >(
        mut self,
        root: &str,
        special_f: F1,
        virtual_f: F2,
    ) -> Result<MBChannelShareMemBuilder<SM>, String> {
        self.sys.fs = Arc::new(Some(
            MBFs::with_special_and_virtual(root, special_f, virtual_f)
                .map_err(|e| e.to_string())?,
        ));
        Ok(self)
    }

    pub fn fs_with_special<
        F: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
    >(
        mut self,
        root: &str,
        f: F,
    ) -> Result<MBChannelShareMemBuilder<SM>, String> {
        self.sys.fs = Arc::new(Some(
            MBFs::with_special(root, f).map_err(|e| e.to_string())?,
        ));
        Ok(self)
    }
    pub fn fs_with_virtual<
        F: FnMut(&mut HashMap<String, Box<dyn MBFileOpener>>) -> Result<(), String>,
    >(
        mut self,
        root: &str,
        f: F,
    ) -> Result<MBChannelShareMemBuilder<SM>, String> {
        self.sys.fs = Arc::new(Some(
            MBFs::with_virtual(root, f).map_err(|e| e.to_string())?,
        ));
        Ok(self)
    }

    pub fn build(self) -> MBChannelShareMemSys<SM> {
        self.sys
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
        fn write(&mut self, _addr: MBPtrT, data: &[u8]) -> usize {
            data.len()
        }
        fn read(&self, _addr: MBPtrT, data: &mut [u8]) -> usize {
            data.len()
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

    ref: &ref
        - global
        - global2
    space:
        core0:
            - ilm:
                base: 0
                size: 4096
            - dlm:
                base: 4096
                size: 16384
            - *ref
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
        MBChannelShareMemBuilder::<MBShareMemSpace<MyShareMem>>::from_str(s, spaces)
            .unwrap()
            .cfg_channels()
            .unwrap()
            .build();
    }
}
