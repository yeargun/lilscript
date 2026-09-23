//! Canonical complete-artifact size encoders. Inspection and compilation-owned
//! scoring share these paths; neither owns a cache or candidate search policy.
use crate::compilation_policy::WorkKind;
use crate::config::CompressionCostModel;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::alloc::{alloc, alloc_zeroed, dealloc, Layout};
use std::ffi::c_void;
use std::ptr;

pub const CANONICAL_ZLIB_PACKAGE_VERSION: &str = "1.1.24";
pub const CANONICAL_ZLIB_LIBRARY_VERSION: &str = "1.3.1";
pub const CANONICAL_BROTLI_PACKAGE_VERSION: &str = "1.1.0";
pub const CANONICAL_BROTLI_LIBRARY_VERSION: u32 = 0x0100_1000;
/// Part of backend provenance: admission failure is recoverable, not exit(1).
pub const CANONICAL_BROTLI_ALLOCATION_MODE: &str = "BROTLI_ENCODER_CLEANUP_ON_OOM";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodecError {
    Admission(AllocationError),
    Version(&'static str),
    Encoder(&'static str),
    Capacity,
}
impl From<AllocationError> for CodecError {
    fn from(error: AllocationError) -> Self {
        Self::Admission(error)
    }
}

/// Explicit inspection route. Raw needs no allocation or codec invocation.
pub fn measure(bytes: &[u8], model: CompressionCostModel) -> Result<usize, String> {
    match model {
        CompressionCostModel::Raw => Ok(bytes.len()),
        CompressionCostModel::Gzip => canonical_gzip_size(bytes),
        CompressionCostModel::Brotli => canonical_brotli_size(bytes),
    }
}

/// One artifact's delivered size under every canonical codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct JavaScriptTransferSizes {
    pub raw: usize,
    pub gzip9: usize,
    pub brotli11: usize,
}

/// Measure one artifact under every canonical codec, for inspection and
/// delivery manifests. Each call encodes; nothing is cached.
pub fn measure_javascript_transfer_sizes(bytes: &[u8]) -> Result<JavaScriptTransferSizes, String> {
    Ok(JavaScriptTransferSizes {
        raw: bytes.len(),
        gzip9: measure(bytes, CompressionCostModel::Gzip)?,
        brotli11: measure(bytes, CompressionCostModel::Brotli)?,
    })
}

fn measure_inspection(bytes: &[u8], model: CompressionCostModel) -> Result<usize, String> {
    measure_admitted(bytes, model, &mut AllocationBudget::new(None))
        .map_err(|error| format!("canonical candidate measurement failed: {error:?}"))
}

/// All encoder heap requests, including output scratch and allocator headers,
/// are admitted before allocation and released before return in the original
/// domain. Compressed output is counted, not retained. The raw artifact remains
/// owned/charged by the caller throughout this operation.
///
/// The frozen service tariff is one unit per input byte plus one per backend
/// call/allocation; zero-filled zlib state additionally charges its byte count.
/// These are deterministic logical units, not a claimed instruction/time bound.
/// The C backends cannot be interrupted in a CPU segment between callbacks;
/// callers enforce deadlines at service boundaries, not by assuming preemption.
pub(crate) fn measure_admitted(
    bytes: &[u8],
    model: CompressionCostModel,
    budget: &mut AllocationBudget<'_>,
) -> Result<usize, CodecError> {
    if model == CompressionCostModel::Raw {
        return Ok(bytes.len());
    }
    let _timing = match model {
        CompressionCostModel::Gzip => crate::timing::CANONICAL_GZIP.scope(0),
        CompressionCostModel::Brotli => crate::timing::CANONICAL_BROTLI.scope(0),
        CompressionCostModel::Raw => unreachable!(),
    };
    let mut phase = budget.scope();
    phase.work(
        WorkKind::Codec,
        u64::try_from(bytes.len()).map_err(|_| CodecError::Capacity)?,
    )?;
    let result = {
        let mut memory = CodecMemory {
            budget: &mut phase,
            error: None,
        };
        match model {
            CompressionCostModel::Raw => unreachable!(),
            CompressionCostModel::Gzip => gzip_size(bytes, &mut memory),
            CompressionCostModel::Brotli => brotli_size(bytes, &mut memory),
        }
    };
    // A final C CPU segment can cross the deadline without another allocation
    // callback. Its state/output guards have now freed all scratch. Preserve
    // original backend errors; successful results need this final admission,
    // which checks time without changing the deterministic work tariff.
    let size = result?;
    phase.work(WorkKind::Codec, 0)?;
    Ok(size)
}

pub fn canonical_zlib_version() -> Result<&'static str, String> {
    // SAFETY: process-lifetime NUL-terminated version supplied by linked zlib.
    unsafe { std::ffi::CStr::from_ptr(libz_sys::zlibVersion()) }
        .to_str()
        .map_err(|error| format!("zlib returned a non-UTF-8 version: {error}"))
}
pub fn canonical_brotli_version() -> u32 {
    // SAFETY: version query has no arguments or caller-owned memory.
    unsafe { compu_brotli_sys::BrotliEncoderVersion() }
}
pub(crate) fn canonical_gzip_size(bytes: &[u8]) -> Result<usize, String> {
    measure_inspection(bytes, CompressionCostModel::Gzip)
}
pub(crate) fn canonical_brotli_size(bytes: &[u8]) -> Result<usize, String> {
    measure_inspection(bytes, CompressionCostModel::Brotli)
}

// C requires alignment for its scalar/vector state members. The pinned zlib
// and Brotli allocation APIs require no over-aligned storage; a 16-byte header
// preserves suitable alignment and remembers each exact allocation Layout.
#[repr(C, align(16))]
struct AllocationHeader {
    bytes: usize,
}
const HEADER: usize = std::mem::size_of::<AllocationHeader>();
const ALIGN: usize = std::mem::align_of::<AllocationHeader>();
const OUTPUT_CHUNK: usize = 16 * 1024;

struct CodecMemory<'a, 'ledger> {
    budget: &'a mut AllocationBudget<'ledger>,
    error: Option<CodecError>,
}
impl CodecMemory<'_, '_> {
    fn fail(&mut self, error: CodecError) -> *mut c_void {
        if self.error.is_none() {
            self.error = Some(error);
        }
        ptr::null_mut()
    }
    fn call(&mut self) -> Result<(), CodecError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        self.budget.work(WorkKind::Codec, 1).map_err(Into::into)
    }
    fn allocate(&mut self, bytes: usize, zero: bool) -> *mut c_void {
        if self.error.is_some() {
            return ptr::null_mut();
        }
        let Some(total) = bytes.max(1).checked_add(HEADER) else {
            return self.fail(CodecError::Capacity);
        };
        let Ok(layout) = Layout::from_size_align(total, ALIGN) else {
            return self.fail(CodecError::Capacity);
        };
        let Ok(charge) = u64::try_from(total) else {
            return self.fail(CodecError::Capacity);
        };
        let work = if zero { charge } else { 0 };
        let Some(work) = work.checked_add(1) else {
            return self.fail(CodecError::Capacity);
        };
        if let Err(error) = self.budget.work(WorkKind::Codec, work) {
            return self.fail(error.into());
        }
        if let Err(error) = self.budget.retain(AllocationClass::Scratch, charge) {
            return self.fail(error.into());
        }
        // SAFETY: checked nonzero layout. The header is initialized before use;
        // the returned address preserves ALIGN and the requested payload size.
        let allocation = unsafe {
            if zero {
                alloc_zeroed(layout)
            } else {
                alloc(layout)
            }
        };
        if allocation.is_null() {
            let _ = self.budget.release(AllocationClass::Scratch, charge);
            return self.fail(CodecError::Admission(AllocationError::AllocationFailed));
        }
        unsafe {
            allocation
                .cast::<AllocationHeader>()
                .write(AllocationHeader { bytes: total });
            allocation.add(HEADER).cast()
        }
    }
    unsafe fn free(&mut self, address: *mut c_void) {
        if address.is_null() {
            return;
        }
        // SAFETY: both C backends return only live pointers from allocate. The
        // private header records the checked allocation layout, including itself.
        let base = unsafe { address.cast::<u8>().sub(HEADER) };
        let bytes = unsafe { (*base.cast::<AllocationHeader>()).bytes };
        let layout = unsafe { Layout::from_size_align_unchecked(bytes, ALIGN) };
        unsafe {
            dealloc(base, layout);
        }
        if let Err(error) = self.budget.release(AllocationClass::Scratch, bytes as u64) {
            self.fail(error.into());
        }
    }
    fn result(&self, fallback: &'static str) -> CodecError {
        self.error.unwrap_or(CodecError::Encoder(fallback))
    }
}
unsafe extern "C" fn zalloc(opaque: *mut c_void, items: u32, size: u32) -> *mut c_void {
    // SAFETY: opaque remains at its original stack address until deflateEnd.
    let memory = unsafe { &mut *opaque.cast::<CodecMemory<'_, '_>>() };
    match (items as usize).checked_mul(size as usize) {
        Some(bytes) => memory.allocate(bytes, true),
        None => memory.fail(CodecError::Capacity),
    }
}
unsafe extern "C" fn c_alloc(opaque: *mut c_void, size: usize) -> *mut c_void {
    // SAFETY: opaque remains live through BrotliEncoderDestroyInstance.
    unsafe { (&mut *opaque.cast::<CodecMemory<'_, '_>>()).allocate(size, false) }
}
unsafe extern "C" fn c_free(opaque: *mut c_void, address: *mut c_void) {
    // No panic/error unwinds across C; callback failures remain in memory.error.
    unsafe {
        (&mut *opaque.cast::<CodecMemory<'_, '_>>()).free(address);
    }
}
struct BufferAllocation<'a, 'ledger> {
    address: *mut u8,
    memory: *mut CodecMemory<'a, 'ledger>,
}
impl<'a, 'ledger> BufferAllocation<'a, 'ledger> {
    fn new(memory: &mut CodecMemory<'a, 'ledger>) -> Result<Self, CodecError> {
        let address = memory.allocate(OUTPUT_CHUNK, false).cast::<u8>();
        if address.is_null() {
            return Err(memory.result("output allocation failed"));
        }
        Ok(Self { address, memory })
    }
}
impl Drop for BufferAllocation<'_, '_> {
    fn drop(&mut self) {
        // SAFETY: lexical codec scope keeps memory alive until buffer drop.
        unsafe {
            (*self.memory).free(self.address.cast());
        }
    }
}
struct DeflateGuard(*mut libz_sys::z_stream);
impl Drop for DeflateGuard {
    fn drop(&mut self) {
        // SAFETY: initialized stream remains at this address; end frees state.
        unsafe {
            libz_sys::deflateEnd(self.0);
        }
    }
}
fn gzip_size(bytes: &[u8], memory: &mut CodecMemory<'_, '_>) -> Result<usize, CodecError> {
    // Compare bytes directly: no allocated diagnostics inside admitted scoring.
    let version = unsafe { std::ffi::CStr::from_ptr(libz_sys::zlibVersion()) };
    if version.to_bytes() != CANONICAL_ZLIB_LIBRARY_VERSION.as_bytes() {
        return Err(CodecError::Version("zlib1.3.1 required"));
    }
    let output = BufferAllocation::new(memory)?;
    let mut stream = libz_sys::z_stream {
        next_in: ptr::null_mut(),
        avail_in: 0,
        total_in: 0,
        next_out: ptr::null_mut(),
        avail_out: 0,
        total_out: 0,
        msg: ptr::null_mut(),
        state: ptr::null_mut(),
        zalloc,
        zfree: c_free,
        opaque: (memory as *mut CodecMemory<'_, '_>).cast(),
        data_type: 0,
        adler: 0,
        reserved: 0,
    };
    memory.call()?;
    // Same raw-deflate parameters as flate2 GzEncoder::best: level9,
    // window15, memLevel8, default strategy. zlib may retain &stream internally;
    // it is never moved between Init and End.
    let initialized = unsafe {
        libz_sys::deflateInit2_(
            &mut stream,
            9,
            libz_sys::Z_DEFLATED,
            -15,
            8,
            libz_sys::Z_DEFAULT_STRATEGY,
            libz_sys::zlibVersion(),
            std::mem::size_of::<libz_sys::z_stream>() as i32,
        )
    };
    if initialized != libz_sys::Z_OK {
        return Err(memory.result("deflate initialization failed"));
    }
    let _guard = DeflateGuard(&mut stream);
    let mut offset = 0usize;
    // Canonical flate2 header is10 bytes (mtime0/no extra/name/comment),
    // trailer8 bytes (CRC32/input length). Their values never affect size.
    let mut size = 18usize;
    loop {
        let input = (bytes.len() - offset).min(u32::MAX as usize);
        stream.next_in = unsafe { bytes.as_ptr().add(offset) }.cast_mut();
        stream.avail_in = input as u32;
        stream.next_out = output.address;
        stream.avail_out = OUTPUT_CHUNK as u32;
        let flush = if offset == bytes.len() {
            libz_sys::Z_FINISH
        } else {
            libz_sys::Z_NO_FLUSH
        };
        memory.call()?;
        let result = unsafe { libz_sys::deflate(&mut stream, flush) };
        if memory.error.is_some() {
            return Err(memory.result("deflate allocation failed"));
        }
        let consumed = input - stream.avail_in as usize;
        let produced = OUTPUT_CHUNK - stream.avail_out as usize;
        offset += consumed;
        size = size.checked_add(produced).ok_or(CodecError::Capacity)?;
        if result == libz_sys::Z_STREAM_END {
            return Ok(size);
        }
        if result != libz_sys::Z_OK
            || (consumed == 0 && produced == 0 && flush == libz_sys::Z_FINISH)
        {
            return Err(memory.result("deflate encoding failed"));
        }
    }
}
struct BrotliGuard(*mut compu_brotli_sys::BrotliEncoderState);
impl Drop for BrotliGuard {
    fn drop(&mut self) {
        unsafe {
            compu_brotli_sys::BrotliEncoderDestroyInstance(self.0);
        }
    }
}
fn brotli_size(bytes: &[u8], memory: &mut CodecMemory<'_, '_>) -> Result<usize, CodecError> {
    use compu_brotli_sys::*;
    if canonical_brotli_version() != CANONICAL_BROTLI_LIBRARY_VERSION {
        return Err(CodecError::Version("Brotli1.1.0 required"));
    }
    // Preserve the pinned convenience wrapper's special empty stream.
    if bytes.is_empty() {
        memory.call()?;
        return Ok(1);
    }
    let max = unsafe { BrotliEncoderMaxCompressedSize(bytes.len()) };
    if max == 0 {
        return Err(CodecError::Capacity);
    }
    let output = BufferAllocation::new(memory)?;
    memory.call()?;
    let state = unsafe {
        BrotliEncoderCreateInstance(
            Some(c_alloc),
            Some(c_free),
            (memory as *mut CodecMemory<'_, '_>).cast(),
        )
    };
    if state.is_null() {
        return Err(memory.result("Brotli initialization failed"));
    }
    let _guard = BrotliGuard(state);
    for (parameter, value) in [
        (BrotliEncoderParameter_BROTLI_PARAM_QUALITY, 11),
        (BrotliEncoderParameter_BROTLI_PARAM_LGWIN, 22),
        (
            BrotliEncoderParameter_BROTLI_PARAM_MODE,
            BrotliEncoderMode_BROTLI_MODE_GENERIC as u32,
        ),
        // This cast intentionally matches the pinned convenience API.
        (
            BrotliEncoderParameter_BROTLI_PARAM_SIZE_HINT,
            bytes.len() as u32,
        ),
    ] {
        memory.call()?;
        if unsafe { BrotliEncoderSetParameter(state, parameter, value) } == 0 {
            return Err(CodecError::Encoder("Brotli parameter rejected"));
        }
    }
    let mut remaining = bytes.len();
    let mut next = bytes.as_ptr();
    let mut size = 0usize;
    loop {
        let mut capacity = OUTPUT_CHUNK.min(max - size);
        let before_out = capacity;
        let before_in = remaining;
        let mut target = output.address;
        memory.call()?;
        let succeeded = unsafe {
            BrotliEncoderCompressStream(
                state,
                BrotliEncoderOperation_BROTLI_OPERATION_FINISH,
                &mut remaining,
                &mut next,
                &mut capacity,
                &mut target,
                ptr::null_mut(),
            )
        };
        // Budget/allocator denial is never converted to an eligible fallback.
        if memory.error.is_some() {
            return Err(memory.result("Brotli allocation failed"));
        }
        size = size
            .checked_add(before_out - capacity)
            .ok_or(CodecError::Capacity)?;
        if succeeded != 0 && unsafe { BrotliEncoderIsFinished(state) } != 0 {
            return Ok(size);
        }
        if succeeded == 0 || size == max {
            return uncompressed_brotli_size(bytes.len());
        }
        if before_in == remaining && before_out == capacity {
            return Err(CodecError::Encoder("Brotli made no progress"));
        }
    }
}

// Exact length of pinned1.1.0 encode.c::MakeUncompressedStream, the convenience
// API fallback when its MaxCompressedSize buffer cannot fit compressed output.
// This counts the wrapper's framing only; no second compressor/bit writer is
// implemented. Each full24-bit block has4 header bytes, the remainder has3/4,
// plus2 leading bytes and1 final byte. Empty input is the special single byte.
fn uncompressed_brotli_size(input: usize) -> Result<usize, CodecError> {
    if input == 0 {
        return Ok(1);
    }
    let full = input / (1 << 24);
    let remaining = input % (1 << 24);
    let last = if remaining == 0 {
        0
    } else if remaining > (1 << 20) {
        4
    } else {
        3
    };
    input
        .checked_add(full.checked_mul(4).ok_or(CodecError::Capacity)?)
        .and_then(|size| size.checked_add(last + 3))
        .ok_or(CodecError::Capacity)
}

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
