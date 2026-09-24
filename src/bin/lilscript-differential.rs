use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bumpalo::Bump;
use clap::Parser;
use lilscript::{analyze, interpret_program, parse_source};

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
        return Err("--cases must be greater than zero".to_string());
    }
    if args.random_seed {
        args.seed = entropy_seed();
    }
    // Announced before any work so a hang, a timeout or a crash still names the
    // seed. The divergence and success paths repeat it; only this line survives
    // a run that never reaches either.
    eprintln!(
        "lilscript-differential: seed {:#018x}, {} cases",
        args.seed, args.cases
    );

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| root.join("target/differential"));
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;
    let source_path = output_dir.join("generated.lil");
    let expected_path = output_dir.join("expected.out");
    let source = ProgramGenerator::new(args.seed).generate(args.cases);

    let arena = Bump::new();
    let program = parse_source(&arena, &source)
        .map_err(|error| format!("generated source did not parse: {error}"))?;
    let semantics = analyze(&program)
        .map_err(|error| format!("generated source did not type-check: {error}"))?;
    let expected = interpret_program(&program, &semantics)
        .map_err(|error| format!("reference evaluation failed: {error}"))?;
    fs::write(&source_path, source)
        .map_err(|error| format!("failed to write {}: {error}", source_path.display()))?;
    fs::write(&expected_path, &expected)
        .map_err(|error| format!("failed to write {}: {error}", expected_path.display()))?;

    let compiler = match args.compiler {
        Some(compiler) => compiler,
        None => std::env::current_exe()
            .map_err(|error| format!("failed to locate current executable: {error}"))?
            .parent()
            .expect("an executable has a parent directory")
            .join(executable_name("lilscript")),
    };
    if !compiler.is_file() {
        return Err(format!(
            "compiler not found at {}; build all release binaries first or pass --compiler",
            compiler.display()
        ));
    }

    // The JavaScript lanes: the production policy (the repository's
    // `lilscript.toml`, which keeps `print`), development mode (the same
    // policy without candidate search), and formation only (every tactic
    // vetoed and no search, `tests/config/no-optimization.toml`). Each names
    // its configuration, so the output directory does not decide it.
    //
    // The native lanes are masked. Every generated program uses `Record<int>`
    // (the `differentialIdentity` prelude), which the native target refuses
    // until native records land; plan M11.4 owns them and restores these lanes.
    let production = root.join("lilscript.toml");
    let formation_only = root.join("tests/config/no-optimization.toml");
    let lanes: [(&str, Vec<&std::ffi::OsStr>); 3] = [
        (
            "production JavaScript",
            vec![
                "--mode".as_ref(),
                "production".as_ref(),
                "--config".as_ref(),
                production.as_os_str(),
            ],
        ),
        (
            "development JavaScript",
            vec![
                "--mode".as_ref(),
                "development".as_ref(),
                "--config".as_ref(),
                production.as_os_str(),
            ],
        ),
        (
            "formation-only JavaScript",
            vec![
                "--mode".as_ref(),
                "production".as_ref(),
                "--config".as_ref(),
                formation_only.as_os_str(),
            ],
        ),
    ];
    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());
    for (index, (lane, flags)) in lanes.iter().enumerate() {
        let javascript = output_dir.join(format!("lane-{index}.js"));
        run_checked(
            Command::new(&compiler)
                .arg(&source_path)
                .args(["--target", "js"])
                .args(flags)
                .arg("-o")
                .arg(&javascript),
            &format!("{lane} compilation"),
        )?;
        compare_output(
            lane,
            &expected,
            run_checked(Command::new(&node).arg(&javascript), lane)?,
            args.seed,
            &source_path,
        )?;
    }

    println!(
        "{} deterministic programs matched the Rust reference evaluator across production, development and formation-only JavaScript; the native lanes are masked until native records land (plan M11.4) (seed {:#018x}).",
        args.cases, args.seed
    );
    Ok(())
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

    fn generate(&mut self, cases: usize) -> String {
        let mut source = String::from(
            "int differentialCalls=0;\n\
             bool differentialProbe(int value){differentialCalls++;return (value&1)==0;}\n\
             int differentialRotate(int value,int amount){return (value<<amount)|(value>>>(32-amount));}\n\
             int differentialMemory(int seed){ArrayBuffer storage=new ArrayBuffer(6);Uint8Array bytes=new Uint8Array(storage);bytes[0]=seed;bytes[1]=seed>>>8;bytes[2]=-1;Uint8Array alias=bytes.subarray(1,4);int old=alias[0]++;Uint8Array copied=bytes.slice(-5,4);copied[0]^=255;ArrayBuffer middle=storage.slice(1,4);Uint8Array middleBytes=new Uint8Array(middle);SharedArrayBuffer shared=new SharedArrayBuffer(2);Uint8Array sharedBytes=new Uint8Array(shared);sharedBytes[0]=copied[0]+middleBytes[1];return bytes[0]+(bytes[1]<<8)+(bytes[2]<<16)+old+alias.byteOffset+copied.length+shared.byteLength+sharedBytes[0];}\n\
             int differentialSnapshotWrite(Record<int> node,int next){int saved=node.href??0;node.href=next;return saved+(node.href??0);}\n\
             int differentialSnapshotRebind(Record<int> node,Record<int> next){int saved=node.href??0;node=next;return saved+(node.href??0);}\n\
             int differentialSnapshotComputed(Record<int> node,int next){int saved=node[\"href\"]??0;node.href=next;return saved+(node.href??0);}\n\
             int differentialSnapshotCapturedRebind(Record<int> node,int next){int saved=node.href??0;func()->void rebind=()=>{node=record{href:next,title:0};};rebind();return saved+(node.href??0);}\n\
             int differentialIdentity(int seed){Record<int> written=record{href:seed,title:seed^1};Record<int> reboundFrom=record{href:seed,title:seed^1};Record<int> reboundTo=record{href:seed^7,title:seed^3};Record<int> computed=record{href:seed,title:seed^1};Record<int> captured=record{href:seed,title:seed^1};int prev=0;int cur=seed&15;if(cur==0){cur=1;}int count=0;while(prev!=cur){prev=cur;if(cur>3){cur=cur-3;}else{cur=0;}count=count+1;}return differentialSnapshotWrite(written,seed^9)+differentialSnapshotRebind(reboundFrom,reboundTo)+differentialSnapshotComputed(computed,seed^11)+differentialSnapshotCapturedRebind(captured,seed^13)+count;}\n",
        );
        let mut calls = String::new();
        for case in 0..cases {
            self.generate_case(case, &mut source);
            let lhs = self.random.literal();
            let rhs = self.random.literal();
            writeln!(calls, "print(differentialCase{case}({lhs},{rhs}));")
                .expect("writing to String cannot fail");
        }
        source.push_str(&calls);
        let memory_seed = self.random.literal();
        writeln!(source, "print(differentialMemory({memory_seed}));")
            .expect("writing to String cannot fail");
        let identity_seed = self.random.literal();
        writeln!(source, "print(differentialIdentity({identity_seed}));")
            .expect("writing to String cannot fail");
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
        // One line per generated case, plus memory, identity, and probe-count
        // summaries appended by `generate`.
        assert_eq!(output.lines().count(), 8 + 3);
    }

    #[test]
    fn seed_parser_accepts_decimal_and_hex() {
        assert_eq!(parse_seed("42").unwrap(), 42);
        assert_eq!(parse_seed("0x2a").unwrap(), 42);
    }
}
