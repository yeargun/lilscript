//! Shared short identifier encoding; lexical/property users own their reservations.
pub(crate) const ALPHABET: &[u8; 54] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ$_";

pub(crate) fn encode(
    mut index: usize,
    alphabet: &[u8; 54],
    compact: bool,
    bytes: &mut [u8],
) -> usize {
    let mut length = 0;
    loop {
        let radix = if compact && length != 0 { 64 } else { 54 };
        let digit = index % radix;
        bytes[length] = if digit < 54 {
            alphabet[digit]
        } else {
            b'0' + (digit - 54) as u8
        };
        length += 1;
        index /= radix;
        if index == 0 {
            return length;
        }
        index -= 1;
    }
}
