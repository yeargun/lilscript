use super::folds::{
    fold_early_exit_guards, fold_fresh_empty_array_pushes, fold_identifier_copies,
    fold_identity_arrow_iife, fold_if_expression_to_and, fold_sequence_assignments_into_first_use,
    fold_single_use_if_assigns, fold_single_use_temporaries,
    fold_statement_assignments_into_first_use, fold_typeof_identifier_caches,
};
use super::parse::{non_overlapping_parsed_node_count, parse_expression_regions};
use super::token::{lex, punctuation_width};
use super::{
    analyze_generated_javascript, function_leading_declaration_variant,
    generated_javascript_binding_occurrences, generated_javascript_bit_or_zero_count,
    generated_javascript_dynamic_property_occurrences, generated_javascript_export_names,
    generated_javascript_export_witnesses, generated_javascript_free_identifiers,
    generated_javascript_static_imports, generated_javascript_static_property_names,
    generated_javascript_static_property_occurrences, generated_javascript_template_literals,
    optimize_generated_javascript, optimize_generated_javascript_assuming,
    optimize_generated_javascript_preserving_functions, reorder_uninitialized_var_declarators,
    validate_generated_javascript_syntax_floor, PeepholeResult,
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
        "function f(a,b){a+=b;if(a+b)a*=2;for(;a<9;a++)b^=a;return a}"
    );
    // Five compound-assignment rewrites, plus the two that fold `let c=a+b`
    // into its only use.
    assert_eq!(optimized.rewrites, 7);
}

#[test]
fn preserves_non_identifier_assignments_and_different_operands() {
    let source = "a.x=a.x+1;a=b+1;return a";
    let optimized = optimize_generated_javascript(source).unwrap();
    // Neither assignment is folded; only the top-level comma join applies.
    // The implicit `a=` becomes a module binding so the fragment is valid ESM.
    assert_eq!(optimized.code, "var a;a.x=a.x+1,a=b+1;return a");
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

    // No negation is folded; only the top-level comma join applies.
    assert_eq!(
        optimized.code,
        "!(x==y).z,!(x==y)(z),!(x==y)[z],!(x==y)?.z,!(x==y)`tag`,!(x==y)++,!(x==y)--,!(x==y)**z,new!(x==y);function* g(){return!(yield x==y)}!({}==x),!(function(){}==x),!(class{}==x),!(async function(){}==x)"
    );
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
        "function plain(){var empty,initialized=read();return[initialized,empty]}"
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

    // No spelling is canonicalized; only the top-level comma join applies,
    // stopping before the `if` statement and the declaration.
    assert_eq!(
        optimized.code,
        "typeof(value).length,typeof(value)[\"length\"],typeof(value)(),typeof(value)?.length,typeof(value)**2,typeof value,typeof(/*keep*/value),object[\"not-valid\"],object[\"\\x66oo\"],object[\"é\"],object?.[\"safe\"],object /*keep*/[\"safe\"],call()[\"safe\"],[\"safe\"];if(flag)[\"safe\"];let text=`typeof(value) object[\"safe\"]`"
    );

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
        "typeof value,object.safe,use(/typeof(value)/,/object[\"safe\"]/)"
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
        "function f(x){var a;var b;(a=read(x))&&use(a),(b=next())&&use(b)}"
    );
    assert!(optimized.rewrites >= 2);
}

#[test]
fn a_comma_sequence_assignment_is_not_an_if_condition() {
    let source = concat!(
        "function go(g,t){var i=[],h=[],d=0,f,c;",
        "while(d<g){f=d<t,c=d+1;if(f)h.push(c);else if(c!=null)i.push(c);d++}",
        "return JSON.stringify([i,h])}",
        "process.stdout.write(go(1,0)+go(2,1))",
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(!optimized.code.contains("if(f="), "{}", optimized.code);
    let output = std::process::Command::new("node")
        .arg("-e")
        .arg(&optimized.code)
        .output()
        .expect("node must execute generated JavaScript");
    assert!(
        output.status.success(),
        "node failed:\n{}\nsource:\n{}",
        String::from_utf8_lossy(&output.stderr),
        optimized.code
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("node stdout must be UTF-8"),
        "[[1],[]][[2],[1]]"
    );
}

#[test]
fn assignment_guard_folding_stays_within_proven_statement_boundaries() {
    let sources = [
        "function f(x){if(x)a=read();if(a)use(a)}",
        "function f(x){while(x)a=read();if(a)use(a)}",
        "function f(){a.x=read();if(a.x)use(a.x)}",
    ];

    for source in sources {
        let optimized = optimize_generated_javascript(source).unwrap();
        assert_eq!(optimized.code, source);
    }

    assert_eq!(
        optimize_generated_javascript("function f(x){x?a=read():b=next();if(b)use(b)}")
            .unwrap()
            .code,
        "function f(x){var a;var b;x?a=read():b=next();if(b)use(b)}"
    );

    assert_eq!(
        optimize_generated_javascript("function f(){a=read();if(b)use(a)}")
            .unwrap()
            .code,
        "function f(){var a;a=read();if(b)use(a)}"
    );

    let nested =
        optimize_generated_javascript("function f(){a=choose(call(1),{x:[2,3]});if(a){use(a)}}")
            .unwrap();
    assert_eq!(
        nested.code,
        "function f(){var a;(a=choose(call(1),{x:[2,3]}))&&use(a)}"
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
        "let f=x=>{var a=first(x);if(a)use(a);if(a=second(x))use(a)}"
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
        "function f(c){(()=>function(){try{}catch(e){var j=first();use(j)}})();if(c){if(c){var m=second();return m}}}",
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
            "function f(x){var a=first(x),b=second(x);use(a,b)}",
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
    assert_eq!(optimized.code, "let f=(a,b)=>a?b:e=>e||b");
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
        "function f(){var a=true;while(a)if(!(read()))a=read()}"
    );
    assert_eq!(
        optimize_generated_javascript("function f(){var a=true;while(a){use(a);a=read()}}")
            .unwrap()
            .code,
        "function f(){var a=true;while(a)use(a),a=read()}"
    );
    assert_eq!(
        optimize_generated_javascript("function f(){var a=false;while(a){work();a=read()}}")
            .unwrap()
            .code,
        "function f(){var a=false;while(a)work(),a=read()}"
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
        // An else arm with a prefix is not a lone-return pair.
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
    assert_eq!(optimized.code, "let f=(a,b)=>a==b?a:c=>c?b:a;use(f)");

    let undefined_arm = optimize_generated_javascript(
        "let f=(condition,fallback)=>{if(condition)return;return fallback()};use(f)",
    )
    .unwrap();
    assert_eq!(
        undefined_arm.code,
        "let f=(condition,fallback)=>condition?void 0:fallback();use(f)"
    );
}

#[test]
fn folds_expression_only_if_else_arms_into_a_conditional_sequence() {
    let optimized = optimize_generated_javascript(
        "function f(a){if(test(a)){a=a+1;b=a<12;}else{b=false;}return b}",
    )
    .unwrap();
    assert!(
        optimized.code == "function f(a){test(a)?(a++,b=a<12):b=false;return b}"
            || optimized.code == "function f(a){var b;test(a)?(a++,b=a<12):b=false;return b}",
        "{}",
        optimized.code
    );

    let optimized = optimize_generated_javascript(
        "function f(x){if(x){first(),second()}else{third(),fourth()}}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function f(x){x?(first(),second()):(third(),fourth())}"
    );

    let optimized =
        optimize_generated_javascript("function f(x){if(x){console.log(1)}else{console.log(0)}}")
            .unwrap();
    assert_eq!(optimized.code, "function f(x){console.log(x?1:0)}");

    let optimized = optimize_generated_javascript(
        "q.call(r,\"a\")?console.log(1):console.log(0);q.call(r,\"b\")?console.log(1):console.log(0)",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "console.log(q.call(r,\"a\")?1:0),console.log(q.call(r,\"b\")?1:0)"
    );

    // The declaration keeps the arm from becoming a conditional sequence;
    // retaining it also avoids moving initialization after callee lookup.
    let collapsed =
        optimize_generated_javascript("function f(a){if(a){let b=1;use(b)}else{use(a)}}")
            .unwrap()
            .code;
    assert!(
        collapsed == "function f(a){if(a){let b=1;use(b)}else{use(a)}}"
            || collapsed == "function f(a){if(a){let b=1;use(b)}else use(a)}",
        "{collapsed}"
    );
    let source = "function f(a){if(a)use(a);else use(0)}";
    assert_eq!(optimize_generated_javascript(source).unwrap().code, source);

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
fn folds_unread_increment_snapshots() {
    let (optimized, rewritten) = crate::js_peephole::fold_dead_increment_snapshots(
        "function f(c,d){if(c<b){var h=c;c++,d.consume(s);return a}return c}",
    )
    .unwrap();
    assert!(rewritten > 0);
    assert_eq!(
        optimized,
        "function f(c,d){if(c<b){c++,d.consume(s);return a}return c}"
    );
    let (kept, kept_rewrites) =
        crate::js_peephole::fold_dead_increment_snapshots("function f(c){var h=c;c++;return h}")
            .unwrap();
    assert_eq!(kept_rewrites, 0);
    assert_eq!(kept, "function f(c){var h=c;c++;return h}");
}

#[test]
fn folds_pristine_static_method_call_this_arg() {
    let (optimized, rewritten) = crate::js_peephole::fold_pristine_static_method_calls(
        "function f(a,b){return Object.assign.call(Object,a,b)+String.fromCharCode.call(String,c)+Array.from.call(Array,d)}",
    )
    .unwrap();
    assert!(rewritten > 0);
    assert_eq!(
        optimized,
        "function f(a,b){return Object.assign(a,b)+String.fromCharCode(c)+Array.from(d)}"
    );
    let (kept, kept_rewrites) = crate::js_peephole::fold_pristine_static_method_calls(
        "function f(a,b){return Object.prototype.hasOwnProperty.call(a,b)}",
    )
    .unwrap();
    assert_eq!(kept_rewrites, 0);
    assert!(kept.contains("hasOwnProperty.call"), "{kept}");
}

#[test]
fn folds_if_body_prefix_into_returned_comma_sequence() {
    let optimized = optimize_generated_javascript(
        "function f(c,e,b,ok,nok){if(c){e.consume(b);return ok}return nok}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function f(c,e,b,ok,nok){return c?(e.consume(b),ok):nok}"
    );

    let chained = optimize_generated_javascript(
        "function f(c,d,e){if(c){e.consume();return a}if(d){e.exit();return b}return n}",
    )
    .unwrap();
    assert_eq!(
        chained.code,
        "function f(c,d,e){return c?(e.consume(),a):d?(e.exit(),b):n}"
    );

    let nested = optimize_generated_javascript(
        "function f(c,d,e){if(c){if(d){e.consume();return a}}return n}",
    )
    .unwrap();
    assert_eq!(
        nested.code,
        "function f(c,d,e){return c&&d?(e.consume(),a):n}"
    );

    let or_cond =
        optimize_generated_javascript("function f(c,d,e){if(c||d){if(e){g();return a}}return n}")
            .unwrap();
    assert_eq!(or_cond.code, "function f(c,d,e){return(c||d)&&e?(g(),a):n}");

    let bare = optimize_generated_javascript("function f(c,e){if(c){e.consume();return}return 1}")
        .unwrap();
    assert!(bare.code.contains("e.consume()"), "{}", bare.code);
    assert!(
        !bare.code.contains("return e.consume()"),
        "bare return must not become a valued comma: {}",
        bare.code
    );
}

#[test]
fn groups_assignment_results_used_as_conditional_tests() {
    let optimized = optimize_generated_javascript(
        "let f=()=>{flag=!flag;if(flag)return 'first';return 'second'};use(f)",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "let f=()=>(flag=!flag)?'first':'second';use(f)"
    );
}

#[test]
fn groups_sequence_expressions_used_as_conditional_arms() {
    let optimized = optimize_generated_javascript(
        "let f=x=>{if(x)return first(),second();return third(),fourth()};use(f)",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "let f=x=>x?(first(),second()):(third(),fourth());use(f)"
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
    assert!(
        super::single_character_name_is_clear_binding("let O=1;export{O as jQuery}", b'O').unwrap()
    );
    assert!(!super::single_character_name_is_clear_binding("export{O as jQuery}", b'O').unwrap());
    assert!(!super::single_character_name_is_clear_binding("console.log(x.O)", b'O').unwrap());
    assert!(!super::single_character_name_is_clear_binding("let O={O:1}", b'O').unwrap());
    assert!(!super::single_character_name_is_clear_binding("f({O})", b'O').unwrap());
    assert_eq!(
        super::single_character_identifier_use_counts("let O=O.fn+O").unwrap()[b'O' as usize],
        3
    );
}

#[test]
fn counts_declared_binding_characters_without_property_or_public_export_noise() {
    let counts = super::declared_identifier_character_use_counts(
        "let q=Object.q;function fn(a){var z=a;return z+q}export{q as publicName}",
    )
    .unwrap();
    assert_eq!(counts[b'q' as usize], 3);
    assert_eq!(counts[b'f' as usize], 1);
    assert_eq!(counts[b'n' as usize], 1);
    assert_eq!(counts[b'a' as usize], 2);
    assert_eq!(counts[b'z' as usize], 2);
    assert_eq!(counts[b'O' as usize], 0);
    assert_eq!(counts[b'p' as usize], 0);
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
fn two_character_remapping_rejects_ambient_and_unresolved_names() {
    assert!(!super::identifier_name_is_clear_binding("console.log(ge)", "ge").unwrap());
    assert!(
        !super::identifier_name_is_clear_binding("function f({x}){return ge+x}", "ge").unwrap()
    );
    assert_eq!(
        super::remap_identifier("console.log(ge)", "ge", "a").unwrap(),
        "console.log(ge)"
    );
}

#[test]
fn two_character_remapping_rejects_unresolved_template_occurrences() {
    let source = "let ge=1;console.log(`${ge}`)";
    assert!(!super::identifier_name_is_clear_binding(source, "ge").unwrap());
    assert_eq!(super::remap_identifier(source, "ge", "a").unwrap(), source);
}

#[test]
fn one_character_remap_candidates_reject_template_expressions() {
    let source = "let a=1;console.log(`${a+1}`)";
    assert!(super::single_character_resolved_binding_identifiers(source)
        .unwrap()
        .is_empty());
    assert!(!super::single_character_name_is_clear_binding(source, b'a').unwrap());
    assert!(
        super::function_local_binding_swap_variants("function f(a,b){return`${a+b}`}")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn fresh_object_assignment_collection_requires_pristine_builtins() {
    let source = "let o={};o.x=value;export{o}";
    let ordinary = optimize_generated_javascript(source).unwrap().code;
    assert!(
        ordinary.contains("o.x=value"),
        "an inherited setter must remain observable: {ordinary}"
    );

    let pristine = optimize_generated_javascript_assuming(source, true)
        .unwrap()
        .code;
    assert!(pristine.contains("{x:value}"), "{pristine}");
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

#[test]
fn observes_generated_export_names_and_aliases() {
    assert_eq!(
        generated_javascript_export_names("let a=1,b=2;export{a as left,b, a as default}").unwrap(),
        ["b", "default", "left"]
    );
}

#[test]
fn rejects_duplicate_generated_export_names() {
    let error =
        generated_javascript_export_names("let a=1,b=2;export{a as value,b as value}").unwrap_err();
    assert!(error
        .to_string()
        .contains("duplicate generated export name"));
}

#[test]
fn observes_generated_export_callable_shapes() {
    let witnesses = generated_javascript_export_witnesses(
        "class B{baseField=0;base(a){}}function f(a,b=1){}class C extends B{ownField=0;constructor(a,b){}read(a=1){}}let g=(a,b)=>a+b,v=1;export{f,C,g,v}",
    )
    .unwrap();
    assert_eq!(witnesses.len(), 4);
    let f = witnesses
        .iter()
        .find(|witness| witness.name == "f")
        .unwrap();
    assert_eq!(f.kind, super::GeneratedJavaScriptExportKind::Function);
    assert_eq!(f.arity, Some(1));
    assert_eq!(f.constructible, Some(true));
    let class = witnesses
        .iter()
        .find(|witness| witness.name == "C")
        .unwrap();
    assert_eq!(
        class.kind,
        super::GeneratedJavaScriptExportKind::Constructor
    );
    assert_eq!(class.arity, Some(2));
    assert_eq!(class.fields, ["baseField", "ownField"]);
    assert_eq!(
        class
            .methods
            .iter()
            .map(|method| (method.name.as_str(), method.arity))
            .collect::<Vec<_>>(),
        [("base", 1), ("read", 0)]
    );
    let arrow = witnesses
        .iter()
        .find(|witness| witness.name == "g")
        .unwrap();
    assert_eq!(arrow.kind, super::GeneratedJavaScriptExportKind::Function);
    assert_eq!(arrow.arity, Some(2));
    assert_eq!(arrow.constructible, Some(false));
    let value = witnesses
        .iter()
        .find(|witness| witness.name == "v")
        .unwrap();
    assert_eq!(value.kind, super::GeneratedJavaScriptExportKind::Value);
}

#[test]
fn observes_generated_static_import_edges_without_local_aliases() {
    assert_eq!(
        generated_javascript_static_imports(
            "import'./setup.ts';import{value as a,other}from\"pkg\";let p=import('./lazy.js')"
        )
        .unwrap(),
        [
            ("./setup.ts".to_string(), Vec::new()),
            (
                "pkg".to_string(),
                vec!["other".to_string(), "value".to_string()]
            )
        ]
    );
}

#[test]
fn counts_generated_bit_or_zero_obligations_from_tokens() {
    assert_eq!(
        generated_javascript_bit_or_zero_count(
            "let text='not |0';let a=value|0,b=(other|0)+1,c=value|1"
        )
        .unwrap(),
        2
    );
}

#[test]
fn observes_static_properties_without_confusing_dynamic_keys() {
    assert_eq!(
        generated_javascript_static_property_names(
            "class C{field=0;method(){return this.field}}let o={named:1,'quoted':2};o.static;o['bracket'];o[key]"
        )
        .unwrap(),
        ["bracket", "field", "method", "named", "quoted", "static"]
    );
    let occurrences = generated_javascript_static_property_occurrences("value.field").unwrap();
    assert_eq!(occurrences[0].name, "field");
    assert_eq!(
        &"value.field"[occurrences[0].start..occurrences[0].end],
        "field"
    );
}

#[test]
fn records_dynamic_property_ranges_separately_from_static_keys() {
    let source = "value[key]+value['fixed']+call()[next()]";
    let occurrences = generated_javascript_dynamic_property_occurrences(source).unwrap();
    assert_eq!(
        occurrences
            .iter()
            .map(|(start, end)| &source[*start..*end])
            .collect::<Vec<_>>(),
        ["[key]", "[next()]"]
    );
}

#[test]
fn observes_free_identifiers_without_properties_or_bound_names() {
    assert_eq!(
        generated_javascript_free_identifiers(
            "let local=external;function f(arg){return local+arg+other.value}"
        )
        .unwrap(),
        ["external", "other"]
    );
}

#[test]
fn records_binding_and_declaration_byte_ranges() {
    let source = "let value=1;function read(){return value+external}";
    let bindings = generated_javascript_binding_occurrences(source).unwrap();
    let captured = bindings
        .iter()
        .find(|binding| binding.name == "value" && binding.start > 20)
        .unwrap();
    assert_eq!(captured.kind, super::GeneratedJavaScriptBindingKind::Bound);
    assert_eq!(captured.declaration_start, Some(4));
    assert_eq!(captured.declaration_end, Some(9));
    let external = bindings
        .iter()
        .find(|binding| binding.name == "external")
        .unwrap();
    assert_eq!(external.kind, super::GeneratedJavaScriptBindingKind::Free);
}

#[test]
fn records_opaque_template_literals_exactly() {
    assert_eq!(
        generated_javascript_template_literals("let a=`x${value}`;let b=`plain`").unwrap(),
        ["`plain`", "`x${value}`"]
    );
}

#[test]
fn rejects_generated_syntax_above_the_configured_floor() {
    use crate::js_syntax_target::EcmaScriptEdition;

    validate_generated_javascript_syntax_floor("let a=o?.x??0", EcmaScriptEdition::Es2020).unwrap();
    let error = validate_generated_javascript_syntax_floor("let a=o?.x", EcmaScriptEdition::Es2019)
        .unwrap_err();
    assert!(error.to_string().contains("syntax floor"));
    let error =
        validate_generated_javascript_syntax_floor("class A{x=0}", EcmaScriptEdition::Es2021)
            .unwrap_err();
    assert!(error.to_string().contains("syntax floor"));
    validate_generated_javascript_syntax_floor(
        "let f=(...a)=>a;let o={...source}",
        EcmaScriptEdition::Es2018,
    )
    .unwrap();
    let error = validate_generated_javascript_syntax_floor(
        "let f=(...a)=>a;let o={...source}",
        EcmaScriptEdition::Es2017,
    )
    .unwrap_err();
    assert!(error.to_string().contains("syntax floor"));
}

#[test]
fn rejects_duplicate_generated_top_level_bindings() {
    let source = "let O=a=>a;let O=/\\D/g;export{O}";
    let error = analyze_generated_javascript(source).unwrap_err();
    assert_eq!(error.offset(), source.find("O=/").unwrap());
    assert!(error
        .to_string()
        .contains("duplicate generated top-level binding"));
}

#[test]
fn permits_the_same_generated_binding_in_nested_scopes() {
    analyze_generated_javascript("let O=1;let f=()=>{let O=2;return O};export{O,f}").unwrap();
}

#[test]
fn permits_a_named_class_expression_assigned_to_an_existing_binding() {
    analyze_generated_javascript("var Ne=0;Ne=class Ne{constructor(){this.x=1}}").unwrap();
}

#[test]
fn permits_a_class_method_that_reuses_a_sibling_function_parameter() {
    analyze_generated_javascript(
        "function f(S){return S}class u{constructor(){this.x=1}S(a){return a}get t(){return this.x}}",
    )
    .unwrap();
}

#[test]
fn still_rejects_a_class_method_body_that_reads_a_sibling_local() {
    let source = "function f(){var y=1;return y}class u{S(a){return y}}";
    let error = analyze_generated_javascript(source).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unresolved generated identifier"),
        "{error}"
    );
}

#[test]
fn permits_var_redeclaration_of_a_module_binding() {
    analyze_generated_javascript(
        "var e=[];Object.freeze(e);var e=C.prototype;C.m=function(){return 1};export{C}",
    )
    .unwrap();
}

#[test]
fn rejects_var_colliding_with_a_class_declaration() {
    let error = analyze_generated_javascript("class e{constructor(){this.x=1}}var e=[];export{e}")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("duplicate generated top-level binding"),
        "{error}"
    );
}

#[test]
fn rejects_a_boolean_fused_into_a_class_body() {
    let error =
        analyze_generated_javascript("class C{set x(t){this.x=t}!1{this.y=1}z(){return this.x}}")
            .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid generated class element"),
        "{error}"
    );
}

#[test]
fn permits_a_computed_false_class_method() {
    analyze_generated_javascript(
        "class C{constructor(){this.y=1}[!1](){this.y=0}z(){return this.y}}",
    )
    .unwrap();
}

#[test]
fn permits_public_class_field_initializers() {
    let source = "class C{x=0;ready=!1;label='';items=[];constructor(){this.x=1}}";
    analyze_generated_javascript(source).unwrap();
    let (declared, rewrites) = super::folds::declare_implicit_assignment_bindings(source).unwrap();
    assert_eq!(rewrites, 0, "{declared}");
    assert_eq!(declared, source);
}

#[test]
fn rejects_a_comma_between_a_class_field_and_method() {
    let error = analyze_generated_javascript("class C{x=0,constructor(){this.x=1}}")
        .expect_err("a comma cannot terminate a public class field");
    assert!(
        error
            .to_string()
            .contains("invalid generated class element"),
        "{error}"
    );
}

#[test]
fn rejects_a_var_declaration_in_a_class_body() {
    let error = analyze_generated_javascript(
        "class C{var j;constructor(c,d,j=[]){this.x=j}buildFromUnknown(j,m={}){return m}}",
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid generated class element"),
        "{error}"
    );
}

#[test]
fn rejects_a_declaration_in_a_for_update_clause() {
    let error = analyze_generated_javascript(
        "function each(r,n){for(var t=r.values();!t.next().done;var e;)r.call(n,e.value)}",
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid generated for-update clause"),
        "{error}"
    );
}

#[test]
fn permits_a_c_style_for_with_empty_update() {
    analyze_generated_javascript("function each(t){for(var i=t.values();!i.next().done;){}}")
        .unwrap();
}

#[test]
fn rejects_a_local_read_from_a_sibling_function() {
    let source = "function a(){var y=/a/;return y.test(\"a\")}function b(e){return y.exec(e)}";
    let error = analyze_generated_javascript(source).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unresolved generated identifier"),
        "{error}"
    );
}

#[test]
fn rejects_a_sibling_local_leaked_through_a_shared_iife() {
    let source = "var S=(function(){function list(){var y=/a/;return y.test(\"a\")}function table(e){return y.exec(e)}return table})()";
    let error = analyze_generated_javascript(source).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unresolved generated identifier"),
        "{error}"
    );
}

#[test]
fn permits_sibling_function_declarations_inside_an_iife() {
    analyze_generated_javascript(
        "var ea=(function(){function a(){return 1}function b(m){return a()+m}return b})();var W=0",
    )
    .unwrap();
}

#[test]
fn permits_sibling_functions_that_reuse_the_same_parameter_name() {
    analyze_generated_javascript("function a(e){return e}function b(e){return e}").unwrap();
}

#[test]
fn permits_static_module_names_that_match_nested_bindings() {
    analyze_generated_javascript(
        "import{track as importedTrack}from\"pkg\";function a(){function track(){}return importedTrack}let value=1;export{value as track}",
    )
    .unwrap();
}

#[test]
fn imported_alias_is_visible_at_module_scope() {
    analyze_generated_javascript(
        "import{value as importedValue}from\"pkg\";function a(){function importedValue(){}}console.log(importedValue)",
    )
    .unwrap();
}

#[test]
fn permits_an_expression_arrow_parameter_that_matches_a_sibling_local() {
    analyze_generated_javascript("function a(x){return x}let n=x=>x+1;console.log(n(2))").unwrap();
}

#[test]
fn permits_a_catch_parameter_that_matches_a_sibling_local() {
    analyze_generated_javascript(
        "function a(){var v31=0;return v31}function b(){try{return 1}catch(v31){return v31}}",
    )
    .unwrap();
}

#[test]
fn permits_a_later_var_after_an_asi_var_list_that_matches_a_sibling() {
    analyze_generated_javascript(
        "function a(){if(x){var y=1,z}var v130=2;return v130}function b(){var v130=0;return v130}",
    )
    .unwrap();
}

#[test]
fn permits_a_nested_function_to_read_an_outer_var() {
    analyze_generated_javascript("function a(){var y=/a/;return function b(e){return y.exec(e)}}")
        .unwrap();
}

fn assert_generated_binding_index_matches_reference(source: &str) {
    let tokens = lex(source).unwrap();
    let matching_close = super::token::matching_closers(&tokens);
    let bindings = super::scope::GeneratedBindingIndex::new(&tokens, &matching_close);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != super::token::TokenKind::Identifier {
            continue;
        }
        let indexed_binding = bindings.identifier_is_binding(index);
        let reference_binding =
            super::generated_identifier_is_binding(&tokens, &matching_close, index);
        assert_eq!(
            indexed_binding, reference_binding,
            "binding occurrence differed for {:?} at {} in {source}",
            token.text, token.start,
        );
        assert_eq!(
            bindings.enclosing_function_span(index),
            super::scope::enclosing_function_span(&tokens, &matching_close, index),
            "enclosing function differed for {:?} at {} in {source}",
            token.text,
            token.start,
        );
        assert_eq!(
            bindings.name_is_declared_in_any_scope(index, token.text),
            super::scope::name_is_declared_in_any_enclosing_scope(
                &tokens,
                &matching_close,
                index,
                token.text,
            ),
            "enclosing declaration differed for {:?} at {} in {source}",
            token.text,
            token.start,
        );
        assert_eq!(
            bindings.name_is_module_var_binding(token.text),
            super::scope::name_is_module_var_binding(&tokens, &matching_close, token.text),
            "module var binding differed for {:?} at {} in {source}",
            token.text,
            token.start,
        );
        if bindings.enclosing_function_span(index).is_some() {
            assert_eq!(
                bindings.name_is_declared_in_enclosing_function_scope(index, token.text),
                super::scope::name_is_declared_in_any_enclosing_function_scope(
                    &tokens,
                    &matching_close,
                    index,
                    token.text,
                ),
                "enclosing function declaration differed for {:?} at {} in {source}",
                token.text,
                token.start,
            );
        }
        if super::identifier_occurrence_is_clear_binding(&tokens, index) {
            assert_eq!(
                indexed_binding || bindings.name_is_visible(index, token.text),
                reference_binding
                    || super::scope::name_is_visible_generated_binding(
                        &tokens,
                        &matching_close,
                        index,
                        token.text,
                    ),
                "resolved binding differed for {:?} at {} in {source}",
                token.text,
                token.start,
            );
        }
        if super::rewrite::is_property_identifier(&tokens, index)
            || reference_binding
            || super::generated_identifier_is_ambient(token.text)
        {
            continue;
        }
        let indexed_visible = bindings.name_is_visible(index, token.text);
        let reference_visible = super::scope::name_is_visible_generated_binding(
            &tokens,
            &matching_close,
            index,
            token.text,
        );
        assert_eq!(
            indexed_visible, reference_visible,
            "visible binding differed for {:?} at {} in {source}",
            token.text, token.start,
        );
        if reference_visible {
            continue;
        }
        assert_eq!(
            bindings.name_is_bound_as_non_enclosing_function_local(index, token.text),
            super::scope::name_is_bound_as_non_enclosing_function_local(
                &tokens,
                &matching_close,
                index,
                token.text,
            ),
            "non-enclosing binding differed for {:?} at {} in {source}",
            token.text,
            token.start,
        );
    }
}

#[test]
fn generated_binding_scope_index_matches_the_reference_scope_model() {
    for source in [
        "let m=1;const top=x=>x+m;function outer(a,b=m){var v=a;let l=b;function inner(c){return v+l+c}return inner}console.log(top(Math.max(m,2)))",
        "function outer(a){let block=x=>{let y=x+a;return y};let expression=(x,y)=>x+y+a;try{return block(expression(1,2))}catch(error){return error.message}}",
        "var api=(function(root){function first(shared){var local=shared;return ()=>local+root}function second(shared){return first(shared)}return second})(globalThis);export{api}",
        "class Box{constructor(value){this.value=value}map(callback){return new Box(callback(this.value))}};let box=new Box(1);box.map(value=>value+1)",
        "function one(reused){var sibling=1;return reused+sibling}function two(reused){let own=2;return reused+own+sibling}",
        "function generator(seed){var make=function inner(value){var nested=value;return nested+seed};return make}function sibling(nested){return nested}",
    ] {
        assert_generated_binding_index_matches_reference(source);
    }
}

#[test]
fn generated_binding_validation_keeps_ambient_and_nested_scope_behavior() {
    analyze_generated_javascript(
        "let choose=x=>Math.max(x,1);function run(value){try{return choose(value)}catch(error){console.log(error);return undefined}};run(2)",
    )
    .unwrap();
    let source =
        "function left(){let privateValue=1;return privateValue}function right(){return privateValue}";
    let error = analyze_generated_javascript(source).unwrap_err();
    assert_eq!(error.offset(), source.rfind("privateValue").unwrap());
    assert!(error
        .to_string()
        .contains("unresolved generated identifier"));
}

#[test]
fn generated_binding_scope_index_construction_has_a_linear_work_bound() {
    fn indexed_work(functions: usize) -> (usize, usize) {
        let mut source = String::new();
        for function in 0..functions {
            source.push_str(&format!(
                "function f{function}(p{function}){{var v{function}=p{function};let l{function}=v{function};return l{function}}}"
            ));
        }
        let tokens = lex(&source).unwrap();
        let matching_close = super::token::matching_closers(&tokens);
        let bindings = super::scope::GeneratedBindingIndex::new(&tokens, &matching_close);
        (tokens.len(), bindings.construction_token_visits())
    }

    let (small_tokens, small_work) = indexed_work(200);
    let (large_tokens, large_work) = indexed_work(400);
    assert!(
        small_work <= small_tokens * 16,
        "{small_work} for {small_tokens} tokens"
    );
    assert!(
        large_work <= large_tokens * 16,
        "{large_work} for {large_tokens} tokens"
    );
    assert!(
        large_work <= small_work * 2 + 32,
        "doubling a flat generated module grew indexed work from {small_work} to {large_work}"
    );
}

#[test]
fn inlines_bit_or_zero_into_subtract_without_stealing_the_minus() {
    let source = r#"function f(x){if(x){var l=x+2|0;return l-1|0}return 0}console.log(f(5))"#;
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("|0-1"),
        "bitwise |0 must stay grouped before subtract:\n{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "6");
}

#[test]
fn inlines_add_into_multiply_with_grouping() {
    let source = r#"function f(x){if(x){var l=x+1;return l*2}return 0}console.log(f(3))"#;
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("(x+1)*2") || optimized.code.contains("(x+1)*2"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("x+1*2"), "{}", optimized.code);
    assert_eq!(run_javascript(&optimized.code).trim(), "8");
}

#[test]
fn inlines_ternary_into_add_with_grouping() {
    let source = r#"function f(x){if(x){var l=x?2:3;return l+1}return 0}console.log(f(1))"#;
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("(x?2:3)+1") || optimized.code.contains("(x?2:3)+1"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("x?2:3+1"), "{}", optimized.code);
    assert_eq!(run_javascript(&optimized.code).trim(), "3");
}

#[test]
fn folds_while_true_unit_increment_into_for() {
    let source = "function scan(n){var s=-1,c=0;while(!0){s=s+1|0;if(s>=n)break;c=c+1|0}return c}console.log(scan(3))";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(optimized.code.contains("++s<n"), "{}", optimized.code);
    assert!(!optimized.code.contains("while(!0)"), "{}", optimized.code);
    assert!(!optimized.code.contains("s=s+1|0"), "{}", optimized.code);
    assert_eq!(run_javascript(&optimized.code).trim(), "3");

    let exclusive = optimize_generated_javascript(
        "function scan(n){var s=-1,c=0;for(;!0;){s++;if(s>n)break;c++}return c}console.log(scan(2))",
    )
    .unwrap();
    assert!(exclusive.code.contains("++s<=n"), "{}", exclusive.code);
    assert_eq!(run_javascript(&exclusive.code).trim(), "3");
}

#[test]
fn folds_int32_member_counters_to_postfix_updates() {
    let source = "let B=e=>+e|0;let D={inBatch:3};D.inBatch=B(D.inBatch)+1|0;D.inBatch=B(D.inBatch)-1|0;console.log(D.inBatch)";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(folded.0.contains("D.inBatch++"), "{}", folded.0);
    assert!(folded.0.contains("D.inBatch--"), "{}", folded.0);
    assert!(!folded.0.contains("B(D.inBatch)"), "{}", folded.0);
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "3");
}

#[test]
fn folds_int32_index_temps_into_postfix_member_indexes() {
    let source = "let B=e=>+e|0;function track(e,t){var r=t.runId_;r===e.lastAccessedBy_||(e.lastAccessedBy_=r,r=B(t.unboundDepsCount_),t.newObserving_[r]=e,t.unboundDepsCount_=r+1|0);return t.unboundDepsCount_}let obs={lastAccessedBy_:0};let der={runId_:1,unboundDepsCount_:1,newObserving_:[0,0,0]};console.log([track(obs,der),der.newObserving_[1]===obs].join(\",\"))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(
        folded.0.contains("t.newObserving_[t.unboundDepsCount_++]")
            || folded.0.contains("newObserving_[t.unboundDepsCount_++]"),
        "{}",
        folded.0
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "2,true");
}

#[test]
fn postfix_index_fold_terminates_declaration_before_member_assignment() {
    let source = "function dispatch(queue,event){var index=0;var item=queue[index];index++,event.currentTarget=item.elem;return event.currentTarget}console.log(dispatch([{elem:7}],{}))";
    let (folded, rewrites) = super::folds::fold_index_postfix_updates(source).unwrap();
    assert_eq!(rewrites, 1, "{folded}");
    assert!(
        folded.contains("var item=queue[index++];event.currentTarget=item.elem"),
        "{folded}"
    );
    assert!(!folded.contains("index++],event.currentTarget"), "{folded}");
    assert_eq!(run_javascript(&folded).trim(), "7");

    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(run_javascript(&optimized.code).trim(), "7");
}

#[test]
fn keeps_int32_index_temps_that_are_read_after_the_store() {
    let source = "let B=e=>+e|0;function track(e,t){var r=B(t.unboundDepsCount_);t.newObserving_[r]=e;t.unboundDepsCount_=r+1|0;return r}let obs={};let der={unboundDepsCount_:4,newObserving_:[0,0,0,0,0]};console.log(track(obs,der))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(!folded.0.contains("unboundDepsCount_++"), "{}", folded.0);
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(run_javascript(&optimized.code).trim(), "4");
}

#[test]
fn drops_int32_coercions_that_are_already_bitwise() {
    let source = "let B=e=>+e|0;let o={flags_:2};function get(r){return 0!=(B(this.flags_)&r)}function set(t,r){var n=B(this.flags_);t?this.flags_=n|r:this.flags_=n&(r^-1)}set.call(o,!0,1);console.log([get.call(o,1),o.flags_].join(\",\"))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(!folded.0.contains("B(this.flags_)"), "{}", folded.0);
    assert!(
        folded.0.contains("this.flags_&r") || folded.0.contains("this.flags_&"),
        "{}",
        folded.0
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "true,3");
}

#[test]
fn folds_int32_decrement_temps_into_prefix_updates() {
    let source = "let B=e=>+e|0;let D={inBatch:2};function end(){var e=B(D.inBatch)-1|0;D.inBatch=e;if(0==e)return \"flush\";return e}console.log([end(),D.inBatch,end()].join(\",\"))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(folded.0.contains("--D.inBatch"), "{}", folded.0);
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "1,1,flush");
}

#[test]
fn folds_length_int32_helpers_to_member_access() {
    let source = "let F=e=>+e.length|0;let a=[1,2,3];function n(x){var i=0,s=0;while(i<F(x))s+=x[i],i=i+1|0;return s}console.log([F(a),n(a)].join(\",\"))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(
        folded.0.contains("a.length") || folded.0.contains("x.length"),
        "{}",
        folded.0
    );
    assert!(!folded.0.contains("F(a)"), "{}", folded.0);
    assert!(!folded.0.contains("F(x)"), "{}", folded.0);
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "3,6");
}

#[test]
fn grouped_integer_length_fold_cannot_form_postfix_increment_tokens() {
    let source = "let K=[1,2,3],I=[0,0],i=I.length,c=1;I.length=(i+(+K.length|0)|0)-c|0;I[(i+(+K.length|0)|0)+c|0]=7;console.log([I.length,I[6]].join(','))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(!folded.0.contains("++K.length"), "{}", folded.0);
    assert_eq!(
        run_javascript(&folded.0).trim(),
        run_javascript(source).trim(),
        "{}",
        folded.0
    );
}

#[test]
fn folds_int32_unit_updates_to_increment() {
    let source = "let n=1,o={N:3};n=n+1|0;o.N=o.N+1|0;console.log([n,o.N].join(\",\"))";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(optimized.code.contains("n++"), "{}", optimized.code);
    assert!(optimized.code.contains("o.N++"), "{}", optimized.code);
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "2,4");
}

#[test]
fn second_shape_pass_recovers_defaults_and_flag_accessors() {
    let source = r#"function V(t,e,r){m.defineProperty(t,e,r)}var m=Object;function Z(t,e,r){V(t,e,{configurable:!0,get:function(){return 0!=(+this.y&r)},set:function(e){var n=+this.y|0;e?this.y=n|r:this.y=n&(r^-1)}})}class q{constructor(e){var r=arguments.length>0&&e!==void 0?e+"":"Atom";this.e=r,this.y=0}}t=q.prototype;;ct("Atom",q),t=q.prototype,Z(t,"isBeingObserved",1);var br=(0,function(){return this})"#;
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("constructor(e=\"Atom\")")
            || optimized.code.contains("constructor(e=\"Atom\""),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("get isBeingObserved(){"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("Z(t,\""), "{}", optimized.code);
    assert!(optimized.code.contains("(0,function"), "{}", optimized.code);
    assert!(
        !optimized.code.contains("arguments.length>0"),
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_replaceable_call_on_same_receiver_methods() {
    let source = concat!(
        "let array=function(e,n){return[e,n]};",
        "array.call=function(){return'custom-call'};",
        "let y={array:array};console.log(y.array.call(y,[1],2))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("y.array.call(y"),
        "{}",
        optimized.code
    );
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
}

#[test]
fn preserves_empty_object_function_assignment_semantics() {
    let source = concat!(
        "var calls=0,saved;",
        "Object.defineProperty(Object.prototype,'has',{configurable:true,set:function(v){calls++,saved=v}});",
        "try{var ut={};ut.has=function(e,r){return e.has(r)}}finally{delete Object.prototype.has}",
        "console.log(JSON.stringify([calls,Object.hasOwn(ut,'has'),saved.name,",
        "Object.hasOwn(saved,'prototype'),typeof Reflect.construct(Object,[],saved)]))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(
        optimized.code.contains(".has=function"),
        "{}",
        optimized.code
    );
    assert_eq!(
        run_javascript(source).trim(),
        "[1,false,\"\",true,\"object\"]"
    );
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "[1,false,\"\",true,\"object\"]",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_pushes_observable_through_inherited_array_setters() {
    let source = concat!(
        "var trace='',own,len;",
        "Object.defineProperty(Array.prototype,'0',{configurable:true,set:function(v){trace+='0@'+this.length+';'}});",
        "Object.defineProperty(Array.prototype,'1',{configurable:true,set:function(v){trace+='1@'+this.length+';'}});",
        "try{function fill(){let r=[];r.push('a'),r.push('b');return r}",
        "let result=fill();own=Object.hasOwn(result,0);len=result.length}",
        "finally{delete Array.prototype[0];delete Array.prototype[1]}",
        "console.log(trace+'|'+own+'|'+len)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(
        optimized.code.matches(".push(").count() >= 2,
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(source).trim(), "0@0;1@1;|false|2");
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "0@0;1@1;|false|2",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_grouped_zero_function_name_suppression() {
    let source = concat!(
        "var wrapped=(0,function(){});",
        "console.log(JSON.stringify([wrapped.name,Object.hasOwn(wrapped,'prototype'),",
        "typeof Reflect.construct(Object,[],wrapped)]))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(optimized.code.contains("(0,function"), "{}", optimized.code);
    assert_eq!(run_javascript(source).trim(), "[\"\",true,\"object\"]");
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "[\"\",true,\"object\"]",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_object_assign_rhs_before_target_setters() {
    let source = concat!(
        "var order='',target={};",
        "Object.defineProperty(target,'a',{configurable:true,set:function(v){order+='set-a;'}});",
        "function rhs(v){order+='rhs-'+v+';';return v}",
        "Object.assign(target,{a:rhs('a'),b:rhs('b')});console.log(order)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(
        optimized.code.contains("Object.assign"),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(source).trim(), "rhs-a;rhs-b;set-a;");
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "rhs-a;rhs-b;set-a;",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_member_getter_setter_and_proxy_observability() {
    let source = concat!(
        "var setterOrder='',reads=0,trace='';",
        "var target={};",
        "Object.defineProperty(target,'a',{set:function(v){setterOrder+='a;'}});",
        "Object.defineProperty(target,'b',{set:function(v){setterOrder+='b;'}});",
        "target.a=1;target.b=1;",
        "var observed={get value(){reads++;return reads}};",
        "function cached(){var first=observed.value;return first+':'+observed.value}",
        "function dead(){var temp=observed.value;temp=0;return temp}",
        "var indexed=new Proxy({0:7},{get:function(t,k,r){if(k==='0')trace+='index;';return Reflect.get(t,k,r)}});",
        "function use(v){trace+='use-'+v+';'}",
        "function moved(){var item=indexed[0];trace+='middle;';use(item)}",
        "var holder={get value(){trace+='member;';return 8}};",
        "function movedMember(){var item=holder.value;trace+='between;';use(item)}",
        "var callTrace='',input={get value(){callTrace+='arg;';return 9}},",
        "receiver={get invoke(){callTrace+='callee;';return function(v){callTrace+='call-'+v+';'}}};",
        "function adjacent(){var value=input.value;receiver.invoke(value)}",
        "var aliasSets=0,aliased={},aliasTarget={};",
        "Object.defineProperty(Object.prototype,'alias',{configurable:true,set:function(){aliasSets++}});",
        "function aliasWrite(){try{aliasTarget.alias=aliased;aliased.kept=5}",
        "finally{delete Object.prototype.alias}}",
        "var windowReads=0;globalThis.window=globalThis;",
        "Object.defineProperty(window,'cachedValue',{configurable:true,get:function(){windowReads++;return 6}});",
        "function windowSnapshot(){var snapshot=window.cachedValue;return snapshot+snapshot}",
        "var outer={old:true},replacement={};",
        "function outerWrite(){outer=replacement;outer.kept=7}",
        "var lengths=0,list={get length(){lengths++;return 2}};",
        "function loop(){var saved=list.length,total=0;for(var i=0;i<list.length;i++)total+=i;return saved+':'+total}",
        "console.log(setterOrder);console.log(cached());dead();console.log(reads);",
        "moved();movedMember();console.log(trace);adjacent();console.log(callTrace);",
        "aliasWrite();console.log(aliasSets+':'+aliased.kept+':'+Object.hasOwn(aliasTarget,'alias'));",
        "console.log(windowSnapshot()+':'+windowReads);delete window.cachedValue;",
        "outerWrite();console.log((outer===replacement)+':'+replacement.kept);",
        "console.log(loop());console.log(lengths)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    let expected = concat!(
        "a;b;\n1:2\n3\nindex;middle;use-7;member;between;use-8;\n",
        "arg;callee;call-9;\n1:5:false\n12:1\ntrue:7\n2:1\n4"
    );

    assert_eq!(run_javascript(source).trim(), expected);
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        expected,
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_single_use_call_result_evaluation_point() {
    let source = concat!(
        "var order='';",
        "function read(){order+='read;';return 1}",
        "function mutate(){order+='mutate;'}",
        "function probe(){var value=read();mutate();return value}",
        "console.log(probe()+'|'+order)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert_eq!(run_javascript(source).trim(), "1|read;mutate;");
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "1|read;mutate;",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_while_push_array_construction() {
    let source = "function pi(x){return x+1}var r=[1,2,3],n=[],t=0;while(t<r.length)n.push(pi(r[t])),t++;console.log(n.join(\",\"))";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(optimized.code.contains(".push("), "{}", optimized.code);
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "2,3,4");
}

#[test]
fn folds_arguments_indexes_on_assigned_and_method_functions() {
    let assigned = "function $(t){B[t]=function(){var r=this,i=r[b];var a;a=i.A(i.r),i=arguments[0];var n;arguments.length>1&&(n=arguments[1]);return a[t](n)}}";
    let assigned_out = optimize_generated_javascript(assigned).unwrap();
    assert!(
        !assigned_out.code.contains("arguments[0]") && !assigned_out.code.contains("arguments[1]"),
        "{}",
        assigned_out.code
    );

    let method = "class C{s(){var t,n=arguments[0],i=arguments[1];return n+i}}";
    let method_out = optimize_generated_javascript(method).unwrap();
    assert!(
        !method_out.code.contains("arguments[0]"),
        "{}",
        method_out.code
    );
    assert!(
        method_out.code.contains("s(") && method_out.code.contains("n"),
        "{}",
        method_out.code
    );
}

#[test]
fn folds_arguments_length_guard_with_default_formals() {
    let source = "class C{S(l=0,d,E){var i;arguments.length>2&&(i=E);return i}}";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("arguments.length"),
        "{}",
        optimized.code
    );
}

#[test]
fn folds_arguments_length_guard_into_formal() {
    let source = "function f(r,i,o,s){var t,e;arguments.length>2&&(t=o),arguments.length>3&&(e=s);return [t,e].join(\",\")}console.log([f(1,2),f(1,2,3),f(1,2,3,4)].join(\"|\"))";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("arguments.length"),
        "{}",
        optimized.code
    );
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
}

#[test]
fn folds_dead_pure_prototype_aliases() {
    let source = "class C{constructor(){this.a=1}m(){return this.a}}t=C.prototype;t=C.prototype;t=C.prototype;var x=new C();console.log(x.m())";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.matches("t=C.prototype").count() <= 1,
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "1");

    let comma = "class C{constructor(){this.a=1}m(){return this.a}}t=C.prototype,t=C.prototype,t=C.prototype;var x=new C();console.log(x.m())";
    let comma_out = optimize_generated_javascript(comma).unwrap();
    assert!(
        comma_out.code.matches("t=C.prototype").count() <= 1,
        "{}",
        comma_out.code
    );
    assert_eq!(run_javascript(&comma_out.code).trim(), "1");

    let guarded = "var t;t=C.prototype;if(flag)t=C.prototype;use(t)";
    assert_eq!(
        optimize_generated_javascript(guarded)
            .unwrap()
            .code
            .matches("t=C.prototype")
            .count(),
        2,
        "{}",
        optimize_generated_javascript(guarded).unwrap().code
    );

    let delayed = "function ct(a,b){}class C{constructor(){this.a=1}m(){return this.a}}t=C.prototype;var o=Symbol.toPrimitive;ct(\"ComputedValue\",C),t=C.prototype;var x=new C();console.log(x.m())";
    let delayed_out = optimize_generated_javascript(delayed).unwrap();
    assert_eq!(
        delayed_out.code.matches("t=C.prototype").count(),
        0,
        "{}",
        delayed_out.code
    );
    assert_eq!(run_javascript(&delayed_out.code).trim(), "1");

    let across_function = "class I{constructor(){this.e=1}}class M{constructor(){this.e=2}}la=function(n,c){return c};la(\"Atom\",I),a=I.prototype;var be=function(e){return e};la(\"Reaction\",M),a=M.prototype;console.log(a===M.prototype,be(3))";
    let across_out = optimize_generated_javascript(across_function).unwrap();
    assert_eq!(
        across_out.code.matches("a=I.prototype").count(),
        0,
        "{}",
        across_out.code
    );
    assert_eq!(run_javascript(&across_out.code).trim(), "true 3");

    let shadowed_param = "class I{constructor(e){this.e=e}}la=function(n,c){c.prototype.x=!0;return c};la(\"Atom\",I),a=I.prototype;var be=function(e,t,r){return e+t+r};console.log(be(1,2,3),be(4,5,6),new I(9).e)";
    let shadowed_out = optimize_generated_javascript(shadowed_param).unwrap();
    assert!(
        !shadowed_out.code.contains("a=I.prototype"),
        "{}",
        shadowed_out.code
    );
    assert_eq!(run_javascript(&shadowed_out.code).trim(), "6 15 9");

    let comma_run = "class C{constructor(){this.a=1}m(){return this.a}}X=function(n,c){return c};wi=Symbol.toStringTag,mi=C.prototype,mi=C.prototype,mi=C.prototype,lt=X('ObservableSet',C);var x=new C();console.log(x.m(),typeof lt,String(wi))";
    let comma_run_out = optimize_generated_javascript(comma_run).unwrap();
    assert!(
        !comma_run_out.code.contains("prototypemi")
            && !comma_run_out.code.contains("prototypelt")
            && !comma_run_out.code.contains("toStringTagmi")
            && !comma_run_out.code.contains(")lt="),
        "{}",
        comma_run_out.code
    );
    assert_eq!(
        run_javascript(&comma_run_out.code).trim(),
        "1 function Symbol(Symbol.toStringTag)"
    );

    let leftover_has = "class p{constructor(){this.l=1}H(t,r){x(this.l)}}t=p.prototype,t.has=function(t,r,n){n=!!n;try{c();var Z=this.C(t);if(!Z)return Z}finally{f()}return!0}";
    let leftover_has_out = optimize_generated_javascript(leftover_has).unwrap();
    assert!(
        leftover_has_out.code.contains("has(t,r,n){") && !leftover_has_out.code.contains("t.has("),
        "{}",
        leftover_has_out.code
    );

    let reassigned_after_call = "class h{constructor(){this.a=1}}X=function(n,c){return c};X('ComputedValue',h),di=h.prototype,di=function(t,n){return t+n};console.log(di(2,3),new h().a)";
    let reassigned_out = optimize_generated_javascript(reassigned_after_call).unwrap();
    assert!(
        !reassigned_out.code.contains(")di=") && !reassigned_out.code.contains("prototype)di"),
        "{}",
        reassigned_out.code
    );
    assert_eq!(run_javascript(&reassigned_out.code).trim(), "5 1");

    let overwritten_after_if =
        "class F{constructor(){this.a=1}}e=F.prototype;if(0)foo();e=2;console.log(e)";
    let overwritten_out = optimize_generated_javascript(overwritten_after_if).unwrap();
    assert!(
        !overwritten_out.code.contains("e=F.prototype"),
        "{}",
        overwritten_out.code
    );
    assert_eq!(run_javascript(&overwritten_out.code).trim(), "2");

    let read_after_if =
        "class F{constructor(){this.a=1}}e=F.prototype;if(0)other();console.log(e===F.prototype)";
    let read_out = optimize_generated_javascript(read_after_if).unwrap();
    assert!(
        read_out.code.contains("e=F.prototype") || read_out.code.contains("F.prototype"),
        "{}",
        read_out.code
    );
    assert_eq!(run_javascript(&read_out.code).trim(), "true");

    let unused_var_alias =
        "class C{constructor(){this.a=1}}var r=C.prototype;console.log(new C().a)";
    let unused_var_out = optimize_generated_javascript(unused_var_alias).unwrap();
    assert!(
        !unused_var_out.code.contains("r=C.prototype"),
        "{}",
        unused_var_out.code
    );
    assert_eq!(run_javascript(&unused_var_out.code).trim(), "1");

    let nested_param_shadow =
        "class U{constructor(){this.e=1}la(){return this.e}}a=U.prototype,sb=function(n,c){return c};sb(\"Adm\",U);var oa=function(a,b){return a};aa={has:function(a,b){return a[b]}};console.log(oa(3,4),aa.has({x:9},\"x\"),new U().la())";
    let nested_out = optimize_generated_javascript(nested_param_shadow).unwrap();
    assert!(
        !nested_out.code.contains("a=U.prototype"),
        "{}",
        nested_out.code
    );
    assert_eq!(run_javascript(&nested_out.code).trim(), "3 9 1");

    let closure_keeps_alias =
        "class I{constructor(){this.e=1}}a=I.prototype;var be=function(){return a};console.log(be()===I.prototype,new I().e)";
    let closure_out = optimize_generated_javascript(closure_keeps_alias).unwrap();
    assert!(
        closure_out.code.contains("a=I.prototype") || closure_out.code.contains("I.prototype"),
        "{}",
        closure_out.code
    );
    assert_eq!(run_javascript(&closure_out.code).trim(), "true 1");

    let unread_symbol_temp =
        "class M{constructor(){this.n=1}[Symbol.iterator](){return this}get [Symbol.toStringTag](){return\"Map\"}}ve=Symbol.iterator,ve=Symbol.toStringTag,pb=function(n,c){return n};pb(\"ObservableMap\",M);ve=function(c,w){return c+w};console.log(ve(1,2),new M().n)";
    let symbol_out = optimize_generated_javascript(unread_symbol_temp).unwrap();
    assert!(
        !symbol_out.code.contains("ve=Symbol."),
        "{}",
        symbol_out.code
    );
    assert_eq!(run_javascript(&symbol_out.code).trim(), "3 1");

    let parent_alias_used_in_set_proto = concat!(
        "let Z=()=>globalThis.Object;",
        "var a=globalThis.Symbol.toPrimitive,h;",
        "class oa{constructor(g){this.g=g}}",
        "class ea extends oa{constructor(e){super(e),this.e=e}}",
        "h=ea.prototype;a=oa.prototype;Z().setPrototypeOf(h,a);",
        "console.log(typeof a)"
    );
    let (unread, _) =
        crate::js_peephole::fold_unread_prototype_aliases(parent_alias_used_in_set_proto).unwrap();
    assert!(
        unread.contains("a=oa.prototype"),
        "setPrototypeOf(h,a) reads the parent alias: {unread}"
    );
    let factory_out = optimize_generated_javascript(parent_alias_used_in_set_proto).unwrap();
    assert!(
        factory_out.code.contains("a=oa.prototype")
            || factory_out.code.contains("setPrototypeOf(h,oa.prototype)")
            || !factory_out.code.contains("setPrototypeOf"),
        "{}",
        factory_out.code
    );
}

#[test]
fn preserves_copied_method_snapshot_and_replaceable_call() {
    let source = concat!(
        "var reads=0,method=function(r){return this.tag+':'+r};",
        "method.call=function(receiver,r){return'custom:'+receiver.tag+':'+r};",
        "var child={tag:'ok',x:method},o={get b(){reads++;return child}};",
        "function has(e,r){let t=e.b.x;return t.call(e.b,r)}",
        "console.log(has(o,3)+'|'+reads)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        // The snapshot may be folded into its only use when nothing runs in
        // between, which cannot change what `e.b.x` yields. What must survive
        // is the call shape and the receiver.
        optimized.code.contains("e.b.x") && optimized.code.contains("call(e.b,r)"),
        "{}",
        optimized.code
    );

    // With a statement in between the snapshot has to stay: `side()` could
    // replace `e.b.x` before the call.
    let guarded = source.replace(
        "let t=e.b.x;return t.call(e.b,r)",
        "let t=e.b.x;side();return t.call(e.b,r)",
    );
    let guarded = guarded.replace(
        "var reads=0,",
        "var reads=0,side=function(){child.x=function(){return'replaced'}},",
    );
    let guarded_out = optimize_generated_javascript(&guarded).unwrap();
    assert!(
        guarded_out.code.contains("t=e.b.x") && guarded_out.code.contains("t.call(e.b,r)"),
        "an intervening statement must keep the snapshot: {}",
        guarded_out.code
    );
    assert_eq!(
        run_javascript(&guarded_out.code).trim(),
        run_javascript(&guarded).trim()
    );
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
}

#[test]
fn folds_pooled_has_predicates_to_method_calls() {
    let source = "let ze=(e,t)=>!!e.has(t);let m=new Map([[\"a\",1]]);function set(k,v){var r=ze(m,k);r?m.set(k,v):m.set(k,v);return r}console.log([set(\"a\",2),ze(m,\"b\")].join(\",\"))";
    let folded = super::folds::fold_int32_coercions(source).unwrap();
    assert!(folded.0.contains("m.has("), "{}", folded.0);
    assert!(!folded.0.contains("ze(m"), "{}", folded.0);
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        run_javascript(source).trim()
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "true,false");
}

#[test]
fn declares_expression_embedded_implicit_locals() {
    let source = r#"class O{get(){S(this),le(this)&&(e=o.trackingContext,this.U&&!e&&(o.trackingContext=this),o.trackingContext=e);return this.c}}"#;
    let (out, count) = super::folds::declare_implicit_assignment_bindings(source).unwrap();
    assert!(count >= 1, "{out}");
    assert!(out.contains("var e"), "{out}");
    assert!(out.contains("get(){var e"), "{out}");
}

#[test]
fn declares_top_level_implicit_export_bindings() {
    let source = r#"An=function(e){return e};export{An as flowResult}"#;
    let (out, count) = super::folds::declare_implicit_assignment_bindings(source).unwrap();
    assert!(count >= 1, "{out}");
    assert!(out.contains("var An"), "{out}");
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("var An")
            || optimized.code.contains("let An")
            || optimized.code.contains("const An")
            || optimized.code.contains("function An"),
        "{}",
        optimized.code
    );
}

#[test]
fn implicit_binding_index_preserves_outer_captures_and_parameter_shadowing() {
    let captured =
        "function outer(){var shared=0;return function update(){shared=1;return shared}}";
    let (captured_out, captured_rewrites) =
        super::folds::declare_implicit_assignment_bindings(captured).unwrap();
    assert_eq!(captured_rewrites, 0, "{captured_out}");
    assert_eq!(captured_out, captured);

    let shadowed =
        "function outer(){var shared=0;return function update(shared){shared=1;return shared}}";
    let (shadowed_out, shadowed_rewrites) =
        super::folds::declare_implicit_assignment_bindings(shadowed).unwrap();
    assert_eq!(shadowed_rewrites, 0, "{shadowed_out}");
    assert_eq!(shadowed_out, shadowed);
}

#[test]
fn implicit_binding_index_localizes_sibling_and_unbound_assignments() {
    let sibling = "function left(){var temporary=1;return temporary}function right(){temporary=2;return temporary}";
    let (sibling_out, sibling_rewrites) =
        super::folds::declare_implicit_assignment_bindings(sibling).unwrap();
    assert_eq!(sibling_rewrites, 1, "{sibling_out}");
    assert!(
        sibling_out.contains("function right(){var temporary;temporary=2"),
        "{sibling_out}"
    );
    analyze_generated_javascript(&sibling_out).unwrap();

    let unbound = "function create(){missing=1;return missing}";
    let (unbound_out, unbound_rewrites) =
        super::folds::declare_implicit_assignment_bindings(unbound).unwrap();
    assert_eq!(unbound_rewrites, 1, "{unbound_out}");
    assert!(
        unbound_out.contains("function create(){var missing;missing=1"),
        "{unbound_out}"
    );
    analyze_generated_javascript(&unbound_out).unwrap();
}

#[test]
fn does_not_declare_parameter_defaults_as_implicit_bindings() {
    let class_source = "class ErrorPropertiesBuilder{constructor(c,d,j=[]){this.modifiers=j}buildFromUnknown(j,m={}){return m}}";
    let (class_out, class_rewrites) =
        super::folds::declare_implicit_assignment_bindings(class_source).unwrap();
    assert_eq!(class_rewrites, 0, "{class_out}");
    assert_eq!(class_out, class_source);
    analyze_generated_javascript(&class_out).unwrap();

    let first_param = "function create(j=[]){return j}";
    let (first_out, first_rewrites) =
        super::folds::declare_implicit_assignment_bindings(first_param).unwrap();
    assert_eq!(first_rewrites, 0, "{first_out}");
    assert_eq!(first_out, first_param);

    let assignment_in_if = "function run(x){if(y=x)return y;return 0}";
    let (if_out, if_rewrites) =
        super::folds::declare_implicit_assignment_bindings(assignment_in_if).unwrap();
    assert_eq!(if_rewrites, 1, "{if_out}");
    assert!(if_out.contains("var y"), "{if_out}");
}

#[test]
fn moves_single_use_function_to_its_capture_safe_call() {
    let source = "var g=2;function helper(e){return e+g}function run(t){return helper(t)}console.log(run(5))";
    let (folded, count) = super::folds::fold_single_use_function_expressions(source).unwrap();
    assert_eq!(count, 2, "{folded}");
    assert!(!folded.contains("function helper"), "{folded}");
    assert!(folded.contains("(function(e){return e+g})(t)"), "{folded}");
    assert_eq!(run_javascript(&folded), run_javascript(source));
}

#[test]
fn keeps_single_use_async_function_declarations_as_async() {
    for source in [
        "async function helper(){return 1}helper().then(value=>console.log(value))",
        "async function helper(){return await Promise.resolve(2)}helper().then(value=>console.log(value))",
    ] {
        let (folded, count) =
            super::folds::fold_single_use_function_expressions(source).unwrap();
        assert_eq!(count, 0, "{folded}");
        assert_eq!(folded, source);

        let optimized = optimize_generated_javascript(source).unwrap();
        assert!(optimized.code.contains("async"), "{}", optimized.code);
        assert_eq!(run_javascript(&optimized.code), run_javascript(source));
    }
}

#[test]
fn moves_whole_single_use_async_arrow_values_to_their_calls() {
    for source in [
        "var helper=async()=>1;helper().then(value=>console.log(value))",
        "var helper=async value=>{return await Promise.resolve(value+1)};helper(4).then(value=>console.log(value))",
    ] {
        let (folded, count) = super::folds::fold_single_use_function_values(source).unwrap();
        assert_eq!(count, 2, "{folded}");
        assert!(!folded.contains("var helper"), "{folded}");
        assert!(folded.contains("(async"), "{folded}");
        assert_eq!(run_javascript(&folded), run_javascript(source));

        let optimized = optimize_generated_javascript(source).unwrap();
        assert!(optimized.code.contains("async"), "{}", optimized.code);
        assert!(!optimized.code.contains("var helper"), "{}", optimized.code);
        assert_eq!(run_javascript(&optimized.code), run_javascript(source));
    }
}

#[test]
fn keeps_async_arrow_grammar_ambiguities_out_of_modifier_folding() {
    let ordinary_parameter = "var helper=async=>async+1;console.log(helper(2))";
    let (folded, count) =
        super::folds::fold_single_use_function_values(ordinary_parameter).unwrap();
    assert_eq!(count, 0, "{folded}");
    assert_eq!(folded, ordinary_parameter);
    assert_eq!(run_javascript(&folded), "3\n");

    for source in [
        "var helper=async\n value=>value;helper(2)",
        "var helper=async/*\n*/()=>1;helper()",
    ] {
        let (folded, count) = super::folds::fold_single_use_function_values(source).unwrap();
        assert_eq!(count, 0, "{folded}");
        assert_eq!(folded, source);
    }
}

#[test]
fn keeps_single_use_function_when_moving_would_capture_a_caller_local() {
    let source =
        "var g=2;function helper(e){return e+g}function run(g){return helper(1)}console.log(run(5))";
    let (folded, count) = super::folds::fold_single_use_function_expressions(source).unwrap();
    assert_eq!(count, 0, "{folded}");
    assert_eq!(folded, source);
}

#[test]
fn keeps_single_use_arrow_when_moving_would_shadow_a_module_callee() {
    let source = concat!(
        "let t=e=>({id:e});",
        "let find=n=>t(n);",
        "function me(n){var t;t=find(n);return t.id}",
        "console.log(me(7), t(1).id)",
    );
    let (folded, count) = super::folds::fold_single_use_function_values(source).unwrap();
    assert_eq!(count, 0, "{folded}");
    assert_eq!(run_javascript(&folded).trim(), "7 1");
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "7 1",
        "{}",
        optimized.code
    );
}

#[test]
fn moves_statement_assignment_into_its_first_inertly_prefixed_read() {
    let source = "function trim(e){e=e.trim();return e.endsWith('/')?e.slice(0,e.length-1):e}console.log(trim('x/'))";
    let (folded, count) = super::folds::fold_statement_assignments_into_first_use(source).unwrap();
    assert!(count >= 2, "{folded}");
    assert!(!folded.contains("e=e.trim();return"), "{folded}");
    assert!(folded.contains("(e=e.trim()).endsWith"), "{folded}");
    assert_eq!(run_javascript(&folded), run_javascript(source));
}

#[test]
fn keeps_statement_assignment_before_an_effectful_prefix() {
    let source = "function run(){x=read();tick(),use(x)}";
    let (folded, count) = super::folds::fold_statement_assignments_into_first_use(source).unwrap();
    assert_eq!(count, 0, "{folded}");
    assert_eq!(folded, source);
}

#[test]
fn keeps_comma_expression_assignments_out_of_statement_return_folding() {
    let source = "function run(c){c?(x=1):(x=2),x=3;return x}console.log(run(true))";
    let (folded, count) = super::folds::fold_statement_assignments_into_first_use(source).unwrap();
    assert_eq!(count, 0, "{folded}");
    assert_eq!(folded, source);
    assert_eq!(run_javascript(&folded), run_javascript(source));
}

#[test]
fn swaps_bindings_across_a_nested_function_tree_without_touching_globals() {
    let source =
        "var g=7;function run(e,t){return(function(e){return e+t+g})(e)}console.log(run(2,3))";
    let variants = super::function_local_binding_swap_variants(source).unwrap();
    assert!(!variants.is_empty());
    for variant in variants {
        assert!(variant.contains("+g"), "{variant}");
        assert_eq!(
            run_javascript(&variant),
            run_javascript(source),
            "{variant}"
        );
    }
}

#[test]
fn strips_unread_identifier_copy_before_original_method_call() {
    let source = concat!(
        "var X=[],n=X;n.push(\"a\");var o=X;X.push(\"b\");var p=X;X.push(\"c\");",
        "function use(){return X.join(\".\")}",
        "console.log(use())"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("var o=") && !optimized.code.contains("var p="),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "a.b.c");
}

#[test]
fn keeps_identifier_copy_that_is_the_method_receiver() {
    let source = "var X=[],n=X;n.push(\"a\");console.log(n[0])";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("n.push") || optimized.code.contains("X.push"),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "a");
}

#[test]
fn strips_ident_copy_when_only_nested_function_reuses_the_name() {
    let source = concat!(
        "var X=[],o=X;X.push(\"a\");",
        "function f(o){return o}",
        "console.log(X[0]+f(\"z\"))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("var o=X") && !optimized.code.contains("var o=X,"),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "az");
}

#[test]
fn keeps_ident_copy_captured_by_nested_function() {
    let source = concat!(
        "var X=[],o=X;X.push(\"a\");",
        "function f(){return o[0]}",
        "console.log(f())"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(run_javascript(&optimized.code).trim(), "a");
}

#[test]
fn strips_ident_copy_when_nested_function_redeclares_the_name() {
    let source = concat!(
        "function wrap(){",
        "var X=[],n=X;n.push(\"address\");",
        "var F=X;X.push(\"dt\");",
        "var A=ea(\"htmlFlow\",(0,function(p,q,r){",
        "var o=this;let F=function(b){return 62===b?F:1};return F",
        "}));return [X,A]}",
        "function ea(n,f){return f}",
        "console.log(wrap()[0].join(\".\"))",
    );
    let (folded, count) =
        crate::js_peephole::fold_dead_identifier_copy_declarators(source).unwrap();
    assert!(count >= 1, "{folded}");
    assert!(!folded.contains("var F=X"), "{folded}");
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("var F=X") && !optimized.code.contains("var F=X,"),
        "{}",
        optimized.code
    );
    assert_eq!(run_javascript(&optimized.code).trim(), "address.dt");
}

#[test]
fn preserving_functions_still_merges_adjacent_declarations() {
    let source = concat!(
        "function helper(x){return x+1}",
        "function use(n){var a=[];var o=n;var i=0;while(i<o){a.push(helper(i));i=i+1}return a}",
    );
    let before = analyze_generated_javascript(source).unwrap();
    let preserved = optimize_generated_javascript_preserving_functions(source).unwrap();
    assert!(
        preserved.metrics.functions >= before.functions,
        "{}",
        preserved.code
    );
    assert!(
        preserved.code.contains("var a=[],i=0") || preserved.code.contains("a=[],i=0"),
        "{}",
        preserved.code
    );
    assert!(
        preserved.code.contains("function helper") || preserved.code.contains("helper="),
        "{}",
        preserved.code
    );
}

/// `fold_early_exit_guards` inverts a guard so the suffix becomes the guarded
/// arm. The inversion has to negate the *condition*, not its first operand.
#[test]
fn an_inverted_guard_negates_the_whole_disjunction() {
    // The condition starts with a parenthesised group that covers only the
    // first operand of `||`. Dropping the parentheses that `!` needs would
    // leave `!(a==null)||typeof a!="object"`, which is true for `a===undefined`
    // and steps into the body the guard exists to skip.
    let (folded, count) = fold_early_exit_guards(
        "var f=a=>{if((a==null)||\"object\"!=typeof a)return;var b=a.start;return b};",
    )
    .expect("fold");
    assert_eq!(count, 1, "{folded}");
    assert!(
        folded.contains("if(!((a==null)||\"object\"!=typeof a))"),
        "{folded}"
    );

    // A condition that is one parenthesised group still negates in place.
    let (grouped, grouped_count) =
        fold_early_exit_guards("var f=a=>{if((a==null))return;var b=a.start;return b};")
            .expect("fold");
    assert_eq!(grouped_count, 1, "{grouped}");
    assert!(grouped.contains("if(!(a==null))"), "{grouped}");
}

/// Folding a guard into `&&` must parenthesise an operand that binds looser.
///
/// The operator list this replaced named plain `=` but none of the compound
/// assignments, so `if(h>0)a+=.25;` folded to `h>0&&a+=.25` — an assignment to
/// an rvalue. It reached katex's declaration variants and cost every parsed
/// peephole rewrite on them, because the whole leaf was then discarded.
#[test]
fn an_and_operand_that_binds_looser_is_grouped() {
    for (source, expected) in [
        ("function f(h,a){if(h>0){a+=.25}}", "h>0&&(a+=.25)"),
        ("function f(h,a){if(h>0){a-=1}}", "h>0&&(a-=1)"),
        ("function f(h,a){if(h>0){a**=2}}", "h>0&&(a**=2)"),
        ("function f(h,a){if(h>0){a>>>=1}}", "h>0&&(a>>>=1)"),
        ("function f(h,a){if(h>0){a??=1}}", "h>0&&(a??=1)"),
    ] {
        let (folded, count) = fold_if_expression_to_and(source).expect("fold");
        assert_eq!(count, 1, "{folded}");
        assert!(
            folded.contains(expected),
            "{expected} missing from {folded}"
        );
    }

    // Operands that bind tighter than `&&` still need no parentheses.
    let (tight, tight_count) =
        fold_if_expression_to_and("function f(h,a){if(h>0){a.push(1)}}").expect("fold");
    assert_eq!(tight_count, 1, "{tight}");
    assert!(tight.contains("h>0&&a.push(1)"), "{tight}");
}

/// Beta-reducing an IIFE drops the parentheses the call expression provided.
///
/// `?:` is right-associative, so a body that is itself a conditional takes the
/// arms that follow instead of standing as the test. Without the group,
/// `(c=>a?b:d)(x)?e:f` became `a?b:d?e:f` — unified's `process` then discarded
/// every string its compiler produced, and the promise never settled.
#[test]
fn a_beta_reduced_conditional_stays_grouped_in_a_ternary_test() {
    let (folded, count) = fold_identity_arrow_iife(
        "var r=(c=>\"string\"===typeof c?!0:g(c))(b)?f.value=b:f.result=b;",
    )
    .expect("fold");
    assert_eq!(count, 1, "{folded}");
    assert!(
        folded.contains("(\"string\"===typeof b?!0:g(b))?f.value=b"),
        "{folded}"
    );

    // A body that already binds tighter than the test needs no parentheses.
    let (tight, tight_count) =
        fold_identity_arrow_iife("var r=(c=>g(c))(b)?f.value=b:f.result=b;").expect("fold");
    assert_eq!(tight_count, 1, "{tight}");
    assert!(tight.contains("g(b)?f.value=b"), "{tight}");
}

#[test]
fn beta_reduce_inlines_an_unused_assignment_iife() {
    let (folded, count) = fold_identity_arrow_iife(
        "var l={href:42},n=l.href??0;(()=>{l={href:47}})(),console.log(n+(l.href??0)|0)",
    )
    .expect("fold");
    assert_eq!(count, 1, "{folded}");
    assert!(folded.contains("l={href:47},console.log"), "{folded}");
    assert!(!folded.contains("(()=>{"), "{folded}");
    assert!(folded.contains("var l={href:42},n=l.href??0"), "{folded}");
}

/// `[...r]` is rest/spread, not `r` as a property of `.`.
///
/// Beta-reduction skipped the operand because the token before it is `.`, the
/// same test used for `obj.r`. `(r=>[...r].length)(a)` then kept `r`, which is
/// whatever binding that name has in the caller — in marked, the still-live
/// `exec` match rather than the delimiter string `points` was given.
#[test]
fn beta_reduce_substitutes_a_spread_operand() {
    let (folded, count) = fold_identity_arrow_iife("a=(r=>[...r].length)(a);").expect("fold");
    assert_eq!(count, 1, "{folded}");
    assert!(folded.contains("[...a].length"), "{folded}");
    assert!(!folded.contains("[...r]"), "{folded}");

    let tokens = lex("[...r];o.r;({...r});f(...r)").expect("lex");
    let positions = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.kind == super::token::TokenKind::Identifier && token.text == "r")
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 4, "{tokens:?}");
    assert!(!super::rewrite::is_property_identifier(
        &tokens,
        positions[0]
    ));
    assert!(super::rewrite::is_property_identifier(
        &tokens,
        positions[1]
    ));
    assert!(!super::rewrite::is_property_identifier(
        &tokens,
        positions[2]
    ));
    assert!(!super::rewrite::is_property_identifier(
        &tokens,
        positions[3]
    ));
}

/// ident-05: after `a=pickDelim(r)`, `points(a)` must spread `a`. If the
/// inlined helper still spells its parameter `r`, failing to substitute
/// `[...r]` reads the live match instead of the delimiter.
#[test]
fn beta_reduce_does_not_leave_code_point_length_bound_to_the_match() {
    let source = "function tok(r){var a=ln(r);if(0!=a.length){a=(r=>[...r].length)(a)}return a}";
    let (folded, count) = fold_identity_arrow_iife(source).expect("fold");
    assert_eq!(count, 1, "{folded}");
    assert!(folded.contains("[...a].length"), "{folded}");
    assert!(!folded.contains("[...r]"), "{folded}");
    let harness = concat!(
        "function ln(m){return (m[3]==null?\"\":m[3])+\"\"}",
        "function tok(r){var a=ln(r);if(0!=a.length){a=(r=>[...r].length)(a)}return a}",
        "console.log(tok(['full',null,null,'**',null,null,null,null]))",
    );
    let optimized = optimize_generated_javascript(harness).unwrap();
    assert_eq!(
        run_javascript(&optimized.code).trim(),
        "2",
        "{}",
        optimized.code
    );
}

#[test]
fn rematerialization_folds_refuse_a_rebound_receiver() {
    let (temps, _) =
        fold_single_use_temporaries("function f(b,i){var d=b.href;return i(b=b.title,d)}")
            .expect("fold");
    assert!(
        temps.contains("var d=b.href") && !temps.contains("i(b=b.title,b.href)"),
        "{temps}"
    );

    let (ifs, _) =
        fold_single_use_if_assigns("function f(b){if(1){var d=b.href;b=b.title;return d}}")
            .expect("fold");
    assert!(
        !ifs.contains("return b.href") && !ifs.contains("return(b.href)"),
        "{ifs}"
    );

    let (copies, _) = fold_identifier_copies("function f(b){d=b;b=1;return d}").expect("fold");
    assert!(
        copies.contains("return d") && !copies.contains("return b"),
        "{copies}"
    );

    let (typeofs, _) =
        fold_typeof_identifier_caches("function f(x){t=typeof x;x=1;return t}").expect("fold");
    assert!(
        typeofs.contains("typeof x") && typeofs.contains("return t"),
        "{typeofs}"
    );
    assert!(!typeofs.contains("return typeof"), "{typeofs}");

    let (moved, _) =
        fold_statement_assignments_into_first_use("function f(b){d=b.href;b=b.title;return d}")
            .expect("fold");
    assert!(
        !moved.contains("return(d=b.href)") && moved.contains("d=b.href"),
        "{moved}"
    );

    let (sequence, _) = fold_sequence_assignments_into_first_use(
        "function f(b,i){return(d=b.href,i(b=b.title,d))}",
    )
    .expect("fold");
    assert!(
        !sequence.contains("i(b=b.title,d=b.href)") && sequence.contains("d=b.href"),
        "{sequence}"
    );
}

#[test]
fn rematerialization_folds_refuse_a_written_property() {
    let (ifs, _) =
        fold_single_use_if_assigns("function f(b,x){if(1){var d=b.href;b.href=x;return d}}")
            .expect("fold");
    assert!(
        ifs.contains("var d=b.href")
            && !ifs.contains("return b.href")
            && !ifs.contains("return(b.href)"),
        "{ifs}"
    );

    let (temps, _) = fold_single_use_temporaries("function f(b,x){var d=b.href;b.href=x;return d}")
        .expect("fold");
    assert!(
        temps.contains("var d=b.href") && !temps.contains("return b.href"),
        "{temps}"
    );

    let (moved, _) =
        fold_statement_assignments_into_first_use("function f(b,x){d=b.href;b.href=x;return d}")
            .expect("fold");
    assert!(
        moved.contains("d=b.href") && !moved.contains("return(d=b.href)"),
        "{moved}"
    );

    let (sequence, _) =
        fold_sequence_assignments_into_first_use("function f(b,x){return(d=b.href,(b.href=x,d))}")
            .expect("fold");
    assert!(
        sequence.contains("d=b.href") && !sequence.contains("d=b.href,d)"),
        "{sequence}"
    );

    let (sibling, _) =
        fold_single_use_if_assigns("function f(b,x){if(1){var d=b.href;b.src=x;return d}}")
            .expect("fold");
    assert!(
        sibling.contains("return b.href") || sibling.contains("return(b.href)"),
        "{sibling}"
    );

    let (copies, _) =
        fold_identifier_copies("function f(b,x){var d=b;b.href=x;return d}").expect("fold");
    assert!(
        copies.contains("return b") && !copies.contains("return d"),
        "{copies}"
    );

    let (iife, _) =
        fold_single_use_temporaries("function f(b,x){var d=b.href;(()=>{b=x})();return d}")
            .expect("fold");
    assert!(
        iife.contains("var d=b.href")
            && !iife.contains("return b.href")
            && !iife.contains("return(b.href)"),
        "{iife}"
    );

    let (named, _) =
        fold_single_use_temporaries("function f(b,x){var d=b.href;var r=()=>{b=x};r();return d}")
            .expect("fold");
    assert!(
        named.contains("var d=b.href")
            && !named.contains("return b.href")
            && !named.contains("return(b.href)"),
        "{named}"
    );

    let (script, _) = fold_single_use_temporaries(
        "var l={href:42};var n=l.href??0;(function(){l={href:47}})(),console.log(n+(l.href??0)|0)",
    )
    .expect("fold");
    assert!(
        script.contains("var n=l.href??0")
            && !script.contains("console.log((l.href??0)+(l.href??0)")
            && !script.contains("console.log(l.href??0+(l.href??0)"),
        "{script}"
    );
}

/// A saved value moves to its first read when nothing before that read can
/// observe it.
///
/// The prefix test used to be a hand-kept list of "inert" tokens that omitted
/// identifiers, `&&` and `?`, so the fold stopped at the first ordinary
/// expression. It now asks whether the prefix can observe anything: bindings,
/// literals and pure operators cannot; a call, a property read, or an
/// assignment can.
#[test]
fn a_saved_value_moves_to_its_first_read_when_the_prefix_is_pure() {
    // Guarded by a comparison against another binding — the old list stopped at
    // the identifier `i`.
    let (guard, guard_count) =
        fold_statement_assignments_into_first_use("f(){s=n+1;if(s>=i)return;g()}").expect("fold");
    assert!(guard_count > 0, "{guard}");
    assert!(guard.contains("if((s=n+1)>=i)"), "{guard}");

    // A property read before the use can run a getter, so the value stays put.
    let (getter, getter_count) =
        fold_statement_assignments_into_first_use("f(){s=n+1;if(o.k>s)return;g()}").expect("fold");
    assert_eq!(getter_count, 0, "{getter}");

    // So can a call.
    let (call, call_count) =
        fold_statement_assignments_into_first_use("f(){s=n+1;if(h()>s)return;g()}").expect("fold");
    assert_eq!(call_count, 0, "{call}");
}

/// An array built by consecutive pushes is the array literal it spells out.
#[test]
fn consecutive_pushes_onto_a_fresh_array_become_a_literal() {
    let (folded, count) =
        fold_fresh_empty_array_pushes("var a;a=[];a.push(1);a.push(f(2));g(a);").expect("fold");
    assert_eq!(count, 1, "{folded}");
    assert!(folded.contains("a=[1,f(2)];g(a)"), "{folded}");

    // Pushes that continue as a comma sequence keep the binding's terminator,
    // so the assignment after them stays an assignment. Splicing the last
    // push's comma in after a declarator would redeclare `f`.
    let (sequence, sequence_count) =
        fold_fresh_empty_array_pushes("let k=[];k.push(x),k.push(y),f={c:k};").expect("fold");
    assert_eq!(sequence_count, 1, "{sequence}");
    assert!(sequence.contains("let k=[x,y];f={c:k}"), "{sequence}");

    // The shape the emitter produces inside an arrow body.
    let (real, real_count) = fold_fresh_empty_array_pushes(
        "let ue=(a,h,b)=>{var c=[];c.push(h);c.push(2);var d=b.length;};",
    )
    .expect("fold");
    assert_eq!(real_count, 1, "{real}");
    assert!(real.contains("var c=[h,2];var d="), "{real}");

    // A `for` header is not a statement list: the pushes run per iteration.
    let (header, header_count) =
        fold_fresh_empty_array_pushes("for(k=[];k.push(1),x;)g();").expect("fold");
    assert_eq!(header_count, 0, "{header}");

    // A pushed value that reads the array itself must not move ahead of it.
    let (self_read, self_count) =
        fold_fresh_empty_array_pushes("c=[];c.push(c.length);").expect("fold");
    assert_eq!(self_count, 0, "{self_read}");

    // The push must be the whole statement: a used result is a different program.
    let (used, used_count) = fold_fresh_empty_array_pushes("d=[];e=d.push(1);").expect("fold");
    assert_eq!(used_count, 0, "{used}");

    // Only a plain binding is known to still hold the fresh array.
    let (member, member_count) =
        fold_fresh_empty_array_pushes("o.a=[];o.a.push(1);").expect("fold");
    assert_eq!(member_count, 0, "{member}");

    // `(T=[],T.push(x))` is one expression: the push ends at the grouping.
    let (grouped, grouped_count) =
        fold_fresh_empty_array_pushes("f=(T=[],T.push(qa(1))),g(f)").expect("fold");
    assert_eq!(grouped_count, 1, "{grouped}");
    assert!(grouped.contains("f=(T=[qa(1)]),g(f)"), "{grouped}");
}
