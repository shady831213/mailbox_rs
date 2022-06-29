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

use super::{PrintfError, Result};

#[derive(Debug, Clone)]
pub(super) enum FormatElement {
    Verbatim(String),
    Format(ConversionSpecifier),
}

/// Parsed printf conversion specifier
#[derive(Debug, Clone, Copy)]
pub struct ConversionSpecifier {
    /// flag `l`: long
    pub long: bool,
    /// flag `ll`: long
    pub long_long: bool,
    /// flag `#`: use `0x`, etc?
    pub alt_form: bool,
    /// flag `0`: left-pad with zeros?
    pub zero_pad: bool,
    /// flag `-`: left-adjust (pad with spaces on the right)
    pub left_adj: bool,
    /// flag `' '` (space): indicate sign with a space?
    pub space_sign: bool,
    /// flag `+`: Always show sign? (for signed numbers)
    pub force_sign: bool,
    /// field width
    pub width: NumericParam,
    /// floating point field precision
    pub precision: NumericParam,
    /// data type
    pub conversion_type: ConversionType,
}

/// Width / precision parameter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericParam {
    /// The literal width
    Literal(i32),
    /// Get the width from the previous argument
    ///
    /// This should never be passed to [Printf::format()][crate::Printf::format()].
    FromArgument,
}

/// Printf data type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionType {
    /// `d`, `i`, or `u`
    DecInt,
    /// `o`
    OctInt,
    /// `x` or `p`
    HexIntLower,
    /// `X`
    HexIntUpper,
    /// `e`
    SciFloatLower,
    /// `E`
    SciFloatUpper,
    /// `f`
    DecFloatLower,
    /// `F`
    DecFloatUpper,
    /// `g`
    CompactFloatLower,
    /// `G`
    CompactFloatUpper,
    /// `c`
    Char,
    /// `s`
    String,
    /// `%`
    PercentSign,
}

pub(super) fn parse_format_string(fmt: &str) -> Result<Vec<FormatElement>> {
    // find the first %
    let mut res = Vec::new();
    let parts: Vec<&str> = fmt.splitn(2, '%').collect();
    if !parts[0].is_empty() {
        res.push(FormatElement::Verbatim(parts[0].to_owned()));
    }
    if parts.len() > 1 {
        let (spec, rest) = take_conversion_specifier(parts[1])?;
        res.push(FormatElement::Format(spec));
        res.append(&mut parse_format_string(rest)?);
    }

    Ok(res)
}

fn take_conversion_specifier(s: &str) -> Result<(ConversionSpecifier, &str)> {
    let mut spec = ConversionSpecifier {
        long: false,
        long_long: false,
        alt_form: false,
        zero_pad: false,
        left_adj: false,
        space_sign: false,
        force_sign: false,
        width: NumericParam::Literal(0),
        precision: NumericParam::Literal(6),
        // ignore length modifier
        conversion_type: ConversionType::DecInt,
    };

    let mut s = s;

    // parse flags
    loop {
        match s.chars().next() {
            Some('#') => {
                spec.alt_form = true;
            }
            Some('0') => {
                spec.zero_pad = true;
            }
            Some('-') => {
                spec.left_adj = true;
            }
            Some(' ') => {
                spec.space_sign = true;
            }
            Some('+') => {
                spec.force_sign = true;
            }
            _ => {
                break;
            }
        }
        s = &s[1..];
    }
    // parse width
    let (w, mut s) = take_numeric_param(s);
    spec.width = w;
    // parse precision
    if matches!(s.chars().next(), Some('.')) {
        s = &s[1..];
        let (p, s2) = take_numeric_param(s);
        spec.precision = p;
        s = s2;
    }
    // check length specifier
    if s.starts_with("ll") {
        s = s.strip_prefix("ll").ok_or(PrintfError::ParseError)?;
        spec.long_long = true
    } else if s.starts_with("L") {
        s = s.strip_prefix("L").ok_or(PrintfError::ParseError)?;
        spec.long_long = true
    } else if s.starts_with("l") {
        s = s.strip_prefix("l").ok_or(PrintfError::ParseError)?;
        spec.long = true
    } else {
        //ignored length
        for len_spec in ["hh", "h", "q", "L", "j", "z", "Z", "t"] {
            if s.starts_with(len_spec) {
                s = s.strip_prefix(len_spec).ok_or(PrintfError::ParseError)?;
                break; // only allow one length specifier
            }
        }
    }
    // parse conversion type
    spec.conversion_type = match s.chars().next() {
        Some('i') | Some('d') | Some('u') => ConversionType::DecInt,
        Some('o') => ConversionType::OctInt,
        Some('x') => ConversionType::HexIntLower,
        Some('X') => ConversionType::HexIntUpper,
        Some('e') => ConversionType::SciFloatLower,
        Some('E') => ConversionType::SciFloatUpper,
        Some('f') => ConversionType::DecFloatLower,
        Some('F') => ConversionType::DecFloatUpper,
        Some('g') => ConversionType::CompactFloatLower,
        Some('G') => ConversionType::CompactFloatUpper,
        Some('c') | Some('C') => ConversionType::Char,
        Some('s') | Some('S') => ConversionType::String,
        Some('p') => {
            spec.alt_form = true;
            ConversionType::HexIntLower
        }
        Some('%') => ConversionType::PercentSign,
        _ => {
            return Err(PrintfError::ParseError);
        }
    };

    Ok((spec, &s[1..]))
}

fn take_numeric_param(s: &str) -> (NumericParam, &str) {
    match s.chars().next() {
        Some('*') => (NumericParam::FromArgument, &s[1..]),
        Some(digit) if ('1'..='9').contains(&digit) => {
            let mut s = s;
            let mut w = 0;
            loop {
                match s.chars().next() {
                    Some(digit) if ('0'..='9').contains(&digit) => {
                        w = 10 * w + (digit as i32 - '0' as i32);
                    }
                    _ => {
                        break;
                    }
                }
                s = &s[1..];
            }
            (NumericParam::Literal(w), s)
        }
        _ => (NumericParam::Literal(0), s),
    }
}
