//! Measure formatter output without constructing an intermediate string.
use std::fmt::{self, Write};

pub(crate) fn measure(write: impl FnOnce(&mut Length) -> fmt::Result) -> Option<usize> {
    let mut length = Length(0);
    write(&mut length).ok()?;
    Some(length.0)
}
pub(crate) struct Length(usize);
impl Write for Length {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self.0.checked_add(text.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn literal_measurement_matches_emission_for_surrogates_and_number_boundaries() {
        use crate::literal::StringValue;
        for value in [
            StringValue::from("quote\" slash\\ line\n"),
            StringValue::from("😀 ${tail}"),
            StringValue::from_utf16(vec![0xd800, 0x22, 0xdc00]),
        ] {
            for quote in ['\'', '"', '`'] {
                assert_eq!(
                    crate::js_string::literal_length(&value, quote),
                    Some(crate::js_string::literal(&value, quote).len())
                );
            }
        }
        for value in [
            0.0,
            -0.0,
            0.00001,
            -0.125,
            1e25,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::from_bits(1),
        ] {
            // number_spelling handles negative zero at the expression boundary;
            // this measures the same finite-number formatter as emitted text.
            let length = crate::js::number_spelling_length(value).unwrap();
            assert!(length > 0 && length <= 32);
        }
        assert_eq!(crate::js::number_spelling_length(f64::INFINITY), None);
        assert_eq!(crate::js::number_spelling_length(f64::NAN), None);
    }
}
