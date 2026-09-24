use super::scalar_transfer::NumberFacts;
use crate::js::{Binary, Unary};
use std::process::Command;

fn range(minimum: i64, maximum: i64) -> NumberFacts {
    NumberFacts::integer_range(minimum, maximum, false).unwrap()
}

#[test]
fn number_domain_integrality_and_zero_sign_are_independent() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.5, -0.5] {
        let facts = NumberFacts::literal(value);
        assert!(facts.is_number());
        assert_eq!(facts.integer_bounds(), None);
        assert!(!facts.normalization_redundant());
    }
    let zero = NumberFacts::literal(0.0);
    let negative_zero = NumberFacts::literal(-0.0);
    assert_eq!(zero.integer_bounds(), negative_zero.integer_bounds());
    assert!(zero.normalization_redundant());
    assert!(!negative_zero.normalization_redundant());
    assert!(zero.unary(Unary::Negate).may_negative_zero());
    assert!(negative_zero.unary(Unary::Negate).normalization_redundant());
    assert!(negative_zero.to_int32().normalization_redundant());
    assert!(NumberFacts::integer_range(2, 1, false).is_none());
    assert!(NumberFacts::integer_range(1, 2, true).is_none());
    assert!(NumberFacts::integer_range(0, 9_007_199_254_740_992, false).is_none());

    // A raw host value may be a string, object or BigInt. Numeric output and
    // optional normalization do not follow from a source declaration.
    assert_eq!(
        NumberFacts::UNKNOWN.binary(Binary::Add, zero),
        NumberFacts::UNKNOWN
    );
    assert_eq!(
        NumberFacts::UNKNOWN.binary(Binary::Multiply, zero),
        NumberFacts::NUMBER
    );
    assert!(!NumberFacts::UNKNOWN
        .binary(Binary::Multiply, zero)
        .normalization_redundant());
    assert_eq!(
        NumberFacts::UNKNOWN.binary(Binary::BitAnd, NumberFacts::UNKNOWN),
        NumberFacts::UNKNOWN
    );
    assert_eq!(
        NumberFacts::UNKNOWN
            .binary(Binary::UnsignedShiftRight, NumberFacts::UNKNOWN)
            .integer_bounds(),
        Some((0, u32::MAX as i64))
    );
    assert_eq!(
        NumberFacts::UNKNOWN.unary(Unary::BitNot),
        NumberFacts::UNKNOWN
    );
    assert_eq!(NumberFacts::UNKNOWN.to_int32(), NumberFacts::I32);
    assert_eq!(range(1, 3).join(range(5, 8)).integer_bounds(), Some((1, 8)));
    assert_eq!(range(1, 3).join(NumberFacts::UNKNOWN), NumberFacts::UNKNOWN);
}

#[test]
fn raw_arithmetic_is_distinct_from_signed_language_normalization() {
    let overflow =
        NumberFacts::literal(i32::MAX as f64).binary(Binary::Add, NumberFacts::literal(1.0));
    assert_eq!(
        overflow.integer_bounds(),
        Some((2_147_483_648, 2_147_483_648))
    );
    assert!(!overflow.normalization_redundant());
    assert_eq!(
        overflow.to_int32().integer_bounds(),
        Some((i32::MIN as i64, i32::MIN as i64))
    );
    let large_product = NumberFacts::literal(i32::MAX as f64)
        .binary(Binary::Multiply, NumberFacts::literal(i32::MAX as f64));
    assert_eq!(large_product.integer_bounds(), None);
    assert!(!large_product.normalization_redundant());
    assert_eq!(large_product.to_int32(), NumberFacts::I32);
    assert!(!NumberFacts::literal(3.0)
        .binary(Binary::Divide, NumberFacts::literal(2.0))
        .normalization_redundant());
    for op in [Binary::Divide, Binary::Remainder] {
        assert!(!range(-10, 10)
            .binary(op, range(0, 1))
            .normalization_redundant());
    }
    assert!(!NumberFacts::literal(-4.0)
        .binary(Binary::Remainder, NumberFacts::literal(2.0))
        .normalization_redundant());
    assert!(!NumberFacts::literal(0.0)
        .binary(Binary::Multiply, NumberFacts::literal(-1.0))
        .normalization_redundant());
    assert!(!NumberFacts::literal(-1.0)
        .binary(Binary::UnsignedShiftRight, NumberFacts::literal(0.0))
        .normalization_redundant());
    assert!(NumberFacts::literal(-1.0)
        .binary(Binary::UnsignedShiftRight, NumberFacts::literal(1.0))
        .normalization_redundant());
}

#[test]
fn masks_shifts_and_small_intervals_supply_useful_raw_proofs() {
    let byte = NumberFacts::UNKNOWN.binary(Binary::BitAnd, NumberFacts::literal(255.0));
    assert_eq!(byte.integer_bounds(), Some((0, 255)));
    assert!(byte
        .binary(Binary::Add, range(1, 5))
        .normalization_redundant());
    assert_eq!(
        range(-100, 100)
            .binary(Binary::ShiftRight, NumberFacts::literal(2.0))
            .integer_bounds(),
        Some((-25, 25))
    );
    assert_eq!(
        range(0, 10)
            .binary(Binary::ShiftLeft, NumberFacts::literal(2.0))
            .integer_bounds(),
        Some((0, 40))
    );
    assert_eq!(
        range(0, 200)
            .binary(Binary::Remainder, range(2, 10))
            .integer_bounds(),
        Some((0, 9))
    );
    assert!(range(1, 100)
        .binary(Binary::Multiply, range(-10, -1))
        .normalization_redundant());
    assert!(!range(0, 100)
        .binary(Binary::Multiply, range(-10, -1))
        .normalization_redundant());
    assert!(range(-100, -1)
        .unary(Unary::Negate)
        .normalization_redundant());
    assert!(!range(-100, 0)
        .unary(Unary::Negate)
        .normalization_redundant());
}

fn raw(op: Binary, left: f64, right: f64) -> f64 {
    let a = left as i32;
    let b = right as i32;
    match op {
        Binary::Add => left + right,
        Binary::Subtract => left - right,
        Binary::Multiply => left * right,
        Binary::Divide => left / right,
        Binary::Remainder => left % right,
        Binary::BitAnd => (a & b) as f64,
        Binary::BitOr => (a | b) as f64,
        Binary::BitXor => (a ^ b) as f64,
        Binary::ShiftLeft => a.wrapping_shl(b as u32 & 31) as f64,
        Binary::ShiftRight => (a >> (b as u32 & 31)) as f64,
        Binary::UnsignedShiftRight => ((a as u32) >> (b as u32 & 31)) as f64,
        _ => panic!("test arithmetic operation"),
    }
}

fn assert_covers(facts: NumberFacts, value: f64) {
    assert!(facts.is_number());
    if let Some((minimum, maximum)) = facts.integer_bounds() {
        assert!(
            value.is_finite() && value.fract() == 0.0,
            "{facts:?} excludes {value}"
        );
        assert!(
            value >= minimum as f64 && value <= maximum as f64,
            "{facts:?} excludes {value}"
        );
    }
    assert!(
        facts.may_negative_zero() || value.to_bits() != (-0.0f64).to_bits(),
        "{facts:?} excludes -0"
    );
    if facts.normalization_redundant() {
        assert_eq!(value.to_bits(), ((value as i32) as f64).to_bits());
    }
}

#[test]
fn all_small_intervals_cover_each_member_and_both_zero_signs() {
    let mut inputs = Vec::new();
    for minimum in -3..=3 {
        for maximum in minimum..=3 {
            for negative_zero in [false, true] {
                let Some(facts) = NumberFacts::integer_range(minimum, maximum, negative_zero)
                else {
                    continue;
                };
                let mut members: Vec<f64> = (minimum..=maximum).map(|value| value as f64).collect();
                if negative_zero {
                    members.push(-0.0);
                }
                inputs.push((facts, members));
            }
        }
    }
    for (left, left_members) in &inputs {
        for member in left_members {
            assert_covers(left.unary(Unary::Negate), -*member);
        }
        for (right, right_members) in &inputs {
            for op in [
                Binary::Add,
                Binary::Subtract,
                Binary::Multiply,
                Binary::Divide,
                Binary::Remainder,
                Binary::BitAnd,
                Binary::BitOr,
                Binary::BitXor,
                Binary::ShiftLeft,
                Binary::ShiftRight,
                Binary::UnsignedShiftRight,
            ] {
                let facts = left.binary(op, *right);
                for a in left_members {
                    for b in right_members {
                        assert_covers(facts, raw(op, *a, *b));
                    }
                }
            }
        }
    }
}

#[test]
fn javascript_oracle_checks_boundaries_and_host_values_before_normalization() {
    let pairs = [
        (i32::MIN as f64, -1.0),
        (i32::MAX as f64, 1.0),
        (i32::MAX as f64, i32::MAX as f64),
        (65_535.0, 65_535.0),
        (9_007_199_254_740_991.0, 2.0),
        (3.0, 2.0),
        (-4.0, 2.0),
        (0.0, -1.0),
        (-0.0, -1.0),
        (-0.0, 0.0),
        (-0.0, -0.0),
        (1.0, 0.0),
        (-1.0, 0.0),
        (-1.0, 1.0),
    ];
    let mut cases = Vec::new();
    for (left, right) in pairs {
        for (op, spelling) in [
            (Binary::Add, "+"),
            (Binary::Subtract, "-"),
            (Binary::Multiply, "*"),
            (Binary::Divide, "/"),
            (Binary::Remainder, "%"),
            (Binary::UnsignedShiftRight, ">>>"),
        ] {
            let facts = NumberFacts::literal(left).binary(op, NumberFacts::literal(right));
            cases.push(serde_json::json!([
                left,
                right,
                spelling,
                facts.integer_bounds(),
                facts.may_negative_zero(),
                facts.normalization_redundant()
            ]));
        }
    }
    let script = format!(
        r#"
const cases = {};
for (const [a,b,op,bounds,negativeZero,redundant] of cases) {{
  const raw = new Function('a','b','return a '+op+' b')(a,b);
  if (bounds && (!Number.isSafeInteger(raw) || raw<bounds[0] || raw>bounds[1])) throw Error('bounds '+[a,b,op,raw]);
  if (!negativeZero && Object.is(raw,-0)) throw Error('negative zero');
  if (redundant && !Object.is(raw,raw|0)) throw Error('normalization '+[a,b,op,raw]);
}}
// These values can come from a replaced typed host method. Their *raw* facts
// must stay unknown, and the real normalization must still coerce or throw.
let coercions=0;
for (const [value,expected] of [[NaN,0],[4294967297,1],[{{valueOf(){{coercions++;return 7}}}},7]]) {{
  if ((value|0)!==expected) throw Error('host result');
}}
try {{ ({{valueOf(){{throw Error('coercion')}}}})|0; throw Error('missing throw'); }}
catch(error) {{ if(error.message!=='coercion') throw error; }}
if(coercions!==1) throw Error('evaluation count');
// The raw mask itself performs coercion. Removing only its redundant outer
// normalization keeps exactly that operation, including mixed-BigInt errors.
for(const value of ['257', {{valueOf(){{coercions++;return 258}}}}]) {{
  const masked=value&255;
  if(!Object.is(masked+1,(masked+1)|0)) throw Error('mask range');
}}
if(coercions!==2) throw Error('mask coercion count');
for(const value of [1n,{{valueOf(){{throw Error('mask coercion')}}}}]) {{
  let caught=false;
  try {{ value&255; }} catch(error) {{caught=true}}
  if(!caught) throw Error('missing mask throw');
}}
console.log('ok');
"#,
        serde_json::to_string(&cases).unwrap()
    );
    let output = Command::new("node")
        .args(["-e", &script])
        .output()
        .expect("Node is required for the JS arithmetic oracle");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ok\n");
}
