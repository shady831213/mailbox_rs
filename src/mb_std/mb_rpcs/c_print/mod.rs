use crate::mb_channel::*;
use crate::mb_rpcs::*;
mod sprintf;
use super::{MBAsyncRPC, MBAsyncRPCError, MBAsyncRPCResult};
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use sprintf::{vsprintf, ConversionSpecifier, ConversionType, Printf, PrintfError};
use std::fmt::{self, Debug, Display, Formatter};
use std::marker::PhantomData;
use std::sync::Mutex;

struct CPrintArg<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>> {
    arg: MBPtrT,
    r: &'a R,
}

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    CPrintArg<'a, RA, WA, R>
{
    fn new(arg: MBPtrT, r: &'a R) -> Self {
        CPrintArg { arg, r }
    }
}

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>> Printf
    for CPrintArg<'a, RA, WA, R>
{
    fn format(&self, spec: &ConversionSpecifier) -> sprintf::Result<String> {
        match spec.conversion_type {
            // integer format
            ConversionType::DecInt
            | ConversionType::OctInt
            | ConversionType::HexIntLower
            | ConversionType::HexIntUpper => {
                if spec.long_long && std::mem::size_of::<MBPtrT>() < std::mem::size_of::<u64>() {
                    Err(PrintfError::Other(
                        "long long int is not supported".to_string(),
                    ))
                } else {
                    self.arg.format(spec)
                }
            }
            // char format
            ConversionType::Char => (self.arg as u8 as char).format(spec),
            // string format
            ConversionType::String => {
                let c_str = self
                    .r
                    .read_c_str(self.arg as *const u8)
                    .map_err(|e| PrintfError::Other(e))?;
                c_str.format(spec)
            }
            // float format
            ConversionType::SciFloatLower
            | ConversionType::SciFloatUpper
            | ConversionType::DecFloatLower
            | ConversionType::DecFloatUpper
            | ConversionType::CompactFloatLower
            | ConversionType::CompactFloatUpper => {
                if spec.long_long {
                    Err(PrintfError::Other(
                        "long double is not supported".to_string(),
                    ))
                } else if spec.long {
                    if std::mem::size_of::<MBPtrT>() < std::mem::size_of::<f64>() {
                        Err(PrintfError::Other("double is not supported".to_string()))
                    } else {
                        f64::from_bits(self.arg as u64).format(spec)
                    }
                } else {
                    f32::from_bits(self.arg as u32).format(spec)
                }
            }
            _ => Err(PrintfError::WrongType),
        }
    }
    fn as_int(&self, spec: &ConversionSpecifier) -> Option<i32> {
        match spec.conversion_type {
            // integer format
            ConversionType::DecInt
            | ConversionType::OctInt
            | ConversionType::HexIntLower
            | ConversionType::HexIntUpper => self.arg.as_int(spec),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct MBCFmtError {
    e: MBCParseError,
    file: String,
    pos: u32,
}

impl Display for MBCFmtError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(
            f,
            "{}! in {}, line {}!",
            self.e.to_string(),
            self.file,
            self.pos
        )
    }
}

#[derive(Debug)]
enum MBCParseError {
    IOError(String),
    ParseError(String),
}
impl Display for MBCParseError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            MBCParseError::IOError(e) => write!(f, "MBCParseError::IOError{}", e),
            MBCParseError::ParseError(e) => write!(f, "MBCParseError::ParseError{}", e),
        }
    }
}

struct MBCStringFmtParser<
    'a,
    RA: MBPtrReader,
    WA: MBPtrWriter,
    R: MBPtrResolver<READER = RA, WRITER = WA>,
> {
    fmt_str: String,
    file: String,
    pos: u32,
    args: &'a [MBPtrT],
    r: &'a R,
}

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBCStringFmtParser<'a, RA, WA, R>
{
    fn new(
        args: &'a MBCStringArgs,
        r: &'a R,
    ) -> Result<MBCStringFmtParser<'a, RA, WA, R>, MBCFmtError> {
        let pos = args.pos as u32;
        let file = r.read_c_str(args.file as *const u8).map_err(|e| {
            let fmt_e = MBCParseError::IOError(e);
            MBCFmtError {
                e: fmt_e,
                file: "<unknown>".to_string(),
                pos,
            }
        })?;
        let fmt_str = r.read_c_str(args.fmt_str as *const u8).map_err(|e| {
            let fmt_e = MBCParseError::IOError(e);
            MBCFmtError {
                e: fmt_e,
                file: file.to_string(),
                pos,
            }
        })?;
        Ok(MBCStringFmtParser {
            fmt_str,
            file,
            pos,
            args: &args.args[..args.args_len()],
            r,
        })
    }
    fn parse(&self) -> Result<String, MBCFmtError> {
        let args: Vec<CPrintArg<_, _, _>> = self
            .args
            .iter()
            .map(|a| CPrintArg::new(*a, self.r))
            .collect();
        vsprintf(&self.fmt_str, &args).map_err(|e| MBCFmtError {
            e: MBCParseError::ParseError(format!("{:?}", e)),
            file: self.file.clone(),
            pos: self.pos,
        })
    }
}

pub struct MBCPrint<'a> {
    buf: Mutex<String>,
    _marker: PhantomData<&'a u8>,
}
impl<'a> MBCPrint<'a> {
    pub fn new() -> MBCPrint<'a> {
        MBCPrint {
            buf: Mutex::new(String::new()),
            _marker: PhantomData,
        }
    }
}

impl<'a> MBRpc for MBCPrint<'a> {
    type REQ = &'a MBCStringArgs;
    type RESP = ();
    fn put_req(&self, req: Self::REQ, entry: &mut MBReqEntry) {
        entry.set_words(req.len);
        entry.set_action(MBAction::CPRINT);
        entry.set_args(0, req.fmt_str);
        entry.set_args(1, req.file);
        entry.set_args(2, req.pos);
        for (i, d) in req.args[..req.args_len()].iter().enumerate() {
            entry.set_args(3 + i, *d);
        }
        // entry.action = MBAction::CPRINT;
        // entry.words = req.len;
        // entry.args[0] = req.fmt_str;
        // entry.args[1] = req.file;
        // entry.args[2] = req.pos;
        // for (i, d) in req.args[..req.arg_len()].iter().enumerate() {
        //     entry.args[3 + i] = *d
        // }
    }
    fn get_resp(&self, _: &MBRespEntry) -> Self::RESP {}
}

impl<'a, RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>
    MBAsyncRPC<RA, WA, R> for MBCPrint<'a>
{
    fn poll_cmd(
        &self,
        server_name: &str,
        r: &R,
        req: &MBReqEntry,
        _cx: &mut Context,
    ) -> Poll<MBAsyncRPCResult> {
        let mut c_str_args = MBCStringArgs::default();
        c_str_args.len = req.words;
        c_str_args.fmt_str = req.args[0];
        c_str_args.file = req.args[1];
        c_str_args.pos = req.args[2];
        let args_len = c_str_args.args_len();
        for (i, d) in c_str_args.args[..args_len].iter_mut().enumerate() {
            *d = req.args[3 + i];
        }
        let parser = MBCStringFmtParser::new(&c_str_args, r).unwrap();
        let s = parser.parse().unwrap();
        {
            let mut buf = self.buf.lock().unwrap();
            *buf += &s;
            if buf.ends_with('\n') {
                print!("[{}] {}", server_name, buf);
                buf.clear();
            }
        }
        Poll::Ready(Err(MBAsyncRPCError::NoResp))
    }
}

impl<'a> MBCPrint<'a> {
    pub fn to_cstr_args(
        &self,
        file: MBPtrT,
        pos: u32,
        fmt_str: MBPtrT,
        args: &'a [usize],
    ) -> MBCStringArgs {
        let mut c_str_args = MBCStringArgs::default();
        c_str_args.len = args.len() as u32 + 3;
        c_str_args.file = file;
        c_str_args.pos = pos as MBPtrT;
        c_str_args.fmt_str = fmt_str;
        for (i, d) in args.iter().enumerate() {
            c_str_args.args[i] = *d as MBPtrT
        }
        c_str_args
    }
}
pub fn mb_cprint<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    fmt_str: &'a str,
    file: &'a str,
    pos: u32,
    args: &'a [usize],
) -> impl Future<Output = ()> + 'a {
    let cprint_rpc = MBCPrint::new();
    async move {
        let c_str_args = cprint_rpc.to_cstr_args(
            file.as_ptr() as MBPtrT,
            pos,
            fmt_str.as_ptr() as MBPtrT,
            args,
        );
        sender.send_req(&cprint_rpc, &c_str_args).await;
    }
}
