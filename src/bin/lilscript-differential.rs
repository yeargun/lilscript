use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bumpalo::Bump;
use clap::{Parser, ValueEnum};
use lilscript::{analyze, interpret_program, parse_source};
use serde_json::json;
use sha2::{Digest, Sha256};

const DEFAULT_SEED: u64 = 0x6c69_6c73_6372_6970;

#[derive(Debug, Parser)]
#[command(name = "lilscript-differential")]
#[command(
    about = "Generate deterministic programs and compare the compiler's output with the reference interpreter."
)]
struct Args {
    /// Number of generated functions and result rows.
    #[arg(long, default_value_t = 64)]
    cases: usize,

    /// Reproduction seed, accepted in decimal or with a 0x prefix.
    #[arg(long, default_value = "0x6c696c7363726970", value_parser = parse_seed)]
    seed: u64,

    /// Draw the seed from system entropy instead of the pinned default.
    ///
    /// The pinned seed makes this a regression corpus: the same programs run on
    /// every commit, so a shape it never generates is a shape nobody checks.
    /// `--random-seed` makes it a generator. The chosen seed is printed before
    /// any work starts and repeated on every divergence, so a failing run is
    /// replayed with `--seed <printed value>`.
    #[arg(long, conflicts_with = "seed")]
    random_seed: bool,

    /// Compiler executable. Defaults to the lilscript binary beside this executable.
    #[arg(long)]
    compiler: Option<PathBuf>,

    /// Directory for generated sources and compiled artifacts.
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Portable typed programs, JavaScript records, or both independent masks.
    #[arg(long, value_enum, default_value_t = Features::All)]
    features: Features,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Features {
    Portable,
    Javascript,
    All,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = Args::parse();
    if args.cases == 0 {
        return Err("--cases must be greater than zero".into());
    }
    if args.random_seed {
        args.seed = entropy_seed();
    }
    eprintln!(
        "lilscript-differential: seed {:#018x}, {} cases per mask",
        args.seed, args.cases
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = args.output_dir.clone().unwrap_or_else(|| {
        root.join(format!(
            "target/differential/{}-{:x}",
            std::process::id(),
            args.seed
        ))
    });
    if output_dir.exists()
        && fs::read_dir(&output_dir)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("output directory must be empty; preserve previous reproductions".into());
    }
    fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    let compiler = args.compiler.clone().unwrap_or(
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .unwrap()
            .join(executable_name("lilscript")),
    );
    let compiler = compiler
        .canonicalize()
        .map_err(|e| format!("compiler {}: {e}", compiler.display()))?;
    let mut report = json!({"schema":1,"complete":false,"seed":args.seed,"cases_per_mask":args.cases,
        "compiler":{"path":compiler,"sha256":format!("{:x}",Sha256::digest(fs::read(&compiler).map_err(|e| e.to_string())?))},"batches":[]});
    let result = (|| {
        for javascript in [false, true] {
            if (javascript && args.features == Features::Portable)
                || (!javascript && args.features == Features::Javascript)
            {
                continue;
            }
            let batch = run_batch(&args, &root, &output_dir, &compiler, javascript)?;
            report["batches"].as_array_mut().unwrap().push(batch);
        }
        Ok::<(), String>(())
    })();
    report["complete"] = json!(result.is_ok());
    if let Err(error) = &result {
        report["error"] = json!(error);
    }
    fs::write(
        output_dir.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .map_err(|e| e.to_string())?;
    result?;
    println!(
        "Independent typed programs matched every eligible lane (seed {:#018x}); {}",
        args.seed,
        output_dir.display()
    );
    Ok(())
}

fn run_batch(
    args: &Args,
    root: &Path,
    directory: &Path,
    compiler: &Path,
    javascript_only: bool,
) -> Result<serde_json::Value, String> {
    let mask = if javascript_only {
        "javascript-records"
    } else {
        "portable"
    };
    let output_dir = directory.join(mask);
    fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    let source_path = output_dir.join("generated.lil");
    let source = ProgramGenerator::new(args.seed).generate_mask(args.cases, javascript_only);
    // Retain even a generator/checker/interpreter failure, before invoking any compiler.
    fs::write(&source_path, &source).map_err(|e| e.to_string())?;
    let arena = Bump::new();
    let program = parse_source(&arena, &source).map_err(|e| format!("{mask}: parse: {e}"))?;
    let semantics = analyze(&program).map_err(|e| format!("{mask}: check: {e}"))?;
    let expected =
        interpret_program(&program, &semantics).map_err(|e| format!("{mask}: reference: {e}"))?;
    fs::write(output_dir.join("expected.out"), &expected).map_err(|e| e.to_string())?;
    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());
    let cc = std::env::var_os("LILSCRIPT_NATIVE_CLANG")
        .or_else(|| std::env::var_os("CC"))
        .unwrap_or_else(|| "clang".into());
    let mut lanes = Vec::new();
    for (index, (mode, codec)) in [
        ("production", "raw"),
        ("production", "gzip"),
        ("production", "brotli"),
        ("development", "raw"),
        ("formation-only", "raw"),
    ]
    .iter()
    .enumerate()
    {
        let config = output_dir.join(format!("lane-{index}.toml"));
        let text = if *mode == "formation-only" {
            fs::read_to_string(root.join("tests/config/no-optimization.toml"))
                .map_err(|e| e.to_string())?
        } else {
            format!("[policy]\nversion=3\n[objective]\ncodecs=[\"{codec}\"]\n[effort]\nlevel=13\n")
        };
        fs::write(&config, text).map_err(|e| e.to_string())?;
        let native = !javascript_only && *codec == "raw";
        let output = output_dir.join(format!("lane-{index}"));
        let lane = format!("{mask}/{mode}/{codec}");
        run_checked(
            Command::new(compiler)
                .arg(&source_path)
                .args([
                    "--target",
                    if native { "all" } else { "js" },
                    "--mode",
                    if *mode == "development" {
                        "development"
                    } else {
                        "production"
                    },
                ])
                .arg("--config")
                .arg(&config)
                .arg("-o")
                .arg(if native {
                    output.clone()
                } else {
                    output.with_extension("js")
                }),
            &format!("{lane} compilation"),
        )?;
        compare_output(
            &lane,
            &expected,
            run_checked(Command::new(&node).arg(output.with_extension("js")), &lane)?,
            args.seed,
            &source_path,
        )?;
        lanes.push(format!("{lane}/javascript"));
        if native {
            compare_output(
                &lane,
                &expected,
                run_checked(&mut Command::new(&output), &lane)?,
                args.seed,
                &source_path,
            )?;
            lanes.push(format!("{lane}/native"));
            let c_binary = output.with_extension("from-c");
            run_checked(
                Command::new(&cc)
                    .args(["-std=c11", "-O3", "-fno-fast-math", "-ffp-contract=off"])
                    .arg(output.with_extension("c"))
                    .arg("-o")
                    .arg(&c_binary)
                    .arg("-lm"),
                "independent C compilation",
            )?;
            compare_output(
                &lane,
                &expected,
                run_checked(&mut Command::new(c_binary), &lane)?,
                args.seed,
                &source_path,
            )?;
            lanes.push(format!("{lane}/independent-c"));
        }
    }
    Ok(
        json!({"mask":mask,"source_sha256":format!("{:x}",Sha256::digest(source.as_bytes())),"oracle_sha256":format!("{:x}",Sha256::digest(expected.as_bytes())),"lanes":lanes,
        "native_mask_reason":if javascript_only {Some("Record values are owned by native completion N2")} else {None}}),
    )
}

/// A seed drawn from the clock and the process id, mixed so that two runs
/// started in the same millisecond do not produce adjacent generator states.
///
/// `SplitMix64`'s finalizer, which is a bijection: distinct inputs stay
/// distinct, so the mixing cannot collapse two starts onto one corpus.
fn entropy_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64);
    let mut z = nanos ^ u64::from(std::process::id()).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn parse_seed(value: &str) -> Result<u64, String> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|error| error.to_string())
    } else {
        value.parse::<u64>().map_err(|error| error.to_string())
    }
}

fn executable_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn run_checked(command: &mut Command, description: &str) -> Result<Output, String> {
    let rendered = format!("{command:?}");
    let output = command
        .output()
        .map_err(|error| format!("failed to run {description} ({rendered}): {error}"))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "{description} failed with {} ({rendered}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn compare_output(
    backend: &str,
    expected: &str,
    output: Output,
    seed: u64,
    source_path: &Path,
) -> Result<(), String> {
    let actual = String::from_utf8(output.stdout)
        .map_err(|error| format!("{backend} emitted non-UTF-8 output: {error}"))?;
    if actual == expected {
        return Ok(());
    }
    let mismatch = expected
        .lines()
        .zip(actual.lines())
        .position(|(expected, actual)| expected != actual)
        .unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
    let expected_line = expected.lines().nth(mismatch).unwrap_or("<end of output>");
    let actual_line = actual.lines().nth(mismatch).unwrap_or("<end of output>");
    Err(format!(
        "{backend} diverged from the reference evaluator at output line {}\nexpected: {expected_line}\nactual:   {actual_line}\nseed: {seed:#018x}\nsource: {}",
        mismatch + 1,
        source_path.display()
    ))
}

struct ProgramGenerator {
    random: Random,
}

impl ProgramGenerator {
    fn new(seed: u64) -> Self {
        Self {
            random: Random::new(seed),
        }
    }

    #[cfg(test)]
    fn generate(&mut self, cases: usize) -> String {
        self.generate_mask(cases, false)
    }

    fn generate_mask(&mut self, cases: usize, javascript_only: bool) -> String {
        let mut source = String::from(
            "int differentialCalls=0;\nbool differentialProbe(int value){differentialCalls++;return (value&1)==0;}\nint differentialRotate(int value,int amount){return (value<<amount)|(value>>>(32-amount));}\n",
        );
        let mut calls = String::new();
        for case in 0..cases {
            self.generate_case(case, &mut source);
            if javascript_only {
                let key = format!("field{}", self.random.bounded(4));
                let value = self.integer_expression(2, &["x", "y"]);
                writeln!(source, "int recordCase{case}(int x,int y){{Record<int> a=record{{{key}:x}};Record<int> alias=a;int saved=a.{key}??0;alias[\"{key}\"]={value};return saved+(a.{key}??y);}}").unwrap();
            }
            let lhs = self.random.literal();
            let rhs = self.random.literal();
            writeln!(calls, "print(differentialCase{case}({lhs},{rhs}));")
                .expect("writing to String cannot fail");
            if javascript_only {
                writeln!(calls, "print(recordCase{case}({lhs},{rhs}));").unwrap();
            }
        }
        source.push_str(&calls);
        source.push_str("print(differentialCalls);\n");
        source
    }

    fn generate_case(&mut self, case: usize, source: &mut String) {
        let first = self.integer_expression(3, &["x", "y"]);
        let second = self.integer_expression(3, &["x", "y", "a"]);
        let assignment = self.assignment_operator();
        let assignment_rhs = self.integer_expression(2, &["x", "y", "a", "b"]);
        let condition = self.boolean_expression(2, &["x", "y", "a", "b"]);
        let then_value = self.integer_expression(2, &["x", "y", "a", "b"]);
        let else_value = self.integer_expression(2, &["x", "y", "a", "b"]);
        let loop_limit = 3 + self.random.bounded(5);
        let continue_at = self.random.bounded(loop_limit);
        let mut break_at = self.random.bounded(loop_limit);
        if break_at == continue_at {
            break_at = (break_at + 1) % loop_limit;
        }
        let while_limit = 1 + self.random.bounded(4);
        let shadow = self.integer_expression(2, &["x", "y", "a", "b"]);
        let gate_left = self.integer_expression(2, &["x", "y", "a", "b"]);
        let gate_right = self.integer_expression(2, &["x", "y", "a", "b"]);
        let rotate = 1 + self.random.bounded(31);
        let array_pipeline = match case % 4 {
            0 => "int[] mapped=values.map((int value)=>{if(values.length==2){values.push(value^x);}return value+y;});alias[0]+=mapped.length;b+=values.length+mapped.length+mapped[0]+alias[0]+appended+removed;",
            1 => "int[] selected=values.filter((int value)=>{if(values.length==2){values.push(value^x);}return ((value^y)&1)==0;});alias[0]+=selected.length;b+=values.length+selected.length+alias[0]+appended+removed;",
            2 => "int folded=values.reduce((int total,int value)=>{if(values.length==2){values.push(value^x);}return total+value;},0);alias[0]+=values.length;b+=values.length+folded+alias[0]+appended+removed;",
            _ => "values.forEach((int value)=>{if(values.length==2){values.push(value^x);}});alias[0]+=values.length;b+=values.length+values[2]+alias[0]+appended+removed;",
        };

        writeln!(source, "int differentialCase{case}(int x,int y){{")
            .expect("writing to String cannot fail");
        writeln!(source, "int a={first};int b={second};").expect("writing to String cannot fail");
        writeln!(source, "b{assignment}{assignment_rhs};").expect("writing to String cannot fail");
        writeln!(
            source,
            "if({condition}){{a={then_value};}}else{{a={else_value};}}"
        )
        .expect("writing to String cannot fail");
        writeln!(
            source,
            "for(int i=0;i<{loop_limit};i++){{if(i=={continue_at}){{continue;}}b+=differentialRotate(a^x,i+{rotate});if(i=={break_at}){{break;}}a^=b+i;}}"
        )
        .expect("writing to String cannot fail");
        writeln!(
            source,
            "int j=0;while(j<{while_limit}){{b=(b^(a>>>j))+y;j++;}}"
        )
        .expect("writing to String cannot fail");
        writeln!(
            source,
            "bool gate=differentialProbe({gate_left})&&((a^b)<0)||differentialProbe({gate_right});"
        )
        .expect("writing to String cannot fail");
        writeln!(source, "int old=b++;b+=old;if(gate){{--b;}}else{{++b;}}")
            .expect("writing to String cannot fail");
        writeln!(source, "int[] values=[a,b];int[] alias=values;int prior=alias[0]++;alias[1]+=prior;int appended=values.push(x^y);int removed=values.pop();{array_pipeline}")
            .expect("writing to String cannot fail");
        // Evaluate the source before introducing the nested `a`. LilScript
        // puts a lexical binding in scope for its entire initializer, so
        // `{ int a = a; }` is a self-read rather than a read of the outer `a`.
        // Keeping the value in a temporary preserves the intended shadowing
        // coverage without generating an invalid program.
        writeln!(
            source,
            "{{int shadowSource={shadow};{{int a=shadowSource;b+=a;}}}}return b;}}"
        )
        .expect("writing to String cannot fail");
    }

    fn integer_expression(&mut self, depth: usize, variables: &[&str]) -> String {
        if depth == 0 || self.random.bounded(5) == 0 {
            if !variables.is_empty() && self.random.bounded(3) != 0 {
                return variables[self.random.bounded(variables.len() as u32) as usize].to_string();
            }
            return self.random.literal();
        }
        let lhs = self.integer_expression(depth - 1, variables);
        let choice = self.random.bounded(12);
        if choice == 0 {
            return format!("(-{lhs})");
        }
        let rhs = if choice >= 8 {
            self.random.shift_literal()
        } else if choice == 4 || choice == 5 {
            self.random.small_literal_including_zero()
        } else {
            self.integer_expression(depth - 1, variables)
        };
        let operator = match choice {
            1 => "+",
            2 => "-",
            3 => "*",
            4 => "/",
            5 => "%",
            6 => "&",
            7 => "|",
            8 => "^",
            9 => "<<",
            10 => ">>",
            _ => ">>>",
        };
        format!("({lhs}{operator}{rhs})")
    }

    fn boolean_expression(&mut self, depth: usize, variables: &[&str]) -> String {
        if depth > 0 && self.random.bounded(4) == 0 {
            let lhs = self.boolean_expression(depth - 1, variables);
            let rhs = self.boolean_expression(depth - 1, variables);
            let operator = if self.random.bounded(2) == 0 {
                "&&"
            } else {
                "||"
            };
            return format!("({lhs}{operator}{rhs})");
        }
        let lhs = self.integer_expression(depth.min(2), variables);
        let rhs = self.integer_expression(depth.min(2), variables);
        let operator = ["==", "!=", "<", "<=", ">", ">="][self.random.bounded(6) as usize];
        let comparison = format!("({lhs}{operator}{rhs})");
        if self.random.bounded(4) == 0 {
            format!("!{comparison}")
        } else {
            comparison
        }
    }

    fn assignment_operator(&mut self) -> &'static str {
        [
            "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", ">>>=",
        ][self.random.bounded(11) as usize]
    }
}

struct Random(u64);

impl Random {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 { DEFAULT_SEED } else { seed })
    }

    fn next_u32(&mut self) -> u32 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        (value.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }

    fn bounded(&mut self, upper: u32) -> u32 {
        debug_assert!(upper > 0);
        ((u64::from(self.next_u32()) * u64::from(upper)) >> 32) as u32
    }

    fn literal(&mut self) -> String {
        let magnitude = self.bounded(1_000_000_001) as i32;
        if self.bounded(2) == 0 {
            magnitude.to_string()
        } else {
            format!("(-{magnitude})")
        }
    }

    fn small_literal_including_zero(&mut self) -> String {
        parenthesize_negative(self.bounded(11) as i32 - 5)
    }

    fn shift_literal(&mut self) -> String {
        parenthesize_negative(self.bounded(81) as i32 - 40)
    }
}

fn parenthesize_negative(value: i32) -> String {
    if value < 0 {
        format!("({value})")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_deterministic_and_checked() {
        let first = ProgramGenerator::new(7).generate(8);
        let second = ProgramGenerator::new(7).generate(8);
        assert_eq!(first, second);
        let arena = Bump::new();
        let program = parse_source(&arena, &first).unwrap_or_else(|error| {
            let start = error.span.start.saturating_sub(40);
            let end = (error.span.end + 40).min(first.len());
            panic!("{error}: {}", &first[start..end]);
        });
        let semantics = analyze(&program).unwrap();
        let output = interpret_program(&program, &semantics).unwrap();
        // One typed case result and a final observable side-effect counter.
        assert_eq!(output.lines().count(), 8 + 1);
    }

    #[test]
    fn seed_parser_accepts_decimal_and_hex() {
        assert_eq!(parse_seed("42").unwrap(), 42);
        assert_eq!(parse_seed("0x2a").unwrap(), 42);
    }

    #[test]
    fn javascript_mask_is_checked_without_polluting_portable_programs() {
        for seed in [0, 1, 42, u64::MAX] {
            for javascript in [false, true] {
                let source = ProgramGenerator::new(seed).generate_mask(8, javascript);
                assert_eq!(source.contains("Record<int>"), javascript);
                assert!(!source.contains("differentialMemory"));
                let arena = Bump::new();
                let program = parse_source(&arena, &source).unwrap();
                let semantics = analyze(&program).unwrap();
                let output = interpret_program(&program, &semantics).unwrap();
                assert_eq!(output.lines().count(), if javascript { 17 } else { 9 });
            }
        }
    }
}
