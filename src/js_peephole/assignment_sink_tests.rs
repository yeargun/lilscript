use super::{optimize_generated_javascript, shape_declarations};

fn assert_public_observations(source: &str, expected: &str) {
    let mut failures = Vec::new();
    for (entry, result) in [
        ("original", Ok(source.to_owned())),
        (
            "shape_declarations",
            shape_declarations(source).map(|(code, _)| code),
        ),
        (
            "optimize_generated_javascript",
            optimize_generated_javascript(source).map(|result| result.code),
        ),
    ] {
        let javascript = match result {
            Ok(javascript) => javascript,
            Err(error) => {
                failures.push(format!("{entry}: {error:?}\nsource:\n{source}"));
                continue;
            }
        };
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(&javascript)
            .output()
            .expect("node must execute generated JavaScript");
        let stdout = String::from_utf8(output.stdout).expect("node stdout must be UTF-8");
        if !output.status.success() || stdout != expected {
            failures.push(format!(
                "{entry}: status={:?}\nexpected: {expected:?}\nactual: {stdout:?}\nstderr: {}\nsource:\n{javascript}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr),
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

fn expression_observations(cases: &[(&str, &str)]) -> String {
    let mut source = String::from("const rows=[];");
    for (input, expression) in cases {
        source.push_str(&format!(
            "{{const input={input};let calls=0,a='before',t=false;\
             function produce(){{calls++;return input}}\
             a=produce(),t=({expression});rows.push([calls,a===input,t]);}}"
        ));
    }
    source.push_str("console.log(JSON.stringify(rows));");
    source
}

#[test]
fn public_entries_preserve_assigned_values_and_later_nullish_truthiness() {
    let source = r#"
function observe(input) {
    let calls=0,a='before',t=false;
    function produce(){calls++;return input}
    a=produce(),t=(a==null)||!a;
    return [calls,a===input,t,typeof a];
}
console.log(JSON.stringify([
    observe(null),observe(void 0),observe(false),observe(0),observe({value:7})
]));
"#;
    assert_public_observations(
        source,
        "[[1,true,true,\"object\"],[1,true,true,\"undefined\"],[1,true,true,\"boolean\"],[1,true,true,\"number\"],[1,true,false,\"object\"]]\n",
    );
}

#[test]
fn public_entries_preserve_assignment_before_operator_consumers() {
    let source = expression_observations(&[
        ("null", "a==null"),
        ("7", "a===7"),
        ("7", "a+3"),
        ("7", "a*3"),
        ("0", "a?11:12"),
        ("0", "a&&9"),
        ("0", "a||9"),
        ("null", "a??9"),
        ("false", "a??9"),
    ]);
    assert_public_observations(
        &source,
        "[[1,true,true],[1,true,true],[1,true,10],[1,true,21],[1,true,12],[1,true,0],[1,true,9],[1,true,9],[1,true,false]]\n",
    );
}

#[test]
fn public_entries_preserve_assignment_before_postfix_consumers() {
    let source = expression_observations(&[
        ("({value:7})", "a.value"),
        ("({value:7})", "a['value']"),
        ("(function(value){return value+4})", "a(3)"),
        ("(function(parts){return parts[0].length})", "a`x`"),
        ("({value:7})", "a?.value"),
        ("({value:7,method(){return this.value}})", "a.method()"),
    ]);
    assert_public_observations(
        &source,
        "[[1,true,7],[1,true,7],[1,true,7],[1,true,1],[1,true,7],[1,true,7]]\n",
    );
}

#[test]
fn public_entries_preserve_literal_and_call_argument_observations() {
    let source = r#"
const rows=[];
{
    let b,d;
    b=[1],d=[b];
    rows.push([b,d,d[0]===b]);
}
{
    let b,d,x=0;
    b=x+1,d={k:b};
    rows.push([b,d]);
}
{
    let b=0,d=0;
    function g(p,q){return [p,q]}
    b=[1],d=g(2,b);
    rows.push([b,d,d[1]===b]);
}
{
    let b=0,d=0,e=0;
    b=[],d=e=[b];
    rows.push([d===e,d[0]===b]);
}
{
    var b=[],d=[b];
    rows.push([Array.isArray(b),d[0]===b]);
}
console.log(JSON.stringify(rows));
"#;
    assert_public_observations(
        source,
        "[[[1],[[1]],true],[1,{\"k\":1}],[[1],[2,[1]],true],[true,true],[true,true]]\n",
    );
}

#[test]
fn public_entries_preserve_prefix_effects_and_conditional_reachability() {
    let source = r#"
const rows=[];
{
    let x=1,b=0,d=0;
    b=(x=2),d=[x,b];
    rows.push([x,b,d]);
}
{
    let b=0,d=0,events=[];
    const x={valueOf(){events.push(b);return b}};
    b=2,d=[+x,b];
    rows.push([b,d,events]);
}
{
    let b,d,x=0;
    b=[1],x&&(d=[b]);
    rows.push([b,d??null]);
}
{
    let b,d;
    function h(){b=[9];return 0}
    b=[1],d=[h(),b];
    rows.push([b,d]);
}
{
    let b=0,d=0,events=[];
    const x={get y(){events.push(b);return b}};
    b=2,d=[x.y,b];
    rows.push([b,d,events]);
}
{
    let b=0,d=0,x=0;
    b=2,d=[x++,b];
    rows.push([x,b,d]);
}
{
    let b=0,d=null,x=false;
    b=[1],x?(d=[b]):0;
    rows.push([b,d]);
}
console.log(JSON.stringify(rows));
"#;
    assert_public_observations(
        source,
        "[[2,2,[2,2]],[2,[2,2],[2]],[[1],null],[[9],[0,[9]]],[2,[2,2],[2]],[1,2,[0,2]],[[1],null]]\n",
    );
}

#[test]
fn public_entries_preserve_shorthand_and_arrow_binding_roles() {
    let source = r#"
const rows=[];
{
    let b=0,d=0;
    b=2,d={b};
    rows.push([b,d.b]);
}
{
    let b=0,d=0;
    b=2,d=(b)=>b+1;
    rows.push([b,d(4),d()]);
}
{
    let b=0,d=0;
    b=2,d=()=>b;
    rows.push([b,d()]);
}
console.log(JSON.stringify(rows));
"#;
    assert_public_observations(source, "[[2,2],[2,5,null],[2,2]]\n");
}
