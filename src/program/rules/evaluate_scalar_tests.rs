use super::*;
use std::process::Command;

fn checked(inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "print(0);").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program =
        super::super::super::from_source::from_checked_source(&syntax, &semantics).unwrap();
    inspect(&program);
}

fn node(script: &str) -> String {
    let output = Command::new("node").args(["-e", script]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn primitive_predicates_match_node_on_type_zero_nan_and_utf16_edges() {
    use crate::check::BuiltinCall as B;
    let values = vec![
        (Exact::Integer(0), "0"),
        (Exact::Number((-0.0f64).to_bits()), "-0"),
        (Exact::Integer(1), "1"),
        (Exact::Number(f64::NAN.to_bits()), "NaN"),
        (Exact::Number(f64::INFINITY.to_bits()), "Infinity"),
        (Exact::Boolean(false), "false"),
        (Exact::Boolean(true), "true"),
        (Exact::Null, "null"),
        (Exact::Undefined, "void 0"),
        (computed(vec![]), "''"),
        (computed(vec![0xd800]), "'\\ud800'"),
        (computed(vec![0xd83d, 0xde00]), "'😀'"),
    ];
    checked(|program| {
        let mut rows = Vec::new();
        let mut work = Work::bounded(1 << 20, 1 << 20);
        for (left, a) in &values {
            for (right, b) in &values {
                for (operation, token) in [(B::JsStrictEqual, "==="), (B::JsStrictNotEqual, "!==")]
                {
                    let result = builtin(
                        program,
                        operation,
                        &[left.clone(), right.clone()],
                        &mut work,
                    );
                    let Some(Exact::Boolean(result)) = result else {
                        panic!("{operation:?}")
                    };
                    rows.push(serde_json::json!([format!("({a}){token}({b})"), result]));
                }
            }
            for (operation, expression) in [
                (B::JsIsNullish, format!("({a})==null")),
                (B::JsIsFalse, format!("({a})===false")),
                (B::JsIsUndefined, format!("({a})===void 0")),
            ] {
                let Some(Exact::Boolean(result)) =
                    builtin(program, operation, &[left.clone()], &mut work)
                else {
                    panic!("{operation:?}")
                };
                rows.push(serde_json::json!([expression, result]));
            }
            let result = builtin(program, B::JsTypeOf, &[left.clone()], &mut work).unwrap();
            rows.push(serde_json::json!([
                format!("typeof({a})"),
                string(program, &result).unwrap().as_unicode().unwrap()
            ]));
        }
        assert_eq!(node(&format!("for(const [source,expected] of {}){{if(eval(source)!==expected)throw new Error(source);}}console.log('ok');",serde_json::to_string(&rows).unwrap())), "ok\n");
        assert!(builtin(program, B::JsStrictEqual, &[], &mut work).is_none());
        assert!(builtin(program, B::JsTypeOf, &[], &mut work).is_none());
    });
}

#[test]
fn constant_scalar_to_int_matches_number_bit_patterns() {
    let mut numbers = vec![
        0.0,
        -0.0,
        0.9,
        -0.9,
        1.9,
        -1.9,
        2_147_483_647.0,
        2_147_483_648.0,
        -2_147_483_649.0,
        4_294_967_295.9,
        4_294_967_296.0,
        -4_294_967_295.9,
        9_007_199_254_740_991.0,
        -9_007_199_254_740_991.0,
        f64::MAX,
        f64::MIN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::from_bits(1),
        f64::from_bits((1 << 63) | 1),
    ];
    let mut bits = 0x9e3779b97f4a7c15u64;
    for _ in 0..512 {
        bits ^= bits << 13;
        bits ^= bits >> 7;
        bits ^= bits << 17;
        numbers.push(f64::from_bits(bits));
    }
    let bits: Vec<String> = numbers
        .iter()
        .map(|value| format!("{:016x}", value.to_bits()))
        .collect();
    let expected: Vec<i32> = serde_json::from_str(&node(&format!(
        "const view=new DataView(new ArrayBuffer(8));console.log(JSON.stringify({}.map(bits=>{{view.setBigUint64(0,BigInt('0x'+bits));return view.getFloat64(0)|0}})));",
        serde_json::to_string(&bits).unwrap(),
    ))).unwrap();
    checked(|program| {
        for (value, expected) in numbers.into_iter().zip(expected) {
            let actual = intrinsic(
                program,
                ResolvedIntrinsic::Method(Intrinsic::FloatToInt),
                Some(&Exact::Number(value.to_bits())),
                &[],
                &mut Work::bounded(4096, 4096),
            )
            .unwrap();
            assert_eq!(integer(&actual), Some(expected), "{:016x}", value.to_bits());
        }
    });
}

#[test]
fn constant_scalar_code_point_count_matches_utf16_iteration() {
    let mut strings = vec![
        vec![],
        vec![0],
        vec![0xd800],
        vec![0xdc00],
        vec![0xd800, 0xdc00],
        vec![0xdc00, 0xd800],
        vec![0xd800, 0xd800, 0xdc00, 0xdc00],
        vec![0x41, 0xd83d, 0xde00, 0xfe0f, 0x200d, 0xd83d, 0xde00],
    ];
    let alphabet = [
        0, 0x61, 0x300, 0xd7ff, 0xd800, 0xdbff, 0xdc00, 0xdfff, 0xe000, 0xffff,
    ];
    for &first in &alphabet {
        for &second in &alphabet {
            strings.push(vec![first, second, 0xd800, 0xdc00]);
        }
    }
    let expected: Vec<i32> = serde_json::from_str(&node(&format!(
        "console.log(JSON.stringify({}.map(units=>[...String.fromCharCode(...units)].length|0)));",
        serde_json::to_string(&strings).unwrap(),
    )))
    .unwrap();
    checked(|program| {
        for (units, expected) in strings.into_iter().zip(expected) {
            let receiver = computed(units.clone());
            let actual = intrinsic(
                program,
                ResolvedIntrinsic::Method(Intrinsic::StringCodePointLength),
                Some(&receiver),
                &[],
                &mut Work::bounded(4096, 4096),
            )
            .unwrap();
            assert_eq!(integer(&actual), Some(expected), "{units:?}");
        }
    });
}

#[test]
fn constant_scalar_evaluation_obeys_arity_and_storage_limits() {
    checked(|program| {
        let operation = ResolvedIntrinsic::Method(Intrinsic::StringCodePointLength);
        let receiver = computed(vec![0xd800, 0xdc00]);
        for (work, memory) in [(1, 4096), (4096, 1)] {
            assert!(intrinsic(
                program,
                operation,
                Some(&receiver),
                &[],
                &mut Work::bounded(work, memory)
            )
            .is_none());
        }
        assert!(intrinsic(
            program,
            operation,
            Some(&receiver),
            &[Exact::Integer(1)],
            &mut Work::bounded(4096, 4096)
        )
        .is_none());
        assert!(intrinsic(
            program,
            ResolvedIntrinsic::Method(Intrinsic::FloatToInt),
            Some(&Exact::Number(1.0f64.to_bits())),
            &[Exact::Integer(1)],
            &mut Work::bounded(4096, 4096)
        )
        .is_none());
    });
}
