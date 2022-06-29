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
            ConversionType::Char => self.arg.format(spec),
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
        let rest_args_pos = c_str_args.dir_args_len();
        for (i, d) in c_str_args.args[..rest_args_pos].iter_mut().enumerate() {
            *d = req.args[3 + i];
        }
        if c_str_args.rest_args_len() > 0 {
            r.read_slice(
                req.args[MB_MAX_ARGS - 1] as *const MBPtrT,
                &mut c_str_args.args[rest_args_pos..],
            );
        }
        let parser = MBCStringFmtParser::new(&c_str_args, r).unwrap();
        let s = parser.parse().unwrap();
        print!("[{}] {}", server_name, s);
        Poll::Ready(if c_str_args.rest_args_len() > 0 {
            Ok(MBRespEntry::default())
        } else {
            Err(MBAsyncRPCError::NoResp)
        })
    }
}

impl<'a> MBCPrint<'a> {
    pub fn to_cstr_args(
        &self,
        file: MBPtrT,
        pos: u32,
        fmt_str: MBPtrT,
        args: &'a [MBPtrT],
    ) -> MBCStringArgs {
        let mut c_str_args = MBCStringArgs::default();
        c_str_args.len = args.len() as u32 + 3;
        c_str_args.file = file;
        c_str_args.pos = pos as MBPtrT;
        c_str_args.fmt_str = fmt_str;
        for (i, d) in args.iter().enumerate() {
            c_str_args.args[i] = *d
        }
        c_str_args
    }
}
pub fn mb_cprint<'a, CH: MBChannelIf>(
    sender: &'a MBAsyncSender<CH>,
    fmt_str: &'a str,
    file: &'a str,
    pos: u32,
    args: &'a [MBPtrT],
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
        if c_str_args.rest_args_len() > 0 {
            sender.recv_resp(&cprint_rpc).await;
        }
    }
}
