//! Explicit, bounded choice exchange. A lock is an untrusted proposal. It is
//! neither a completed-build cache nor a serialized semantic certificate.
use super::*;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const FORMAT: u32 = 1;
const MAXIMUM: usize = 1024 * 1024;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: u32,
    fingerprint: String,
    // Canonical raw/gzip/Brotli order. No sizes or former verdicts enter replay.
    assignments: [Option<SavedDecision>; 3],
}

pub(super) struct Request {
    read: Option<PathBuf>,
    write: Option<PathBuf>,
    salt: Option<[u8; 32]>,
    fingerprint: String,
    maximum: usize,
    pub assignments: [Option<SavedDecision>; 3],
    pub report: Value,
}

impl Request {
    pub(super) fn new(
        config: &ProjectConfig,
        options: ServiceOptions,
    ) -> Result<Self, ServiceError> {
        let configured = config.decisions.read.is_some() || config.decisions.write.is_some();
        if configured && options.javascript_request().is_none() {
            return Err(ServiceError::new(
                "decisions",
                "decision locks require a JavaScript target",
            ));
        }
        let path = |value: &Option<PathBuf>| -> Result<Option<PathBuf>, ServiceError> {
            value
                .as_ref()
                .map(|path| {
                    if path.as_os_str().is_empty() {
                        return Err(ServiceError::new(
                            "decisions",
                            "lock path must not be empty",
                        ));
                    }
                    Ok(config
                        .config_dir
                        .as_deref()
                        .unwrap_or_else(|| Path::new("."))
                        .join(path))
                })
                .transpose()
        };
        let read = path(&config.decisions.read)?;
        let write = path(&config.decisions.write)?;
        let mut salt = None;
        if configured {
            let compiler = crate::cache_identity::compiler_identity().ok_or_else(|| {
                ServiceError::new("decisions", "cannot identify running compiler")
            })?;
            let mut settings = config.clone();
            settings.decisions = Default::default();
            settings.cache = Default::default();
            let mut hash = Sha256::new();
            hash.update(b"lilscript-decisions-v1");
            hash.update(compiler);
            for field in [
                format!("{settings:?}"),
                format!("{options:?}"),
                format!("{:?}", std::env::current_dir().ok()),
            ] {
                hash.update((field.len() as u64).to_le_bytes());
                hash.update(field.as_bytes());
            }
            salt = Some(hash.finalize().into());
        }
        let effective = if configured {
            options
                .resolve_javascript_policies(config)
                .map_err(|error| ServiceError::new("decisions", error))?
                .iter()
                .filter_map(|policy| policy.resources().retained_bytes)
                .fold(options.retained_bytes, u64::min)
        } else {
            options.retained_bytes
        };
        Ok(Self {
            read,
            write,
            salt,
            fingerprint: String::new(),
            maximum: (effective / 16).min(MAXIMUM as u64) as usize,
            assignments: [None, None, None],
            report: if configured {
                json!({"read":"disabled", "write":"disabled"})
            } else {
                Value::Null
            },
        })
    }

    pub(super) fn identify(&mut self, inputs: &Value) {
        let Some(salt) = self.salt else {
            return;
        };
        let mut hash = Sha256::new();
        hash.update(salt);
        hash.update(serde_json::to_vec(inputs).unwrap());
        self.fingerprint = format!("{:x}", hash.finalize());
        self.report["fingerprint"] = json!(self.fingerprint);
        if let Some(path) = &self.read {
            let result = (|| -> Result<Document, &'static str> {
                let mut file = File::open(path).map_err(|_| "unreadable")?;
                let metadata = file.metadata().map_err(|_| "unreadable")?;
                if !metadata.is_file() || metadata.len() > self.maximum as u64 {
                    return Err("oversized-or-not-file");
                }
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(metadata.len() as usize)
                    .map_err(|_| "capacity")?;
                (&mut file)
                    .take(self.maximum as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "unreadable")?;
                if bytes.len() > self.maximum {
                    return Err("oversized-or-not-file");
                }
                let document: Document =
                    serde_json::from_slice(&bytes).map_err(|_| "invalid-document")?;
                if document.format != FORMAT {
                    return Err("format-mismatch");
                }
                if document.fingerprint != self.fingerprint {
                    return Err("fingerprint-mismatch");
                }
                Ok(document)
            })();
            self.report["path"] = json!(path);
            match result {
                Ok(document) => {
                    self.assignments = document.assignments;
                    self.report["read"] = json!("matched");
                    self.report["objectives"] = json!(["raw", "gzip", "brotli"]
                        .into_iter()
                        .zip(&self.assignments)
                        .filter_map(|(name, assignment)| assignment.as_ref().map(|_| name))
                        .collect::<Vec<_>>());
                }
                Err(reason) => {
                    self.report["read"] = json!("miss");
                    self.report["reason"] = json!(reason);
                }
            }
        }
    }

    pub(super) fn is_writing(&self) -> bool {
        self.write.is_some()
    }

    pub(super) fn finish(&mut self, output: &mut ServiceCompilation) -> Result<(), ServiceError> {
        if let Some(path) = &self.write {
            let assignments = output
                .winners
                .map(|winner| {
                    winner
                        .map(|index| {
                            serde_json::from_value(
                                output.javascript[index].details["decision"].clone(),
                            )
                            .map_err(|_| {
                                ServiceError::new(
                                    "decisions",
                                    "selected implementation has no complete replay recipe",
                                )
                            })
                        })
                        .transpose()
                })
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
            let document = Document {
                format: FORMAT,
                fingerprint: self.fingerprint.clone(),
                assignments: assignments.try_into().unwrap(),
            };
            let mut bytes = Bounded {
                bytes: Vec::new(),
                maximum: self.maximum,
            };
            serde_json::to_writer_pretty(&mut bytes, &document)
                .map_err(|error| ServiceError::new("decisions write", error))?;
            bytes
                .write_all(b"\n")
                .map_err(|error| ServiceError::new("decisions write", error))?;
            atomic_write(path, &bytes.bytes).map_err(|error| {
                ServiceError::new("decisions write", format!("{}: {error}", path.display()))
            })?;
            self.report["write"] = json!("written");
            self.report["written_path"] = json!(path);
            self.report["bytes"] = json!(bytes.bytes.len());
        }
        if !self.report.is_null() {
            output.report["decisions"] = self.report.clone();
        }
        Ok(())
    }
}

struct Bounded {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Write for Bounded {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(value.len())
            .filter(|next| *next <= self.maximum)
            .ok_or_else(|| io::Error::other("decision lock exceeds size limit"))?;
        if next > self.bytes.capacity() {
            let capacity = next
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.maximum);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(io::Error::other)?;
        }
        self.bytes.extend_from_slice(value);
        Ok(value.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Same-directory replace: a failed write never truncates the previous lock.
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut opened = None;
    for _ in 0..16 {
        let temporary = parent.join(format!(
            ".lilscript-choices-{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                opened = Some((temporary, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    let (temporary, mut file) =
        opened.ok_or_else(|| io::Error::other("cannot create temporary decision lock"))?;
    let result = (|| {
        file.write_all(bytes)?;
        file.flush()?;
        drop(file);
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "build_decisions_tests.rs"]
mod tests;
