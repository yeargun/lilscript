//! A bounded cache of completed, admitted build results. This owner cannot
//! supply a live checked program or grant a search permission: it returns only
//! the same immutable handoff for an identical complete request. Session APIs
//! deliberately remain cold. Corruption and IO failures are ordinary misses.
use super::*;
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

const MAGIC: &[u8; 8] = b"LILBLD01";
const PAYLOAD: usize = 4 * 1024 * 1024;
const HEADER: usize = 80;
const SLOTS: u64 = 64;
const SLOT_BYTES: u64 = (HEADER + PAYLOAD) as u64;
const FILE_BYTES: u64 = SLOTS * SLOT_BYTES;

pub(super) struct Request {
    file: PathBuf,
    salt: [u8; 32],
    key: [u8; 32],
    inputs: Value,
    maximum: usize,
    javascript: [bool; 3],
    outputs: Vec<(String, [bool; 3])>,
    native: bool,
    started: Instant,
    lookup_ns: u64,
    miss_reason: &'static str,
}

impl Request {
    fn new(config: &ProjectConfig, options: ServiceOptions) -> Result<Option<Self>, ServiceError> {
        // Explicit lock IO must execute: replay re-proves/re-judges, and an
        // explicit write must not be suppressed by a completed-build hit.
        if config.decisions.read.is_some() || config.decisions.write.is_some()
            || !config.cache.build_reuse || config.cache.directory.is_none() {
            return Ok(None);
        }
        // A physical audit must really execute the requested proofs/encoders.
        if !config.cache.formation_reuse
            || !config.cache.normalization_reuse
            || !config.cache.codec_reuse
            || !crate::schedule::reuses_stability()
            || std::env::var_os("LILSCRIPT_DEBUG_VERIFY").is_some()
        {
            return Ok(None);
        }
        #[cfg(test)]
        if SKIP_PROGRAM_RULES.with(std::cell::Cell::get)
            || SKIP_PROGRAM_INLINING.with(std::cell::Cell::get)
        {
            return Ok(None);
        }
        let started = Instant::now();
        let frontend = Frontend::new(config, options)?;
        let policy = frontend
            .javascript
            .as_ref()
            .or(frontend.native.as_ref())
            .unwrap();
        if policy.resources().wall_time_ms.is_some() {
            return Ok(None);
        }
        let maximum = options
            .retained_bytes
            .min(policy.resources().retained_bytes.unwrap_or(u64::MAX))
            .saturating_div(8)
            .min(PAYLOAD as u64) as usize;
        if maximum == 0 {
            return Ok(None);
        }
        let javascript = if options.target == ServiceTarget::Native {
            [false; 3]
        } else {
            let requested = options
                .requested_objectives(config)
                .map_err(|error| ServiceError::new("policy", error))?;
            [Objective::Raw, Objective::Gzip, Objective::Brotli]
                .map(|codec| requested.iter().any(|found| found == codec))
        };
        let mut outputs = Vec::new();
        if !frontend.additional_outputs.is_empty() {
            outputs.push(("primary".into(), javascript));
            for output in &frontend.additional_outputs {
                outputs.push((output.name.clone(), [Objective::Raw, Objective::Gzip, Objective::Brotli]
                    .map(|codec| output.policies.iter().any(|policy| policy.objective().unwrap().codec==codec))));
            }
        }
        let any = |index: usize| javascript[index] || outputs.iter().any(|(_, requested)| requested[index]);
        // A hit must not suppress the encoder's runtime identity refusal.
        if (any(1)
            && crate::compression::canonical_zlib_version().ok()
                != Some(crate::compression::CANONICAL_ZLIB_LIBRARY_VERSION))
            || (any(2)
                && crate::compression::canonical_brotli_version()
                    != crate::compression::CANONICAL_BROTLI_LIBRARY_VERSION)
        {
            return Ok(None);
        }
        let Some(compiler) = crate::cache_identity::compiler_identity() else {
            return Ok(None);
        };
        let mut hash = Sha256::new();
        hash.update(MAGIC);
        hash.update(compiler);
        // Include every setting, including resolution/host inputs not present
        // in a target policy receipt. Debug is a complete typed encoding here;
        // the running binary identity fixes that encoding. Cache location is
        // not a compilation input. Both physical audit flags remain in it.
        let mut settings = config.clone();
        settings.cache.directory = None;
        for field in [
            format!("{settings:?}"),
            format!("{options:?}"),
            format!("{:?}", std::env::current_dir().ok()),
        ] {
            hash.update((field.len() as u64).to_le_bytes());
            hash.update(field.as_bytes());
        }
        Ok(Some(Self {
            file: policy
                .cache()
                .directory
                .as_ref()
                .unwrap()
                .join("build-v1.bin"),
            salt: hash.finalize().into(),
            key: [0; 32],
            inputs: Value::Null,
            maximum,
            javascript,
            outputs,
            native: options.target != ServiceTarget::JavaScript,
            started,
            lookup_ns: 0,
            miss_reason: "not-read",
        }))
    }

    fn identify(&mut self, inputs: Value) {
        let mut hash = Sha256::new();
        hash.update(self.salt);
        // JSON object construction/order is shared with the cold frontend.
        serde_json::to_writer(HashWriter(&mut hash), &inputs).unwrap();
        self.key = hash.finalize().into();
        self.inputs = inputs;
    }

    pub(super) fn source(
        source: &str,
        config: &ProjectConfig,
        options: ServiceOptions,
    ) -> Result<Option<Self>, ServiceError> {
        let Some(mut request) = Self::new(config, options)? else {
            return Ok(None);
        };
        request.identify(source_inputs(source));
        Ok(Some(request))
    }

    pub(super) fn entries(
        entries: &[EntrySource],
        config: &ProjectConfig,
        options: ServiceOptions,
    ) -> Result<Option<Self>, ServiceError> {
        let Some(mut request) = Self::new(config, options)? else {
            return Ok(None);
        };
        let entries = sorted_entries(entries)?;
        // Rediscover, rather than trusting a saved file list: a newly created
        // import candidate/package mapping may change resolution while every
        // formerly read file has exactly the same bytes.
        let Some(inputs) = discover_inputs(&entries, config, options) else {
            return Ok(None);
        };
        request.identify(inputs);
        Ok(Some(request))
    }

    fn offset(&self) -> u64 {
        (u64::from(self.key[0]) % SLOTS) * SLOT_BYTES
    }

    fn read_payload(&self) -> Option<Vec<u8>> {
        let mut file = File::open(&self.file).ok()?;
        if !file.metadata().ok()?.is_file() || file.metadata().ok()?.len() > FILE_BYTES {
            return None;
        }
        file.seek(SeekFrom::Start(self.offset())).ok()?;
        let mut header = [0; HEADER];
        file.read_exact(&mut header).ok()?;
        if &header[..8] != MAGIC || header[8..40] != self.key {
            return None;
        }
        let length = u64::from_le_bytes(header[40..48].try_into().ok()?);
        if length > self.maximum as u64 {
            return None;
        }
        let mut payload = Vec::new();
        payload.try_reserve_exact(length as usize).ok()?;
        payload.resize(length as usize, 0);
        file.read_exact(&mut payload).ok()?;
        let mut hash = Sha256::new();
        hash.update(&header[..48]);
        hash.update(&payload);
        let digest: [u8; 32] = hash.finalize().into();
        (digest == header[48..80]).then_some(payload)
    }

    pub(super) fn read(&mut self) -> Option<ServiceCompilation> {
        let result = self.read_result();
        self.lookup_ns = nanos(self.started);
        result
    }

    fn read_result(&mut self) -> Option<ServiceCompilation> {
        self.miss_reason = "missing-or-invalid-record";
        let payload = self.read_payload();
        let payload = payload?;
        self.miss_reason = "invalid-serialization";
        let cached: CachedCompilation = serde_json::from_slice(&payload).ok()?;
        let serialized_bytes = payload.len();
        drop(payload);
        self.miss_reason = "invalid-handoff";
        let mut output = cached.restore(self.javascript, self.native, &self.outputs)?;
        self.miss_reason = "input-mismatch";
        if output.report["inputs"] != self.inputs {
            return None;
        }
        let cold_total = output.report["total_ns"].take();
        let cold_phases = output.report["phases_ns"].take();
        let cold_codec = output.report["codec_cache"].take();
        let elapsed = nanos(self.started);
        output.report["total_ns"] = elapsed.into();
        output.report["first_artifact_ns"] = elapsed.into();
        output.report["phases_ns"] = json!({"build_cache_ns": elapsed});
        output.report["codec_cache"] =
            json!({"memory_hits":0,"disk_hits":0,"encodes":0,"disk_write_errors":0});
        let mut cold_artifact_times = Vec::new();
        for (index, javascript) in output.javascript.iter_mut().enumerate() {
            let mut times = serde_json::Map::new();
            for key in [
                "formation_naming_render_ns",
                "codec_ns",
                "complete_artifact_ns",
            ] {
                if let Some(value) = javascript
                    .details
                    .as_object_mut()
                    .and_then(|details| details.remove(key))
                {
                    times.insert(key.into(), value);
                }
                if let Some(details) = output.report["artifacts"][index]["details"].as_object_mut()
                {
                    details.remove(key);
                }
            }
            cold_artifact_times.push(times);
        }
        output.report["build_cache"] = json!({
            "hit":true,"state":"hit","key":hex(&self.key),
            "elapsed_ns":elapsed,"serialized_bytes":serialized_bytes,
            "logical_receipt":"completed cold build for identical input, config, compiler and limits; no new search",
            "cold_total_ns":cold_total,"cold_phases_ns":cold_phases,"cold_codec_cache":cold_codec,"cold_artifact_times":cold_artifact_times,
        });
        Some(output)
    }

    fn write_record(&self, output: &ServiceCompilation) -> io::Result<usize> {
        let mut payload = LimitedBytes {
            bytes: Vec::new(),
            maximum: self.maximum,
        };
        serde_json::to_writer(&mut payload, output).map_err(io::Error::other)?;
        let payload = payload.bytes;
        let mut header = [0u8; HEADER];
        header[..8].copy_from_slice(MAGIC);
        header[8..40].copy_from_slice(&self.key);
        header[40..48].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        let mut hash = Sha256::new();
        hash.update(&header[..48]);
        hash.update(&payload);
        header[48..].copy_from_slice(&hash.finalize());
        std::fs::create_dir_all(self.file.parent().unwrap())?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.file)?;
        if file.metadata()?.len() > FILE_BYTES {
            return Err(io::Error::other("oversized cache"));
        }
        // Separate file handles have independent offsets. A torn/concurrent
        // write cannot be accepted unless its full header/payload checksum fits.
        file.seek(SeekFrom::Start(self.offset() + HEADER as u64))?;
        file.write_all(&payload)?;
        file.seek(SeekFrom::Start(self.offset()))?;
        file.write_all(&header)?;
        Ok(payload.len())
    }

    pub(super) fn write(&mut self, output: &mut ServiceCompilation) {
        let started = Instant::now();
        // The files may have changed while the cold build ran. Only persist
        // when its actual checked graph/code equals the preflight snapshot.
        let (state, bytes) = if output.report["inputs"] != self.inputs {
            ("input-changed", None)
        } else {
            match self.write_record(output) {
                Ok(bytes) => ("stored", Some(bytes)),
                Err(_) => ("not-stored", None),
            }
        };
        let cold_compilation_ns = output.report["total_ns"].take();
        output.report["total_ns"] = nanos(self.started).into();
        if let Some(first) = output.report["first_artifact_ns"].as_u64() {
            output.report["first_artifact_ns"] = first.saturating_add(self.lookup_ns).into();
        }
        output.report["build_cache"] = json!({
            "hit":false,"state":state,"cold_compilation_ns":cold_compilation_ns,"key":hex(&self.key),"lookup_ns":self.lookup_ns,
            "write_ns":nanos(started),"serialized_bytes":bytes,"miss_reason":self.miss_reason,
        });
    }
}

/// Exact same source/host input owner as checking, but without elaboration.
/// On a miss the cold build repeats parsing; that deliberate tradeoff avoids
/// persisting borrowed syntax or reusing an incompletely invalidated graph.
fn discover_inputs(
    entries: &[EntrySource],
    config: &ProjectConfig,
    options: ServiceOptions,
) -> Option<Value> {
    let mut frontend = Frontend::new(config, options).ok()?;
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let result = (|| {
        let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
        let (modules, syntax) =
            discover_parsed_modules_admitted(entries, None, config, &sources, &arena).ok()?;
        let hosts = match host_requests(config, frontend.javascript.as_ref(), &modules) {
            Some((directory, requests, edition)) => {
                crate::host_modules::deliver(directory, &requests, edition).ok()?
            }
            None => Default::default(),
        };
        let inputs = module_inputs(&modules, &hosts);
        drop(syntax);
        Some(inputs)
    })();
    release_source_buffers(sources, &mut frontend.ledger);
    result
}

struct HashWriter<'a>(&'a mut Sha256);
impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct LimitedBytes {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for LimitedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let size = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|size| *size <= self.maximum)
            .ok_or_else(|| io::Error::other("build cache record exceeds limit"))?;
        if size > self.bytes.capacity() {
            let capacity = size
                .checked_next_power_of_two()
                .unwrap_or(self.maximum)
                .min(self.maximum);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// Private deserialization types keep the public service's invariants intact.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedCompilation {
    javascript: Vec<CachedJavaScript>,
    winners: [Option<usize>; 3],
    outputs: Vec<ServiceOutput>,
    native_c: Option<String>,
    native_header: Option<String>,
    report: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CachedJavaScript {
    javascript: String,
    files: Vec<DeliveredFile>,
    layout: Option<crate::js::delivery::DeliveredLayout>,
    sha256: String,
    sizes: Sizes,
    details: Value,
}
impl CachedCompilation {
    fn restore(self, requested: [bool; 3], native: bool, outputs: &[(String, [bool; 3])]) -> Option<ServiceCompilation> {
        if !self.report.is_object()
            || self.javascript.len() > 3 * outputs.len().max(1)
            || self.native_c.is_some() != native
        {
            return None;
        }
        if self.outputs.len() != outputs.len() { return None; }
        for (actual, (name, wanted)) in self.outputs.iter().zip(outputs) {
            if actual.name() != name { return None; }
            for (index, wanted) in wanted.iter().enumerate() {
                if *wanted != actual.winners[index].is_some() { return None; }
                if let Some(index) = actual.winners[index] { self.javascript.get(index)?; }
            }
        }
        if self.outputs.first().is_some_and(|primary| primary.winners != self.winners) { return None; }
        if self.report["native_sha256"]
            != json!(self.native_c.as_ref().map(|text| digest(text.as_bytes())))
            || self.report["native_header_sha256"]
                != json!(self
                    .native_header
                    .as_ref()
                    .map(|text| digest(text.as_bytes())))
        {
            return None;
        }
        let artifacts = self.report.get("artifacts")?.as_array()?;
        if artifacts.len() != self.javascript.len()
            || artifacts
                .iter()
                .any(|artifact| !artifact.get("details").is_some_and(Value::is_object))
        {
            return None;
        }
        for (index, wanted) in requested.into_iter().enumerate() {
            if wanted != self.winners[index].is_some() {
                return None;
            }
            if let Some(winner) = self.winners[index] {
                // Direct low-effort handoffs may intentionally leave a codec
                // unmeasured. Preserve that absence; do not invent a score or
                // reject a valid handoff just because no search ran.
                self.javascript.get(winner)?;
            }
        }
        let javascript = self
            .javascript
            .into_iter()
            .map(CachedJavaScript::restore)
            .collect::<Option<_>>()?;
        Some(ServiceCompilation {
            javascript,
            winners: self.winners,
            outputs: self.outputs,
            native_c: self.native_c,
            native_header: self.native_header,
            report: self.report,
        })
    }
}
impl CachedJavaScript {
    fn restore(self) -> Option<ServiceJavaScript> {
        if !self.details.is_object() {
            return None;
        }
        let expected = if let Some(layout) = &self.layout {
            if !self.javascript.is_empty()
                || self.files.is_empty()
                || self.files.len() != layout.files.len()
            {
                return None;
            }
            let count = self.files.len();
            if layout.entries.iter().any(|entry| {
                entry.file as usize >= count
                    || entry.closure.iter().any(|&file| file as usize >= count)
            }) || layout.files.iter().any(|file| {
                file.imports
                    .iter()
                    .chain(&file.dynamic)
                    .any(|&file| file as usize >= count)
            }) {
                return None;
            }
            let mut hash = Sha256::new();
            for file in &self.files {
                if file.sizes.raw != file.code.len()
                    || file.name.is_empty()
                    || file.name.contains('\0')
                    || !Path::new(&file.name)
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                {
                    return None;
                }
                hash.update(file.name.as_bytes());
                hash.update([0]);
                hash.update(digest(file.code.as_bytes()).as_bytes());
                hash.update(b"\n");
            }
            hex(&hash.finalize())
        } else {
            if !self.files.is_empty() || self.sizes.raw != self.javascript.len() {
                return None;
            }
            digest(self.javascript.as_bytes())
        };
        if expected != self.sha256 {
            return None;
        }
        Some(ServiceJavaScript {
            javascript: self.javascript,
            files: self.files,
            layout: self.layout,
            sha256: self.sha256,
            sizes: self.sizes,
            details: self.details,
        })
    }
}

#[cfg(test)]
#[path = "build_cache_tests.rs"]
mod tests;
