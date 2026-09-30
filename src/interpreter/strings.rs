//! Independent UTF-16 string semantics. Rust's UTF-8 String cannot represent a
//! lone surrogate; retain those units instead of replacing or dropping them.
use super::*;

pub(super) fn string_units(value: &Value) -> Option<Vec<u16>> {
    match value {
        Value::String(value) => Some(value.encode_utf16().collect()),
        Value::StringUnits(value) => Some(value.as_ref().clone()),
        _ => None,
    }
}

pub(super) fn string_value(units: Vec<u16>) -> Value {
    match String::from_utf16(&units) {
        Ok(value) => Value::String(value),
        Err(_) => Value::StringUnits(Rc::new(units)),
    }
}

pub(super) fn decode_source_units(source: &str, span: Span) -> Result<Vec<u16>, InterpretError> {
    let mut result = Vec::new();
    let mut chars = source.chars().peekable();
    let error = || InterpretError::new(span, "invalid source string escape");
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.extend(ch.encode_utf16(&mut [0; 2]).iter().copied());
            continue;
        }
        let escaped = chars.next().ok_or_else(error)?;
        match escaped {
            'n' => result.push(10),
            'r' => result.push(13),
            't' => result.push(9),
            'b' => result.push(8),
            'f' => result.push(12),
            'v' => result.push(11),
            '0' => result.push(0),
            '\n' | '\u{2028}' | '\u{2029}' => {}
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            'x' | 'u' => {
                let braced = escaped == 'u' && chars.peek() == Some(&'{');
                let count = if escaped == 'x' { 2 } else { 4 };
                let mut value = 0u32;
                if braced {
                    chars.next();
                    let mut digits = 0;
                    loop {
                        let digit = chars.next().ok_or_else(error)?;
                        if digit == '}' {
                            if digits == 0 {
                                return Err(error());
                            }
                            break;
                        }
                        value = value
                            .checked_mul(16)
                            .and_then(|value| {
                                digit
                                    .to_digit(16)
                                    .and_then(|digit| value.checked_add(digit))
                            })
                            .ok_or_else(error)?;
                        digits += 1;
                    }
                } else {
                    for _ in 0..count {
                        value = value * 16
                            + chars
                                .next()
                                .and_then(|ch| ch.to_digit(16))
                                .ok_or_else(error)?;
                    }
                }
                if value <= 0xffff {
                    result.push(value as u16);
                } else if value <= 0x10ffff {
                    let value = value - 0x10000;
                    result.extend([
                        0xd800 | (value >> 10) as u16,
                        0xdc00 | (value & 1023) as u16,
                    ]);
                } else {
                    return Err(error());
                }
            }
            // ECMAScript identity escapes, including escaped quotes/backslash.
            ch => result.extend(ch.encode_utf16(&mut [0; 2]).iter().copied()),
        }
    }
    Ok(result)
}

fn trim_unit(unit: u16) -> bool {
    matches!(unit, 0x0009..=0x000d | 0x0020 | 0x00a0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}

impl<'program, 'ast, 'src> ReferenceInterpreter<'program, 'ast, 'src> {
    pub(super) fn evaluate_string_method(
        &self,
        receiver: &str,
        method: &str,
        args: &[Value],
        span: Span,
    ) -> Result<Value, InterpretError> {
        self.evaluate_string_units(
            &receiver.encode_utf16().collect::<Vec<_>>(),
            method,
            args,
            span,
        )
    }

    pub(super) fn evaluate_string_units(
        &self,
        units: &[u16],
        method: &str,
        args: &[Value],
        span: Span,
    ) -> Result<Value, InterpretError> {
        let integer = |index: usize, default: i32| match args.get(index) {
            None => Ok(default),
            Some(Value::Int(value)) => Ok(*value),
            _ => Err(InterpretError::new(
                span,
                "string method expected integer argument",
            )),
        };
        let text = |index: usize| {
            args.get(index)
                .and_then(string_units)
                .ok_or_else(|| InterpretError::new(span, "string method expected string argument"))
        };
        Ok(match method {
            "length" => Value::Int(units.len() as i32),
            "codePointLength" => {
                Value::Int(char::decode_utf16(units.iter().copied()).count() as i32)
            }
            "charAt" | "charCodeAt" | "codeUnitAt" => {
                let unit = usize::try_from(integer(0, 0)?)
                    .ok()
                    .and_then(|index| units.get(index))
                    .copied();
                match method {
                    "charAt" => string_value(unit.into_iter().collect()),
                    "charCodeAt" => Value::Int(i32::from(unit.unwrap_or(0))),
                    _ => Value::Int(i32::from(unit.ok_or_else(|| {
                        InterpretError::new(span, "string index is out of bounds")
                    })?)),
                }
            }
            "slice" | "substring" => {
                let (mut start, mut end) = (integer(0, 0)?, integer(1, units.len() as i32)?);
                let range = if method == "substring" {
                    start = start.max(0).min(units.len() as i32);
                    end = end.max(0).min(units.len() as i32);
                    start.min(end) as usize..start.max(end) as usize
                } else {
                    let start = normalize_slice_index(start, units.len());
                    let end = normalize_slice_index(end, units.len());
                    start..end.max(start)
                };
                string_value(units[range].to_vec())
            }
            "includes" | "startsWith" | "endsWith" | "indexOf" | "lastIndexOf" => {
                let needle = text(0)?;
                let default = if matches!(method, "lastIndexOf" | "endsWith") {
                    units.len() as i32
                } else {
                    0
                };
                let position = integer(1, default)?.max(0) as usize;
                let position = position.min(units.len());
                let equal = |index: usize| {
                    index
                        .checked_add(needle.len())
                        .and_then(|end| units.get(index..end))
                        == Some(needle.as_slice())
                };
                let index = if method == "lastIndexOf" {
                    (0..=position).rev().find(|&index| equal(index))
                } else {
                    (position..=units.len()).find(|&index| equal(index))
                };
                match method {
                    "startsWith" => Value::Bool(equal(position)),
                    "endsWith" => {
                        Value::Bool(position.checked_sub(needle.len()).is_some_and(equal))
                    }
                    "includes" => Value::Bool(index.is_some()),
                    _ => Value::Int(index.map_or(-1, |index| index as i32)),
                }
            }
            "split" => {
                let separator = text(0)?;
                let mut values = Vec::new();
                if separator.is_empty() {
                    values.extend(units.iter().map(|unit| string_value(vec![*unit])));
                } else {
                    let mut start = 0;
                    while let Some(index) = units[start..]
                        .windows(separator.len())
                        .position(|slice| slice == separator.as_slice())
                    {
                        let index = start + index;
                        values.push(string_value(units[start..index].to_vec()));
                        start = index + separator.len();
                    }
                    values.push(string_value(units[start..].to_vec()));
                }
                Value::Array(Rc::new(RefCell::new(values)))
            }
            "repeat" => {
                let count = usize::try_from(integer(0, 0)?)
                    .map_err(|_| InterpretError::new(span, "repeat count must be nonnegative"))?;
                let length = units
                    .len()
                    .checked_mul(count)
                    .ok_or_else(|| InterpretError::new(span, "repeated string is too large"))?;
                // The evaluator has a finite memory model as well as a step
                // bound. Refuse a huge oracle rather than exhaust the host.
                if length > 16 * 1024 * 1024 {
                    return Err(InterpretError::new(
                        span,
                        "reference string allocation limit exceeded",
                    ));
                }
                string_value(units.repeat(count))
            }
            "trim" | "trimStart" | "trimEnd" => {
                let start = if method == "trimEnd" {
                    0
                } else {
                    units
                        .iter()
                        .position(|unit| !trim_unit(*unit))
                        .unwrap_or(units.len())
                };
                let end = if method == "trimStart" {
                    units.len()
                } else {
                    units
                        .iter()
                        .rposition(|unit| !trim_unit(*unit))
                        .map_or(start, |index| index + 1)
                };
                string_value(units[start..end.max(start)].to_vec())
            }
            "toUpperCase" | "toLowerCase" => {
                // Case conversion needs whole scalar runs (contextual sigma),
                // while lone surrogates are preserved as code units.
                let mut output = Vec::new();
                let mut run = String::new();
                let flush = |run: &mut String, output: &mut Vec<u16>| {
                    output.extend(
                        if method == "toUpperCase" {
                            run.to_uppercase()
                        } else {
                            run.to_lowercase()
                        }
                        .encode_utf16(),
                    );
                    run.clear();
                };
                for decoded in char::decode_utf16(units.iter().copied()) {
                    match decoded {
                        Ok(ch) => run.push(ch),
                        Err(error) => {
                            flush(&mut run, &mut output);
                            output.push(error.unpaired_surrogate());
                        }
                    }
                }
                flush(&mut run, &mut output);
                string_value(output)
            }
            _ => {
                return Err(InterpretError::new(
                    span,
                    format!("unsupported interpreted string method `{method}`"),
                ))
            }
        })
    }
}
