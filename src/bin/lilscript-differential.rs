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
#[command(about = "Generate deterministic programs and compare every LilScript backend.")]
struct Args {
    /// Number of generated functions and result rows.
    #[arg(long, default_value_t = 64)]
    cases: usize,

    /// Reproduction seed, accepted in decimal or with a 0x prefix.
    #[arg(long, default_value = "0x6c696c7363726970", value_parser = parse_seed)]
    seed: u64,

    /// Compiler executable. Defaults to the lilscript binary beside this executable.
    #[arg(long)]
    compiler: Option<PathBuf>,

    /// Directory for generated sources and backend artifacts.
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
    let args = Args::parse();
    if args.cases == 0 {
        return Err("--cases must be greater than zero".to_string());
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| root.join("target/differential"));
    fs::create_dir_all(&output_dir)
        .map_err(|error| format!("failed to create {}: {error}", output_dir.display()))?;
    let source_path = output_dir.join("generated.lil");
    let expected_path = output_dir.join("expected.out");
    let optimized_base = output_dir.join("optimized");
    let no_optimization_js = output_dir.join("no-optimization.js");
    let emitted_c_native = output_dir.join("emitted-c-native");
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

    run_checked(
        Command::new(&compiler)
            .arg(&source_path)
            .args(["--target", "all", "--mode", "production", "-o"])
            .arg(&optimized_base),
        "optimized compilation",
    )?;
    run_checked(
        Command::new(&compiler)
            .arg(&source_path)
            .args(["--target", "js", "--mode", "production", "--config"])
            .arg(root.join("tests/config/no-optimization.toml"))
            .arg("-o")
            .arg(&no_optimization_js),
        "optimizer-disabled JavaScript compilation",
    )?;

    let node = std::env::var_os("NODE").unwrap_or_else(|| "node".into());
    let optimized_js = optimized_base.with_extension("js");
    compare_output(
        "optimized JavaScript",
        &expected,
        run_checked(
            Command::new(&node).arg(&optimized_js),
            "optimized JavaScript",
        )?,
        args.seed,
        &source_path,
    )?;
    compare_output(
        "optimizer-disabled JavaScript",
        &expected,
        run_checked(
            Command::new(&node).arg(&no_optimization_js),
            "optimizer-disabled JavaScript",
        )?,
        args.seed,
        &source_path,
    )?;
    compare_output(
        "native executable",
        &expected,
        run_checked(&mut Command::new(&optimized_base), "native executable")?,
        args.seed,
        &source_path,
    )?;

    let cc = std::env::var_os("CC").unwrap_or_else(|| "clang".into());
    let mut cc_command = Command::new(&cc);
    cc_command
        .args(["-std=c11", "-O3"])
        .arg(optimized_base.with_extension("c"))
        .arg("-o")
        .arg(&emitted_c_native);
    #[cfg(not(target_os = "windows"))]
    cc_command.arg("-lm");
    run_checked(&mut cc_command, "independent emitted C compilation")?;
    compare_output(
        "independently compiled C",
        &expected,
        run_checked(
            &mut Command::new(&emitted_c_native),
            "independently compiled C",
        )?,
        args.seed,
        &source_path,
    )?;

    println!(
        "{} deterministic programs matched the Rust reference evaluator across optimized JS, optimizer-disabled JS, emitted C, and native execution (seed {:#018x}).",
        args.cases, args.seed
    );
    Ok(())
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
             int differentialRotate(int value,int amount){return (value<<amount)|(value>>>(32-amount));}\n",
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
        writeln!(source, "int old=b++;b+=old;if(gate){{b--;}}else{{b++;}}")
            .expect("writing to String cannot fail");
        writeln!(source, "{{int a={shadow};b+=a;}}return b;}}")
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
        assert_eq!(output.lines().count(), 9);
    }

    #[test]
    fn seed_parser_accepts_decimal_and_hex() {
        assert_eq!(parse_seed("42").unwrap(), 42);
        assert_eq!(parse_seed("0x2a").unwrap(), 42);
    }
}
