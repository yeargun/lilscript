use super::{
    analyze_generated_javascript, function_leading_declaration_variant,
    generated_javascript_bit_or_zero_count, generated_javascript_export_names,
    generated_javascript_export_witnesses, generated_javascript_static_imports,
    generated_javascript_static_property_names, late_generated_javascript_cleanup_pass,
    optimize_generated_javascript, optimize_generated_javascript_assuming,
    optimize_generated_javascript_preserving_functions, reorder_uninitialized_var_declarators,
    fold_void_initializers_off_fresh_vars, join_adjacent_declarations, shape_declarations, spell_regexp_literals,
    fold_statement_negated_ors, fold_self_assignment_chains, fold_while_trailing_increments,
    fold_for_trailing_increments, fold_constant_string_concatenations,
    fold_null_normalized_nullable_tests, wrap_module_internals_in_function_scope,
    fold_prefix_increment_for_bounds,
    validate_generated_javascript_syntax_floor, LateJavaScriptCleanupPass, PeepholeResult,
};
use super::folds::{
    fold_dead_pure_identifier_assigns,
    fold_array_literal_borrow_pushes, fold_common_conditional_arms, fold_ident_ternary_to_or, fold_early_exit_guards, fold_fresh_empty_array_pushes,
    absorb_property_writes_into_literals, fold_assigned_truthy_ternaries, fold_fresh_empty_object_assign, fold_identifier_copies, fold_identity_arrow_iife, fold_if_expression_to_and,
    fold_sequence_assignments_into_first_use, fold_single_use_if_assigns,
    fold_single_use_literal_bindings, fold_single_use_temporaries, fold_statement_assignments_into_first_use,
    fold_typeof_identifier_caches,
};
use super::parse::{non_overlapping_parsed_node_count, parse_expression_regions};
use super::token::{lex, punctuation_width};

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
        "class B{base(a){}}function f(a,b=1){}class C extends B{constructor(a,b){}read(a=1){}}let g=(a,b)=>a+b,v=1;export{f,C,g,v}",
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
fn observes_exported_callable_named_with_contextual_of() {
    let witnesses = generated_javascript_export_witnesses(
        "let a=1,of=(value)=>value+a;export{of as advancePositionWithClone}",
    )
    .unwrap();
    assert_eq!(witnesses.len(), 1);
    assert_eq!(witnesses[0].name, "advancePositionWithClone");
    assert_eq!(
        witnesses[0].kind,
        super::GeneratedJavaScriptExportKind::Function
    );
    assert_eq!(witnesses[0].arity, Some(1));
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
fn resolves_reexported_import_aliases() {
    let witnesses = generated_javascript_export_witnesses(
        "import{value as importedValue}from\"pkg\";export{importedValue as value}",
    )
    .unwrap();
    assert_eq!(witnesses[0].name, "value");
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

