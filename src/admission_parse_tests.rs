//! The canonical form: the spellings a printer may choose read as one
//! structure, and a different grouping or nesting never does.
use super::*;

fn canonical(text: &str, module: bool) -> Vec<Canon> {
    parse_canonical(text, module)
        .unwrap_or_else(|refusal| panic!("{text}: {}", refusal.0))
        .0
}

fn same(left: &str, right: &str) {
    assert_eq!(
        canonical(left, false),
        canonical(right, false),
        "`{left}` and `{right}` should be one structure"
    );
}

fn different(left: &str, right: &str) {
    assert_ne!(
        digest(&canonical(left, false)),
        digest(&canonical(right, false)),
        "`{left}` and `{right}` should differ"
    );
}

#[test]
fn printer_spellings_read_as_one_structure() {
    // Literals, including the printer's boolean, undefined and signed spellings.
    same("f(!0,!1,void 0,-5,1e3)", "f(true,false,0,5,1000)");
    // Integer normalization is printed or omitted by facts.
    same("x=a+b|0", "x=a+b");
    same("x=(a|0)*2", "x=a*2");
    // Call unbinding and name wrappers.
    same("(0,o.f)(1)", "o.f(1)");
    same("x=(0,function(){})", "x=function(){}");
    same("x={f:function(){}}.f", "x=function(){}");
    // Compound assignment and increments.
    same("x+=y", "x=x+y");
    same("++x", "x=x+1");
    same("o.p*=2", "o.p=o.p*2");
    // Statement spellings of `if`.
    same("a&&b()", "if(a)b()");
    same("a||b()", "if(!a)b()");
    same("x=c?1:2", "if(c)x=1;else x=2");
    same("void f()", "f()");
    same(
        "function g(){return c?a:b}",
        "function g(){if(c)return a;return b}",
    );
    same(
        "function g(){return c?a:b}",
        "function g(){if(c)return a;else return b}",
    );
    same(
        "function g(){if(c)return a;return d?e:f}",
        "function g(){return c?a:d?e:f}",
    );
    // Declarations, loop heads, braces and arrow bodies.
    same("let a=1,b", "let a=1;let b");
    same(
        "for(let i=0;i<n;i++)f(i)",
        "{let i=0;for(;i<n;i=i+1){f(i)}}",
    );
    same("while(c)f()", "for(;c;)f()");
    same("if(c){f()}else{g()}", "if(c)f();else g()");
    same("x=a=>a", "x=a=>{return a}");
    same("x=(a,b=null)=>a", "x=(a,b=0)=>{return a}");
    // Keys: identifier, quoted, numeric and computed literal keys.
    same("x=o.k+o[5]", "x=o[\"k\"]+o[\"5\"]");
    same("x={k:1,\"s\":2,3:3,[\"c\"]:4,v}", "x={k:1,s:2,3:3,c:4,v:v}");
    // Grouping parentheses and empty statements.
    same("(f());;", "f()");
}

#[test]
fn grouping_nesting_and_statement_boundaries_are_structure() {
    different("x=a+b*c", "x=(a+b)*c");
    different("x=a-(-b)", "x=a-b");
    different("if(c){f();g()}", "if(c)f();g()");
    different("if(a)if(b)f();else g()", "if(a){if(b)f()}else g()");
    different("x=new f()()", "x=new(f())()");
    different("x=(a,b)", "x=b");
    different("x=a?b:c?d:e", "x=(a?b:c)?d:e");
    different("f(a,b)", "f(a)");
    different("x=[a,...b]", "x=[a,b]");
    different("try{f()}catch{}", "try{f()}catch(e){}");
    different("for(let k in o)f()", "for(let k of o)f()");
    different("function g(){\"use strict\";f()}", "function g(){f()}");
    different("x=typeof a", "x=a");
    different("x=async()=>1", "x=()=>1");
}

#[test]
fn a_text_that_does_not_parse_is_refused_with_where() {
    let refusal = parse_canonical("x=a--b;f()", false).unwrap_err();
    assert!(refusal.0.contains("does not parse"), "{}", refusal.0);
    assert!(refusal.0.contains("at byte"), "{}", refusal.0);
    // A module-only form in a script.
    let refusal = parse_canonical("let a;export{a}", false).unwrap_err();
    assert!(refusal.0.contains("imports or exports"), "{}", refusal.0);
    assert!(parse_canonical("let a;export{a}", true).is_ok());
    // An invalid regular expression literal is an early error.
    assert!(parse_canonical("x=/(/", false).is_err());
}

#[test]
fn a_structure_mismatch_names_its_statements() {
    let expected = digest(&canonical("f();if(c){g();h()}k()", false));
    let refusal = admit(&expected, "f();if(c)g();h();k()", false).unwrap_err();
    assert!(refusal.0.contains("top-level statements"), "{}", refusal.0);
    let refusal = admit(&expected, "f();if(c){g(),h()}k()", false).unwrap_err();
    assert!(refusal.0.contains("different structure"), "{}", refusal.0);
    assert!(refusal.0.contains("if(c){g(),h()}"), "{}", refusal.0);
    assert!(admit(&expected, "f();c&&(g(),0);if(c){g();h()}k()", false).is_err());
    assert!(admit(&expected, "f();if(c){g();h()}k()", false).is_ok());
}
