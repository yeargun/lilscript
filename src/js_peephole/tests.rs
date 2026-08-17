use super::parse::{non_overlapping_parsed_node_count, parse_expression_regions};
use super::token::{lex, punctuation_width};
use super::{
    analyze_generated_javascript, function_leading_declaration_variant,
    optimize_generated_javascript, reorder_uninitialized_var_declarators, PeepholeResult,
};

const LEGACY_PUNCTUATION: [&str; 31] = [
    ">>>=", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "++", "--", "**",
    "<<", ">>", "<=", ">=", "==", "!=", "&&", "||", "??", "+=", "-=", "*=", "/=", "%=", "&=", "|=",
    "^=",
];

fn legacy_punctuation_width(source: &str) -> usize {
    LEGACY_PUNCTUATION
        .iter()
        .find(|punctuation| source.starts_with(**punctuation))
        .map_or_else(
            || source.chars().next().map_or(1, char::len_utf8),
            |punctuation| punctuation.len(),
        )
}

fn assert_punctuation_width_matches_legacy(source: &str) {
    assert_eq!(
        punctuation_width(source),
        legacy_punctuation_width(source),
        "punctuation dispatch differed for {source:?}"
    );
}

fn assert_operator_alphabet_equivalence(source: &mut String, remaining: usize) {
    if !source.is_empty() {
        assert_punctuation_width_matches_legacy(source);
    }
    if remaining == 0 {
        return;
    }
    for byte in b"=!*<>&|?+-/%^" {
        source.push(char::from(*byte));
        assert_operator_alphabet_equivalence(source, remaining - 1);
        source.pop();
    }
}

fn optimize_emitted_without_regex_literals(source: &str) -> PeepholeResult {
    optimize_generated_javascript(source).unwrap()
}

fn run_javascript(source: &str) -> String {
    let output = std::process::Command::new("node")
        .arg("-e")
        .arg(source)
        .output()
        .expect("node must execute generated JavaScript");
    assert!(
        output.status.success(),
        "node failed:\n{}\nsource:\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("node stdout must be UTF-8")
}

#[test]
fn punctuation_dispatch_recognizes_every_legacy_operator() {
    for punctuation in LEGACY_PUNCTUATION {
        assert_eq!(
            punctuation_width(punctuation),
            punctuation.len(),
            "failed to consume all of {punctuation:?}"
        );
        assert_punctuation_width_matches_legacy(punctuation);
        assert_punctuation_width_matches_legacy(&format!("{punctuation}suffix"));
    }
}

#[test]
fn punctuation_dispatch_matches_every_strict_prefix_and_ambiguous_munch() {
    assert_punctuation_width_matches_legacy("");
    for punctuation in LEGACY_PUNCTUATION {
        for prefix_width in 0..punctuation.len() {
            assert_punctuation_width_matches_legacy(&punctuation[..prefix_width]);
        }
    }

    let mut source = String::with_capacity(4);
    assert_operator_alphabet_equivalence(&mut source, 4);

    for (source, expected) in [
        (">>>=", 4),
        (">>>==", 4),
        (">>>>", 3),
        (">>=", 3),
        (">>==", 3),
        (">>", 2),
        ("====", 3),
        ("!===", 3),
        ("**==", 3),
        ("&&==", 3),
        ("||==", 3),
        ("??==", 3),
    ] {
        assert_eq!(punctuation_width(source), expected, "{source:?}");
        assert_punctuation_width_matches_legacy(source);
    }
}

#[test]
fn punctuation_dispatch_preserves_single_character_and_unicode_fallbacks() {
    for byte in 0_u8..=127 {
        let source = char::from(byte).to_string();
        assert_punctuation_width_matches_legacy(&source);
    }

    for source in [
        "a",
        ".25",
        ";tail",
        "~value",
        "@decorator",
        "é",
        "×next",
        "中=value",
        "🦀>>>=",
        "\u{80}",
        "…",
        "é===",
        ">é",
        "=🦀",
    ] {
        assert_punctuation_width_matches_legacy(source);
    }
    for (source, expected) in [("é", 2), ("中=value", 3), ("🦀>>>=", 4)] {
        assert_eq!(punctuation_width(source), expected, "{source:?}");
    }
}

#[test]
fn variants_only_simple_function_leading_generated_declarations() {
    let source = "var a='function(){var no=1}';let b=function*(){var c=1;yield c};function d(e,f){var g=-2,h;return g+h}";
    assert_eq!(
        function_leading_declaration_variant(source).as_deref(),
        Some("var a='function(){var no=1}';let b=function*(){let c=1;yield c};function d(e,f){let g=-2,h;return g+h}")
    );

    for rejected in [
        "function defaults(a=1){var b=1;return b}",
        "function parameter(a){var a=1;return a}",
        "function self(){var a=a||1;return a}",
        "function call(){var a=read();return a}",
        "function redeclared(){var a=1;var a=2;return a}",
        "function hoisted(){var a=1;function a(){}return a}",
        "function nested(){if(x){var a=1}return a}",
        "function header(){for(var a=0;a<1;a++)use(a)}",
        "function text(){return `var a=${value}`}",
    ] {
        assert_eq!(
            function_leading_declaration_variant(rejected),
            None,
            "{rejected}"
        );
    }
}

#[test]
fn rewrites_only_complete_parsed_assignment_statements() {
    let optimized = optimize_generated_javascript(
        "function f(a,b){a=a+b;let c=a+b;if(c)a=a*2;for(;a<9;a=a+1)b=b^a;return a}",
    )
    .unwrap();

    assert_eq!(
        optimized.code,
        "function f(a,b){a+=b;let c=a+b;if(c)a*=2;for(;a<9;a++)b^=a;return a}"
    );
    assert_eq!(optimized.rewrites, 5);
}

#[test]
fn preserves_non_identifier_assignments_and_different_operands() {
    let source = "a.x=a.x+1;a=b+1;return a";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(optimized.code, source);
    assert_eq!(optimized.rewrites, 0);
}

#[test]
fn negated_equality_folds_loose_and_strict_without_reordering_operands() {
    let optimized = optimize_emitted_without_regex_literals(
        "let a=!(left()==right()),b=!(left()===right()),c=!(left()!=right()),d=!(left()!==right());use(a,b,c,d)",
    );

    assert_eq!(
        optimized.code,
        "let a=left()!=right(),b=left()!==right(),c=left()==right(),d=left()===right();use(a,b,c,d)"
    );
    assert_eq!(optimized.rewrites, 4);
}

#[test]
fn negated_equality_groups_under_tighter_parents_and_refuses_non_roots() {
    let optimized = optimize_emitted_without_regex_literals(
        "let a=1+!(left()==right()),b=typeof !(left()===right()),c=!(left()==right()&&guard()),d=!(x==/a==b/.test(s));use(a,b,c,d)",
    );

    assert_eq!(
        optimized.code,
        "let a=1+(left()!=right()),b=typeof(left()!==right()),c=!(left()==right()&&guard()),d=x!=/a==b/.test(s);use(a,b,c,d)"
    );
    assert_eq!(optimized.rewrites, 4);
}

#[test]
fn negated_equality_refuses_postfix_yield_and_ambiguous_expression_starts() {
    let source = "!(x==y).z;!(x==y)(z);!(x==y)[z];!(x==y)?.z;!(x==y)`tag`;!(x==y)++;!(x==y)--;!(x==y)**z;new!(x==y);function* g(){return!(yield x==y)};!({}==x);!(function(){}==x);!(class{}==x);!(async function(){}==x)";
    let optimized = optimize_emitted_without_regex_literals(source);

    assert_eq!(optimized.code, source);
    assert_eq!(optimized.rewrites, 0);
}

#[test]
fn negated_equality_requires_proof_that_regex_literals_are_absent() {
    let source = "let a=/!(x==y)/,b=/prefix!(x===y)suffix/;use(a,b)";
    let optimized = optimize_generated_javascript(source).unwrap();

    assert_eq!(optimized.code, "use(/!(x==y)/,/prefix!(x===y)suffix/)");
}

#[test]
fn canonicalizes_exact_typeof_identifiers_and_ascii_string_members() {
    let optimized = optimize_emitted_without_regex_literals(
        "let a=typeof(value)==\"number\",b=object[\"scrollTop\"],c=this['default'],d=typeof(value)instanceof Object;use(a,b,c,d)",
    );

    assert_eq!(
        optimized.code,
        "let a=typeof value==\"number\",b=object.scrollTop,c=this.default,d=typeof value instanceof Object;use(a,b,c,d)"
    );
    assert_eq!(optimized.rewrites, 4);
}

#[test]
fn reorders_only_uninitialized_var_declarators_before_initializers() {
    let source = "function run(){var log=[],first=(log.push('first'),1),empty,second=(log.push('second'),2),later;var stable,tail=3;return [empty,first,later,second,stable,tail,log.join(',')]}console.log(JSON.stringify(run()))";
    let optimized = optimize_emitted_without_regex_literals(source);

    assert_eq!(
        optimized.code,
        "function run(){var empty,later,stable,log=[],first=(log.push('first'),1),second=(log.push('second'),2),tail=3;return[empty,first,later,second,stable,tail,log.join(',')]}console.log(JSON.stringify(run()))"
    );
    assert_eq!(run_javascript(&optimized.code), run_javascript(source));
}

#[test]
fn var_declarator_reordering_preserves_hoisting_and_for_initializer_order() {
    let source = "let log=[];function run(){var observed=(log.push('first'),typeof later),later;for(var start=(log.push('second'),0),unused,index=(log.push('third'),start);index<1;index++){log.push(observed)}return [later,unused,log.join(',')]}console.log(JSON.stringify(run()))";
    let optimized = optimize_emitted_without_regex_literals(source);

    assert!(
        optimized
            .code
            .contains("var later,observed=(log.push('first'),typeof later)"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains(
            "for(var unused,start=(log.push('second'),0),index=(log.push('third'),start);"
        ),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code), run_javascript(source));
}

#[test]
fn var_declarator_reordering_refuses_tdz_destructuring_and_ambiguous_boundaries() {
    let source = "var top=read(),laterTop;var object={function:0};object.function;if(true){var initializedTop=read(),laterInTopBlock}var methodObject={function(){}};methodObject.function()\n{var initializedCallBlock=read(),laterCallBlock}function run(values){let initialized=read(),later;const first=read(),second=2;var {item}=source,plain;for(var key in source)use(key);for(var value of values)use(value);var kept=read(),/*keep*/empty;use(kept,empty,item,plain,initialized,later,first,second);var asi=read(),laterAsi\nuse(asi,laterAsi);var a=1,empty,b=2\nx,y;return y}use(top,laterTop,initializedTop,laterInTopBlock,initializedCallBlock,laterCallBlock)";
    let (optimized, rewrites) = reorder_uninitialized_var_declarators(source).unwrap();

    assert_eq!(optimized, source);
    assert_eq!(rewrites, 0);

    let unicode_asi = "function unicode(){var a=1,empty,b=2\u{2028}x,y;return y}";
    assert_eq!(
        reorder_uninitialized_var_declarators(unicode_asi)
            .unwrap()
            .0,
        unicode_asi
    );

    let reorderable = "function plain(){var initialized=read(),empty;return [initialized,empty]}";
    assert_eq!(
        optimize_generated_javascript(reorderable).unwrap().code,
        "function plain(){var empty;return[read(),empty]}"
    );
}

#[test]
fn canonical_member_syntax_separates_adjacent_keywords_and_preserves_comments() {
    let source = "let a=object[\"item\"]instanceof Object,b=object[\"key\"]in container;for(object[\"slot\"]of values){}let c=object[\"key\"]/*keep*/in container";
    let optimized = optimize_emitted_without_regex_literals(source);

    assert_eq!(
        optimized.code,
        "let a=object.item instanceof Object,b=object.key in container;for(object.slot of values){}let c=object.key/*keep*/in container"
    );
    assert_eq!(optimized.rewrites, 4);

    let runtime_source = "let object={item:{},key:\"present\",slot:0},container={present:true},values=[3,5],result=[];result.push(object[\"item\"]instanceof Object,object[\"key\"]in container,object[\"key\"]/*keep*/in container);for(object[\"slot\"]of values){result.push(object.slot)}console.log(JSON.stringify(result))";
    let runtime_optimized = optimize_emitted_without_regex_literals(runtime_source);
    assert_eq!(
        run_javascript(&runtime_optimized.code),
        run_javascript(runtime_source)
    );
}

#[test]
fn canonical_leaf_syntax_refuses_ambiguous_or_non_exact_spellings() {
    let source = "typeof(value).length;typeof(value)[\"length\"];typeof(value)();typeof(value)?.length;typeof(value)**2;typeof value;typeof(/*keep*/value);object[\"not-valid\"];object[\"\\x66oo\"];object[\"é\"];object?.[\"safe\"];object /*keep*/[\"safe\"];call()[\"safe\"];[\"safe\"];if(flag)[\"safe\"];let text=`typeof(value) object[\"safe\"]`";
    let optimized = optimize_emitted_without_regex_literals(source);

    assert_eq!(optimized.code, source);
    assert_eq!(optimized.rewrites, 0);

    // A regex body holds the same character sequences the rewrite selects.
    // The lexer reads each literal as one token, so only the spellings
    // outside them fold.
    // `use(a,b)` keeps both bindings live; without a reader they are dead
    // literal initializers and a different pass removes them outright.
    let regex =
        "let a=/typeof(value)/,b=/object[\"safe\"]/;typeof(value);object[\"safe\"];use(a,b)";
    let alongside_regex_literals = optimize_generated_javascript(regex).unwrap();
    assert_eq!(
        alongside_regex_literals.code,
        "typeof value;object.safe;use(/typeof(value)/,/object[\"safe\"]/)"
    );

    // `}` is the one predecessor that does not decide `/` on its own: a
    // block ends at statement position, an object or function expression
    // does not. Refuse the whole artifact rather than guess.
    let after_brace = "function f(){}/re/.test(source);typeof(value)";
    assert_eq!(
        optimize_generated_javascript(after_brace).unwrap().code,
        after_brace
    );
}

#[test]
fn canonical_leaf_syntax_preserves_runtime_property_and_typeof_behavior() {
    let source = "let reads=[],object=new Proxy({value:7,default:9},{get(target,key){reads.push(key);return target[key]}});let result=[typeof(object),object[\"value\"],object[\"default\"]];console.log(JSON.stringify([result,reads]))";
    let optimized = optimize_emitted_without_regex_literals(source);

    assert!(
        optimized.code.contains("typeof object"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("object.value"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("object.default"),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code), run_javascript(source));
}

#[test]
fn folds_assigned_truthy_ternary_to_logical_or() {
    let optimized = optimize_generated_javascript(
        "function f(n){if(n==null)return n+\"\";var s=typeof n;return \"object\"==s||\"function\"==s?(n=i[Object.prototype.toString.call(n)])?n:\"object\":s}",
    )
    .unwrap();
    assert!(
        optimized
            .code
            .contains("i[Object.prototype.toString.call(n)]||\"object\""),
        "{}",
        optimized.code
    );
    assert!(
        !optimized.code.contains(")?n:\"object\""),
        "{}",
        optimized.code
    );
}

#[test]
fn folds_an_assignment_followed_by_its_truthiness_guard() {
    let optimized = optimize_generated_javascript(
        "function f(x){a=read(x);if(a){use(a)}b=next();if(b){use(b)}}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function f(x){(a=read(x))&&use(a),(b=next())&&use(b);}"
    );
    assert!(optimized.rewrites >= 2);
}

#[test]
fn assignment_guard_folding_stays_within_proven_statement_boundaries() {
    let sources = [
        "function f(x){if(x)a=read();if(a)use(a)}",
        "function f(x){while(x)a=read();if(a)use(a)}",
        "function f(x){x?a=read():b=next();if(b)use(b)}",
        "function f(){a.x=read();if(a.x)use(a.x)}",
        "function f(){a=read();if(b)use(a)}",
    ];

    for source in sources {
        let optimized = optimize_generated_javascript(source).unwrap();
        assert_eq!(optimized.code, source);
    }

    let nested =
        optimize_generated_javascript("function f(){a=choose(call(1),{x:[2,3]});if(a){use(a)}}")
            .unwrap();
    assert_eq!(
        nested.code,
        "function f(){(a=choose(call(1),{x:[2,3]}))&&use(a);}"
    );
}

#[test]
fn reuses_a_dead_function_scoped_var_binding() {
    let optimized = optimize_generated_javascript(
        "function f(x){var a=first(x);if(a)use(a);var b=second(x);if(b)use(b)}",
    )
    .unwrap();

    assert_eq!(
        optimized.code,
        "function f(x){var a=first(x);if(a)use(a);if(a=second(x))use(a)}"
    );
    assert_eq!(optimized.rewrites, 2);

    let optimized = optimize_generated_javascript(
        "let f=x=>{var a=first(x);if(a)use(a);var b=second(x);if(b)use(b)};",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "let f=x=>{var a=first(x);if(a)use(a);if(a=second(x))use(a)};"
    );
}

#[test]
fn never_reuses_a_binding_from_a_sibling_arrow_scope() {
    let source = "let f=x=>{var a=first(x);return a},g=x=>{var b=second(x);return b};";
    let (optimized, reused) = super::reuse_dead_var_binding(source).unwrap();
    assert!(!reused);
    assert_eq!(optimized, source);

    for source in [
        "function f(){var a=first();var b=second();use(b=>a+b)}",
        "function f(){var a=first();var b=second();use((b)=>a+b)}",
        "function f(){var a=first();save(()=>{use(a)});var b=second();return b}",
        "function f(c,i){var l=function(){return 1};var $=function(){try{l()}catch(N){}};if(0!=c){$()}else{var m=i.Deferred;m.getErrorHook&&($.error=m.getErrorHook());setTimeout($)}}",
    ] {
        let (optimized, reused) = super::reuse_dead_var_binding(source).unwrap();
        assert!(!reused, "{optimized}");
        assert_eq!(optimized, source);
    }
}

#[test]
fn keeps_var_keyword_for_multi_declarator_reuse_candidates() {
    let source = "function f(x){var a=first(x);if(a)use(a);var b=second(x),i=0,tmp;for(;i<2;i++)tmp=b;return tmp}";
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(
        optimized.code.contains("var tmp,b=second(x),i=0"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("a=second(x)"));
}

#[test]
fn keeps_var_bindings_when_lifetimes_or_nested_names_overlap() {
    for (source, expected) in [
        (
            "function f(x){var a=first(x);var b=second(x);use(a,b)}",
            "function f(x){use(first(x),second(x))}",
        ),
        (
            "function f(x){use(b);var a=first(x);var b=second(x);use(b)}",
            "function f(x){use(b);var a=first(x),b=second(x);use(b)}",
        ),
        (
            "function f(x){var a=first(x);var b=second(x);use(function(b){return b})}",
            "function f(x){var a=first(x),b=second(x);use(function(b){return b})}",
        ),
    ] {
        let optimized = optimize_generated_javascript(source).unwrap();
        assert_eq!(optimized.code, expected);
    }
}

#[test]
fn removes_only_unreferenced_standalone_var_declarations() {
    let optimized = optimize_generated_javascript(
        "let f=a=>{var h;if(a)return 1;return b=>{var h;return b}},q=a=>{var g;return b=>{var x=0,g=1;return b+g}};function g(){var x;use(x)}",
    )
    .unwrap();
    // `var x=0` is dropped too: nothing reads `x`, and evaluating a literal
    // is unobservable, so the initializer cannot be missed.
    assert_eq!(
        optimized.code,
        "let f=a=>a?1:b=>b,q=a=>b=>{var g=1;return b+g};function g(){var x;use(x)}"
    );
    assert_eq!(optimized.rewrites, 7);

    let optimized = optimize_generated_javascript(
        "let f=(a,b)=>{var e;if(a)return b;return e=>{if(e)return e;return b}};",
    )
    .unwrap();
    assert_eq!(optimized.code, "let f=(a,b)=>a?b:e=>e?e:b;");
}

#[test]
fn keeps_bindings_whose_initializer_could_be_observed_or_read() {
    // An unread binding still has to stay when dropping it would drop a
    // call, a property read, or any other evaluation that can be observed.
    for source in [
        "function f(){var x=side();use(1)}",
        "function f(o){var x=o[k()];use(1)}",
        "function f(){var x=1+g();use(1)}",
        "function f(){let x=new C;use(1)}",
    ] {
        assert_eq!(
            optimize_generated_javascript(source).unwrap().code,
            source,
            "must keep an observable initializer: {source}"
        );
    }
    // A literal-initialized binding that is actually read also stays.
    let read = "function f(){var x=2;return x+1}";
    assert_eq!(optimize_generated_javascript(read).unwrap().code, read);
}

#[test]
fn rotates_only_proven_initial_true_flag_loops() {
    let optimized = optimize_generated_javascript(
        "function f(a){var b,c;b=!0;while(b){c=work(a);b=c<12;}return c}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function f(a){var c;do{c=work(a)}while(c<12);return c}"
    );

    let omitted_terminal_semicolon = optimize_generated_javascript(
        "function f(a){var b,c;b=!0;while(b){c=work(a);b=c<12}return c}",
    )
    .unwrap();
    assert_eq!(omitted_terminal_semicolon.code, optimized.code);

    assert_eq!(
        optimize_generated_javascript(
            "function f(){var a=true;while(a){if(read())continue;a=read()}}"
        )
        .unwrap()
        .code,
        "function f(){var a=true;while(a){if(read())continue;a=read()}}"
    );
    assert_eq!(
        optimize_generated_javascript("function f(){var a=true;while(a){use(a);a=read()}}")
            .unwrap()
            .code,
        "function f(){var a=true;while(a)use(a),a=read();}"
    );
    assert_eq!(
        optimize_generated_javascript("function f(){var a=false;while(a){work();a=read()}}")
            .unwrap()
            .code,
        "function f(){var a=false;while(a)work(),a=read();}"
    );
}

#[test]
fn merges_only_adjacent_same_kind_declarations() {
    let optimized = optimize_generated_javascript(
        "let a;let b=1;var c=2;var d=3;const e=4;const f=5;use(a,b,c,d,e,f)",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "let a,b=1;var c=2,d=3;const e=4,f=5;use(a,b,c,d,e,f)"
    );
}

#[test]
fn folds_guarded_returns_and_their_tails_into_one_conditional_return() {
    for (source, expected) in [
        (
            "function f(a){if(a)return 1;return 2}",
            "function f(a){return a?1:2}",
        ),
        (
            "function f(a,b){if(a)return 1;if(b)return 2;return 3}",
            "function f(a,b){return a?1:b?2:3}",
        ),
        (
            "function f(a){if(a){return 1}return 2}",
            "function f(a){return a?1:2}",
        ),
        (
            "function f(a){if(a)return;return 2}",
            "function f(a){return a?void 0:2}",
        ),
        (
            "function f(a){if(a)return 1;else return 2}",
            "function f(a){return a?1:2}",
        ),
        (
            "function f(a){if(a)return 1;else{return 2}}",
            "function f(a){return a?1:2}",
        ),
        (
            "function f(a,b){while(a)if(b)return 1;else return 2}",
            "function f(a,b){while(a)return b?1:2}",
        ),
        (
            "function f(a,b){if(a){if(b)return 1;return 2}return 3}",
            "function f(a,b){return a?b?1:2:3}",
        ),
        (
            "function f(a,b){if(a=b)return 1;return 2}",
            "function f(a,b){return(a=b)?1:2}",
        ),
        (
            "function f(a,b){if(a)return b,1;return 2}",
            "function f(a,b){return a?(b,1):2}",
        ),
        (
            "function f(a){if(/x/.test(a))return 1;return 2}",
            "function f(a){return/x/.test(a)?1:2}",
        ),
    ] {
        assert_eq!(
            optimize_generated_javascript(source).unwrap().code,
            expected
        );
    }
}

#[test]
fn refuses_return_tails_that_are_not_the_next_statement_of_the_same_block() {
    for source in [
        // The `if` is the loop body: fusing the tail into it would leave
        // the loop on the first iteration.
        "function f(a,b){while(b)if(a)return 1;return 2}",
        "function f(a,b){for(;b;)if(a)return 1;return 2}",
        "function f(a,b){l:if(a)return 1;return 2}",
        // Another statement runs between the two returns.
        "function f(a,b){if(a)return 1;b();return 2}",
        // The guarded arm is not a lone return.
        "function f(a,b){if(a){b();return 1}return 2}",
        "function f(a,b){if(a)return 1;else{b();return 2}}",
    ] {
        assert_eq!(optimize_generated_javascript(source).unwrap().code, source);
    }
}

#[test]
fn conditional_return_fusion_preserves_loop_and_ladder_behavior() {
    let source = "function classify(n){if(n<0)return\"negative\";if(n==0)return\"zero\";return\"positive\"}function scan(values){var i=0;while(i<values.length){if(values[i]>2)return i;i++}return -1}var out=[];for(var n of[-1,0,7])out.push(classify(n));out.push(scan([1,2,3]),scan([1,1,1]));console.log(JSON.stringify(out))";
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(
        optimized
            .code
            .contains("return n<0?\"negative\":n==0?\"zero\":\"positive\""),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code), run_javascript(source));
}

#[test]
fn folds_arrow_guard_returns_into_conditional_bodies() {
    let optimized = optimize_generated_javascript(
        "let f=(a,b)=>{if(a==b)return a;return c=>{if(c)return b;return a}};use(f)",
    )
    .unwrap();
    assert_eq!(optimized.code, "use((a,b)=>a==b?a:c=>c?b:a)");

    let undefined_arm = optimize_generated_javascript(
        "let f=(condition,fallback)=>{if(condition)return;return fallback()};use(f)",
    )
    .unwrap();
    assert_eq!(
        undefined_arm.code,
        "use((condition,fallback)=>condition?void 0:fallback())"
    );
}

#[test]
fn folds_expression_only_if_else_arms_into_a_conditional_sequence() {
    let optimized = optimize_generated_javascript(
        "function f(a){if(test(a)){a=a+1;b=a<12;}else{b=false;}return b}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function f(a){test(a)?(a++,b=a<12):b=false;return b}"
    );
    assert_eq!(optimized.rewrites, 4);

    let optimized = optimize_generated_javascript(
        "function f(x){if(x){first(),second()}else{third(),fourth()}}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function f(x){x?(first(),second()):(third(),fourth());}"
    );

    let optimized =
        optimize_generated_javascript("function f(x){if(x){console.log(1)}else{console.log(0)}}")
            .unwrap();
    assert_eq!(optimized.code, "function f(x){console.log(x?1:0);}");

    let optimized = optimize_generated_javascript(
        "q.call(r,\"a\")?console.log(1):console.log(0);q.call(r,\"b\")?console.log(1):console.log(0)",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "console.log(q.call(r,\"a\")?1:0);console.log(q.call(r,\"b\")?1:0);"
    );

    for source in [
        "function f(a){if(a){let b=1;use(b)}else{use(a)}}",
        "function f(a){if(a)use(a);else use(0)}",
    ] {
        assert_eq!(optimize_generated_javascript(source).unwrap().code, source);
    }

    // Two returning arms are a conditional return, not a conditional
    // sequence, so the dedicated fold owns them.
    assert_eq!(
        optimize_generated_javascript("function f(a){if(a){return 1}else{return 2}}")
            .unwrap()
            .code,
        "function f(a){return a?1:2}"
    );
}

#[test]
fn groups_assignment_results_used_as_conditional_tests() {
    let optimized = optimize_generated_javascript(
        "let f=()=>{flag=!flag;if(flag)return 'first';return 'second'};use(f)",
    )
    .unwrap();
    assert_eq!(optimized.code, "use(()=>(flag=!flag)?'first':'second')");
}

#[test]
fn groups_sequence_expressions_used_as_conditional_arms() {
    let optimized = optimize_generated_javascript(
        "let f=x=>{if(x)return first(),second();return third(),fourth()};use(f)",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "use(x=>x?(first(),second()):(third(),fourth()))"
    );
}

#[test]
fn derives_stable_nonzero_startup_metrics() {
    let metrics = analyze_generated_javascript(
        "function f(a){while(a)a=f(a-1.25);return a}console.log(f(.5))",
    )
    .unwrap();
    assert_eq!(metrics.functions, 1);
    assert_eq!(metrics.loops, 1);
    assert!(metrics.tokens > 10);
    assert!(metrics.ast_nodes > 5);
    assert!(metrics.parse_cost > 0);
    assert!(metrics.compile_cost > metrics.parse_cost / 2);
    assert!(metrics.estimated_memory_bytes > metrics.bytes as u64);
}

#[test]
fn startup_metrics_count_overlapping_expression_regions_once() {
    let tokens = lex("a?b:c?d:e?f:g").unwrap();
    let parsed = parse_expression_regions(&tokens);
    let overlapping_sum = parsed
        .iter()
        .map(|region| region.expression.node_count())
        .sum::<usize>();
    let non_overlapping = non_overlapping_parsed_node_count(&parsed);

    assert!(
        parsed.len() > 1,
        "expected nested conditional suffix regions"
    );
    assert!(non_overlapping < overlapping_sum);
    assert_eq!(non_overlapping, parsed[0].expression.node_count());
    let metrics = analyze_generated_javascript("a?b:c?d:e?f:g").unwrap();
    assert!(metrics.ast_nodes <= metrics.tokens, "{metrics:?}");
}

#[test]
fn startup_metrics_still_add_disjoint_expression_regions() {
    let tokens = lex("a+b;c*d").unwrap();
    let parsed = parse_expression_regions(&tokens);
    let independent_sum = parsed
        .iter()
        .map(|region| region.expression.node_count())
        .sum::<usize>();

    assert_eq!(parsed.len(), 2, "{parsed:?}");
    assert_eq!(non_overlapping_parsed_node_count(&parsed), independent_sum);
}

#[test]
fn nesting_metric_includes_expression_depth_without_delimiters() {
    let shallow = analyze_generated_javascript("a?b:c").unwrap();
    let deep = analyze_generated_javascript("a?b?c?d:e:f:g").unwrap();
    assert!(
        deep.max_nesting > shallow.max_nesting,
        "{shallow:?} {deep:?}"
    );
}

#[test]
fn remaps_only_identifier_tokens_for_entropy_probes() {
    let mut mapping = std::array::from_fn(|index| index as u8);
    mapping[b'a' as usize] = b'z';
    assert_eq!(
        super::remap_single_character_identifiers(
            "let a='a';a=obj.a+1e-7;console.log(`${a}`)",
            &mapping,
        )
        .unwrap(),
        "let z='a';z=obj.z+1e-7;console.log(`${z}`)"
    );
}

#[test]
fn clear_binding_names_exclude_properties_and_object_keys() {
    assert!(super::single_character_name_is_clear_binding("let O=1;O.fn=O", b'O').unwrap());
    assert!(
        super::single_character_name_is_clear_binding("function X(r){return X(r)}", b'X').unwrap()
    );
    assert!(super::single_character_name_is_clear_binding("export{O as jQuery}", b'O').unwrap());
    assert!(!super::single_character_name_is_clear_binding("console.log(x.O)", b'O').unwrap());
    assert!(!super::single_character_name_is_clear_binding("let O={O:1}", b'O').unwrap());
    assert!(!super::single_character_name_is_clear_binding("f({O})", b'O').unwrap());
    assert_eq!(
        super::single_character_identifier_use_counts("let O=O.fn+O").unwrap()[b'O' as usize],
        3
    );
}

#[test]
fn remaps_two_character_bindings_without_touching_longer_names() {
    let source = "var ge=i.apply(r,n);if(ge==r.promise()){return ge}console.log(obj.get,merge)";
    assert!(super::identifier_name_is_clear_binding(source, "ge").unwrap());
    assert_eq!(
        super::remap_identifier(source, "ge", "d").unwrap(),
        "var d=i.apply(r,n);if(d==r.promise()){return d}console.log(obj.get,merge)"
    );
    assert!(!super::identifier_name_is_clear_binding("f({ge:1,ge})", "ge").unwrap());
}

#[test]
fn scans_unicode_in_nested_template_interpolations_without_splitting_utf8() {
    let source = "let f=a=>`${show(`value × ${a}`)}`;console.log(f(2))";
    let metrics = analyze_generated_javascript(source).unwrap();
    assert!(metrics.tokens > 5, "{metrics:?}");

    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(optimized.code.contains('×'), "{}", optimized.code);
}

#[test]
fn rejects_malformed_generated_javascript() {
    let error = analyze_generated_javascript("function f(){return [1,2}").unwrap_err();
    assert_eq!(error.offset(), 24);
}
