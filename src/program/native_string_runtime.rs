//! Reference-counted immutable UTF-16 strings, owned slice views and exact
//! numeric/text conversion. Literal storage is static; dynamic strings use the
//! same allocator and owner protocol as other native values.
pub(super) const STRINGS: &str = concat!(include_str!("runtime/strings.c"), include_str!("runtime/string_builder.c"));
