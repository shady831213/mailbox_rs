// Copyright (c) 2021 Thomas Jollans

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is furnished
// to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS
// OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
// WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF
// OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

// Modified https://github.com/tjol/sprintf-rs
//! Libc s(n)printf clone written in Rust, so you can use printf-style
//! formatting without a libc (e.g. in WebAssembly).
//!
//! **Note:** *You're probably better off using standard Rust string formatting
//! instead of thie crate unless you specificaly need printf compatibility.*
//!
//! It follows the standard C semantics, except:
//!
//!  * Locale-aware UNIX extensions (`'` and GNU’s `I`) are not supported.
//!  * `%a`/`%A` (hexadecimal floating point) are currently not implemented.
//!  * Length modifiers (`h`, `l`, etc.) are checked, but ignored. The passed
//!    type is used instead.
//!
//! The types of the arguments are checked at runtime.
//!

mod format;
mod parser;

pub use format::Printf;
use parser::{parse_format_string, FormatElement};
pub use parser::{ConversionSpecifier, ConversionType, NumericParam};

/// Error type
#[derive(Debug, Clone)]
pub enum PrintfError {
    /// Error parsing the format string
    ParseError,
    /// Incorrect type passed as an argument
    WrongType,
    /// Too many arguments passed
    TooManyArgs,
    /// Too few arguments passed
    NotEnoughArgs,
    /// other with msg
    Other(String),
    /// Other error (should never happen)
    Unknown,
}

pub type Result<T> = std::result::Result<T, PrintfError>;

/// Format a string. (Roughly equivalent to `vsnprintf` or `vasprintf` in C)
///
/// Takes a printf-style format string `format` and a slice of dynamically
/// typed arguments, `args`.
///
/// See also: [sprintf]
pub fn vsprintf<A: Printf>(format: &str, args: &[A]) -> Result<String> {
    vsprintfp(&parse_format_string(format)?, args)
}

fn vsprintfp<A: Printf>(format: &[FormatElement], args: &[A]) -> Result<String> {
    let mut res = String::new();

    let mut args = args;
    let mut pop_arg = || {
        if args.is_empty() {
            Err(PrintfError::NotEnoughArgs)
        } else {
            let a = &args[0];
            args = &args[1..];
            Ok(a)
        }
    };

    for elem in format {
        match elem {
            FormatElement::Verbatim(s) => {
                res.push_str(s);
            }
            FormatElement::Format(spec) => {
                if spec.conversion_type == ConversionType::PercentSign {
                    res.push('%');
                } else {
                    let mut completed_spec = *spec;
                    if spec.width == NumericParam::FromArgument {
                        completed_spec.width = NumericParam::Literal(
                            pop_arg()?.as_int(spec).ok_or(PrintfError::WrongType)?,
                        )
                    }
                    if spec.precision == NumericParam::FromArgument {
                        completed_spec.precision = NumericParam::Literal(
                            pop_arg()?.as_int(spec).ok_or(PrintfError::WrongType)?,
                        )
                    }
                    res.push_str(&pop_arg()?.format(&completed_spec)?);
                }
            }
        }
    }

    if args.is_empty() {
        Ok(res)
    } else {
        Err(PrintfError::TooManyArgs)
    }
}
