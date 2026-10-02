//! One C11 toolchain owner for native clients. Semantic compilation does not
//! execute processes; callers explicitly resolve a toolchain and link its C.
use crate::config::ProjectConfig;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct NativeToolchain {
    compiler: PathBuf,
    provenance: &'static str,
    flags: Vec<OsString>,
    sources: Vec<PathBuf>,
    libraries: Vec<OsString>,
    controls: Value,
}
fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn absolute(path: &Path, base: &Path) -> Result<PathBuf, String> {
    let path = if path.is_absolute() { path.to_owned() } else { base.join(path) };
    if path.is_absolute() { Ok(path) } else {
        Ok(std::env::current_dir().map_err(|e| format!("native working directory: {e}"))?.join(path))
    }
}
fn compiler_path(name: &OsStr, base: &Path) -> Result<PathBuf, String> {
    let name = Path::new(name);
    if name.components().count() > 1 || name.is_absolute() {
        return absolute(name, base);
    }
    // PATH lookup belongs to the explicitly requested external toolchain, never
    // the semantic compiler or its fingerprint.
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let path = directory.join(name);
        if path.is_file() {
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                if fs::metadata(&path).map_err(|e| e.to_string())?.permissions().mode() & 0o111 == 0 { continue; }
            }
            return absolute(&path, Path::new("."));
        }
        #[cfg(windows)] {
            let path = path.with_extension("exe");
            if path.is_file() { return absolute(&path, Path::new(".")); }
        }
    }
    Err(format!("native compiler `{}` was not found on PATH", name.display()))
}

impl NativeToolchain {
    /// TOML wins over the optional caller-provided CC adapter. Library clients
    /// pass None to avoid that adapter. No shell or implicit CFLAGS are read.
    pub fn resolve(config: &ProjectConfig, cc: Option<&OsStr>) -> Result<Self, String> {
        let native = &config.target.native;
        native.validate()?;
        let base = config.config_dir.as_deref().unwrap_or_else(|| Path::new("."));
        let (name, provenance, compiler_base) = match native.compiler.as_ref() {
            Some(path) => (path.as_os_str(), "target.native.compiler", base),
            None => match cc {
                Some(name) if !name.is_empty() => (name, "CC adapter", Path::new(".")),
                Some(_) => return Err("CC must name one native compiler executable".into()),
                None => (OsStr::new("cc"), "platform default", Path::new(".")),
            },
        };
        let compiler = compiler_path(name, compiler_base)?;
        let mut flags = vec!["-x".into(), "c".into(), "-std=c11".into(), native.objective.optimization_flag().into(),
            "-fno-fast-math".into(), "-ffp-contract=off".into()];
        if let Some(triple) = &native.triple { flags.push(format!("--target={triple}").into()); }
        if let Some(sysroot) = &native.sysroot {
            let mut flag = OsString::from("--sysroot="); flag.push(absolute(sysroot, base)?); flags.push(flag);
        }
        if native.debug_info { flags.push("-g".into()); }
        if native.warnings_as_errors { flags.extend(["-Wall".into(), "-Wextra".into(), "-Werror".into()]); }
        if native.link_time_optimization { flags.push("-flto".into()); }
        for sanitizer in &native.sanitizers { flags.push(sanitizer.flag().into()); }
        if !native.sanitizers.is_empty() { flags.push("-fno-omit-frame-pointer".into()); }
        let target = native.triple.as_deref().unwrap_or(std::env::consts::OS);
        let mut libraries = Vec::new();
        if !target.contains("windows") { libraries.push("-lm".into()); }
        if target.contains("darwin") || target == "macos" { libraries.push("-Wl,-no_uuid".into()); }
        let sources = config.host.native_sources.iter().map(|p| absolute(Path::new(p), base)).collect::<Result<_,_>>()?;
        Ok(Self { compiler, provenance, flags, sources, libraries, controls: json!(native) })
    }
    /// No compiler execution: suitable for --print-policy and build planners.
    pub fn receipt(&self) -> Value {
        json!({"schema":1,"compiler":self.compiler.to_string_lossy(),"compiler_os":self.compiler.as_os_str(),"provenance":self.provenance,
            "controls":self.controls,"flags":self.flags.iter().map(|flag|flag.to_string_lossy()).collect::<Vec<_>>(),
            "host_sources":self.sources.iter().map(|path|path.to_string_lossy()).collect::<Vec<_>>(),
            "host_sources_os":self.sources.iter().map(|path|path.as_os_str()).collect::<Vec<_>>(),
            "libraries":self.libraries.iter().map(|flag|flag.to_string_lossy()).collect::<Vec<_>>(),
            "numeric_contract":"C11; no fast math; floating-point contraction disabled"})
    }
    /// Compile unchanged admitted C. The writer and output drain run together;
    /// failed compilers cannot publish a partial executable over a previous one.
    /// The returned receipt measures the linked file, not C-source byte counts.
    pub fn compile(&self, c: &str, header: Option<&str>, output: &Path) -> Result<Value, String> {
        let parent = output.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        if output.file_name().is_none() || header.is_some() && output.with_extension("h") == output {
            return Err("native output must name an executable distinct from its generated .h file".into());
        }
        fs::create_dir_all(parent).map_err(|e| format!("native output directory: {e}"))?;
        let version = Command::new(&self.compiler).arg("--version").output()
            .map_err(|e| format!("failed to identify native compiler {}: {e}", self.compiler.display()))?;
        if !version.status.success() { return Err(format!("native compiler --version failed: {}", String::from_utf8_lossy(&version.stderr))); }
        let identity = fs::read(&self.compiler).map_err(|e| format!("native compiler identity: {e}"))?;
        let source_identities = self.sources.iter().map(|path| {
            fs::read(path).map(|bytes| json!({"path":path.to_string_lossy(),"path_os":path.as_os_str(),"sha256":hash(&bytes)}))
                .map_err(|e| format!("native host source {}: {e}", path.display()))
        }).collect::<Result<Vec<_>,_>>()?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let staged = parent.join(format!(".lilscript-native-{}-{}", std::process::id(), SERIAL.fetch_add(1,Ordering::Relaxed)));
        // Reserve a directory atomically, so staging never follows an existing
        // file/symlink and cleanup only removes this invocation's own file.
        fs::create_dir(&staged).map_err(|e| format!("native staging: {e}"))?;
        struct Staging(PathBuf);
        impl Drop for Staging { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
        let staging = Staging(staged);
        let temporary = staging.0.join("program");
        let header_path = output.with_extension("h");
        let temporary_header = header_path.file_name().map(|name| staging.0.join(name));
        if let Some(header) = header {
            fs::write(temporary_header.as_ref().ok_or("native output needs a file name")?, header)
                .map_err(|e| format!("native staging header: {e}"))?;
        }
        let mut command = Command::new(&self.compiler);
        command.args(&self.flags).arg("-o").arg(&temporary).arg("-").arg("-I").arg(&staging.0).arg("-I").arg(parent)
            .args(&self.sources).args(&self.libraries);
        let arguments = command.get_args().map(OsStr::to_owned).collect::<Vec<_>>();
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()
            .map_err(|e| format!("failed to start native compiler {}: {e}", self.compiler.display()))?;
        let mut stdin = child.stdin.take().expect("piped native input");
        let (result, written) = std::thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(c.as_bytes()));
            let result = child.wait_with_output();
            (result, writer.join())
        });
        let result = result.map_err(|e| format!("failed to wait for native compiler: {e}"))?;
        if !result.status.success() {
            return Err(format!("native compiler failed ({}):\n{}{}", result.status,
                String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr)));
        }
        written.map_err(|_| "native source writer panicked")?.map_err(|e| format!("failed to send native C: {e}"))?;
        let bytes = fs::read(&temporary).map_err(|e| format!("native compiler did not produce an executable: {e}"))?;
        // Reject source edits during the external compile; the receipt must name
        // the same complete inputs on both sides of the invocation.
        for (path, before) in self.sources.iter().zip(&source_identities) {
            if hash(&fs::read(path).map_err(|e| e.to_string())?) != before["sha256"].as_str().unwrap() {
                return Err(format!("native host source changed during compilation: {}", path.display()));
            }
        }
        if header.is_some() {
            fs::rename(temporary_header.as_ref().unwrap(), &header_path).map_err(|e| format!("failed to publish native header: {e}"))?;
        }
        fs::rename(&temporary, output).map_err(|e| format!("failed to publish native executable: {e}"))?;
        let mut receipt = self.receipt();
        receipt["compiler_sha256"] = json!(hash(&identity));
        receipt["compiler_version"] = json!(String::from_utf8_lossy(&version.stdout).trim());
        receipt["arguments"] = json!(arguments.iter().map(|argument|argument.to_string_lossy()).collect::<Vec<_>>());
        receipt["arguments_os"] = json!(arguments);
        receipt["c_sha256"] = json!(hash(c.as_bytes()));
        receipt["header_sha256"] = json!(header.map(|text| hash(text.as_bytes())));
        receipt["host_inputs"] = json!(source_identities);
        receipt["output"] = json!({"path":output.to_string_lossy(),"path_os":output.as_os_str(),"sha256":hash(&bytes),"bytes":bytes.len()});
        receipt["diagnostics"] = json!(String::from_utf8_lossy(&result.stderr));
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NativeObjective;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static SERIAL: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("lilscript n1 toolchain {} {}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(path)
        }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    #[test]
    fn n1_native_config_is_closed_validated_and_separate_from_js_objectives() {
        for settings in ["objective='brotli'", "triple='--bad target'", "sanitizers=['address','address']", "compiler=''", "fast_math=true"] {
            assert!(crate::config::parse_project_config(&format!("[target.native]\n{settings}")).is_err(), "{settings}");
        }
        let config = crate::config::parse_project_config("objective.codecs=['raw','gzip','brotli']\n[target.native]\nobjective='size'\nsanitizers=['address','undefined']\nwarnings_as_errors=true").unwrap().config;
        assert_eq!(config.target.native.objective, NativeObjective::Size);
        assert_eq!(config.objective.codecs.len(), 3);
    }
    #[test]
    fn n1_native_toolchain_executes_all_objectives_and_identifies_actual_files() {
        let scratch = Scratch::new();
        let c = "#include <stdio.h>\n#include <stdint.h>\nint main(void){volatile uint32_t n=2147483647;printf(\"%d\\n\",(int32_t)(n+1));return 0;}\n";
        for objective in [NativeObjective::Speed, NativeObjective::Size, NativeObjective::Balanced] {
            let mut config = ProjectConfig::default(); config.target.native.objective = objective;
            config.target.native.compiler = std::env::var_os("LILSCRIPT_NATIVE_CC").map(PathBuf::from);
            let toolchain = NativeToolchain::resolve(&config, None).unwrap();
            let output = scratch.0.join(format!("program {objective:?}"));
            let receipt = toolchain.compile(c, None, &output).unwrap();
            let run = Command::new(&output).output().unwrap();
            assert!(run.status.success()); assert_eq!(run.stdout, b"-2147483648\n");
            assert_eq!(receipt["c_sha256"], hash(c.as_bytes()));
            assert_eq!(receipt["output"]["sha256"], hash(&fs::read(&output).unwrap()));
            assert!(receipt["compiler_sha256"].as_str().unwrap().len() == 64);
            assert!(receipt["flags"].as_array().unwrap().iter().any(|v| v == "-ffp-contract=off"));
            assert!(receipt["flags"].as_array().unwrap().iter().any(|v| v == objective.optimization_flag()));
        }
    }
    #[cfg(unix)]
    #[test]
    fn n1_native_failure_drains_pipes_keeps_previous_outputs_and_prefers_config() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new();
        let compiler = scratch.0.join("fake compiler");
        fs::write(&compiler, "#!/bin/sh\nif [ \"$1\" = --version ]; then echo fake-n1; exit 0; fi\nhead -c 131072 /dev/zero >&2\necho 'intentional native refusal' >&2\nexit 23\n").unwrap();
        fs::set_permissions(&compiler, fs::Permissions::from_mode(0o755)).unwrap();
        let mut config = ProjectConfig::default(); config.config_dir = Some(scratch.0.clone());
        config.target.native.compiler = Some(PathBuf::from("./fake compiler"));
        let toolchain = NativeToolchain::resolve(&config, Some(OsStr::new("nonexistent-CC"))).unwrap();
        assert_eq!(toolchain.receipt()["provenance"], "target.native.compiler");
        let output = scratch.0.join("kept"); fs::write(&output, "original executable").unwrap();
        fs::write(output.with_extension("h"), "original header").unwrap();
        let error = toolchain.compile(&"x".repeat(1_000_000), Some("new header"), &output).unwrap_err();
        assert!(error.contains("intentional native refusal")); assert!(error.contains("23"));
        assert_eq!(fs::read_to_string(&output).unwrap(), "original executable");
        assert_eq!(fs::read_to_string(output.with_extension("h")).unwrap(), "original header");
        assert!(!fs::read_dir(&scratch.0).unwrap().any(|entry| entry.unwrap().file_name().to_string_lossy().starts_with(".lilscript-native-")));
    }
}
