use crate::mb_channel::*;
use crate::mb_rpcs::*;
extern crate nom;
use super::{MBAsyncRPC, MBAsyncRPCError, MBAsyncRPCResult};
use crate::mb_std::mb_async_channel::*;
use crate::mb_std::mb_ptr_resolver::*;
use async_std::prelude::*;
use async_std::task::Context;
use async_std::task::Poll;
use nom::{branch::alt, bytes::complete::*, character::complete::*, IResult};
use std::fmt::{self, Debug, Display, Formatter};
use std::slice::Iter;

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
    FmtError(String),
}
impl Display for MBCParseError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            MBCParseError::IOError(e) => write!(f, "MBCParseError::IOError{}", e),
            MBCParseError::ParseError(e) => write!(f, "MBCParseError::ParseError{}", e),
            MBCParseError::FmtError(e) => write!(f, "MBCParseError::FmtError{}", e),
        }
    }
}

impl<E: Debug> From<nom::Err<E>> for MBCParseError {
    fn from(e: nom::Err<E>) -> MBCParseError {
        MBCParseError::ParseError(e.to_string())
    }
}

#[derive(Debug)]
enum MBCStringToken {
    FMT(char),
    ORIGIN(String),
}
impl MBCStringToken {
    fn get_string<RA: MBPtrReader, WA: MBPtrWriter, R: MBPtrResolver<READER = RA, WRITER = WA>>(
        &self,
        arg: &mut Iter<MBPtrT>,
        r: &R,
    ) -> Result<String, MBCParseError> {
        match self {
            MBCStringToken::FMT(c) => {
                if let Some(data) = arg.next() {
                    match c {
                        'd' => Ok(format!("{}", *data)),
                        'x' => Ok(format!("{:#x}", *data)),
                        's' => {
                            let c_str = r
                                .read_c_str(*data as *const u8)
                                .map_err(|e| MBCParseError::IOError(e))?;
                            Ok(c_str.to_string())
                        }
                        'f' => Ok(format!(
                            "{:.10}",
                            f32::from_bits(*data as u32)
                        )),
                        _ => Err(MBCParseError::FmtError(format!(
                            "Format \"%{}\" is not support! Only support \"%d\", \"%x\", \"%s\", \"%f\"!",
                            c
                        ))),
                    }
                } else {
                    Err(MBCParseError::FmtError(
                        "number of args does not match format!".to_string(),
                    ))
                }
            }
            MBCStringToken::ORIGIN(s) => Ok(format!("{}", s)),
        }
    }
}

impl From<char> for MBCStringToken {
    fn from(value: char) -> Self {
        MBCStringToken::FMT(value)
    }
}

fn parse_str<'a>(s: &'a str) -> IResult<&'a str, MBCStringToken> {
    let (s, origin) = take_till(|c| c == '\\' || c == '%')(s)?;
    Ok((s, MBCStringToken::ORIGIN(origin.to_string())))
}
fn parse_escaped<'a>(s: &'a str) -> IResult<&'a str, MBCStringToken> {
    let (s, _) = tag("\\")(s)?;
    let (s, token) = anychar(s)?;
    Ok((s, MBCStringToken::ORIGIN(token.to_string())))
}
fn parse_fmt<'a>(s: &'a str) -> IResult<&'a str, MBCStringToken> {
    let (s, _) = tag("%")(s)?;
    let (s, t) = anychar(s)?;
    Ok((s, MBCStringToken::from(t)))
}

fn parse_symbol<'a>(s: &'a str) -> IResult<&'a str, MBCStringToken> {
    alt((parse_escaped, parse_fmt))(s)
}

struct MBCStringFmtParser<
    'a,
    RA: MBPtrReader,
    WA: MBPtrWriter,
    R: MBPtrResolver<READER = RA, WRITER = WA>,
> {
    buffer: String,
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
            buffer: String::new(),
            fmt_str,
            file,
            pos,
            args: &args.args[..args.args_len()],
            r,
        })
    }
    fn parse_next<'b, F: FnMut(&str) -> IResult<&str, MBCStringToken>>(
        &mut self,
        s: &'b str,
        args: &mut Iter<MBPtrT>,
        mut parser: F,
    ) -> Result<&'b str, MBCFmtError> {
        let (s, t) = parser(s).map_err(|e| MBCFmtError {
            e: MBCParseError::from(e),
            file: self.file.to_string(),
            pos: self.pos,
        })?;
        let result = t.get_string(args, self.r).map_err(|e| MBCFmtError {
            e,
            file: self.file.to_string(),
            pos: self.pos,
        })?;
        self.buffer += &result;
        Ok(s)
    }
    fn parser_inner<'b>(
        &mut self,
        s: &'b str,
        args: &mut Iter<MBPtrT>,
    ) -> Result<&'b str, MBCFmtError> {
        let s = self.parse_next(s, args, parse_str)?;
        if s.len() == 0 {
            return Ok(s);
        }
        let s = self.parse_next(s, args, parse_symbol)?;
        if s.len() == 0 {
            return Ok(s);
        }
        self.parser_inner(s, args)
    }
    fn parse(&mut self) -> Result<String, MBCFmtError> {
        let mut arg_iter = self.args.iter();
        let fmt_str = self.fmt_str.to_owned();
        self.parser_inner(&fmt_str, &mut arg_iter)?;
        Ok(self.buffer.to_owned())
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
        let mut parser = MBCStringFmtParser::new(&c_str_args, r).unwrap();
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
