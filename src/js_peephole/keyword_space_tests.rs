use super::optimize_generated_javascript;

#[test]
fn drops_separators_a_keyword_does_not_need() {
    let optimized = optimize_generated_javascript(
        "function f(a){if(a)return \"x\";return !1}function g(){return [1,2]}",
    )
    .unwrap();
    // The conditional-return fold also fires here, so the guarded pair
    // collapses; what this asserts is that no `return` keeps a separator it
    // does not need.
    assert_eq!(
        optimized.code,
        "function f(a){return a?\"x\":!1}function g(){return[1,2]}"
    );
}

#[test]
fn keeps_separators_that_a_keyword_still_needs() {
    for source in [
        "function f(a){return a}",
        "function f(){return 5}",
        "function f(){return .5}",
        "function f(a){return typeof a}",
    ] {
        assert_eq!(
            optimize_generated_javascript(source).unwrap().code,
            source,
            "must not fuse: {source}"
        );
    }
}

#[test]
fn elided_output_still_runs_identically() {
    let source = "function f(a){if(a)return -1;return (a,0)}function g(){return /b/.test(\"abc\")}";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(optimized.code.len() < source.len());
    let script = format!(
        "{}\nconsole.log([f(1),f(0),g()].join(\",\"));",
        optimized.code
    );
    let original = format!("{source}\nconsole.log([f(1),f(0),g()].join(\",\"));");
    let run = |code: &str| {
        let path = std::env::temp_dir().join(format!("lil-kw-{}.js", code.len()));
        std::fs::write(&path, code).unwrap();
        let out = std::process::Command::new("node")
            .arg(&path)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    assert_eq!(run(&script), run(&original));
    assert_eq!(run(&script), "-1,0,true");
}

#[test]
fn folds_index_assigns_into_postfix_updates() {
    let assign = optimize_generated_javascript(
        "function m(a,b){var i=a.length,j=0;for(;j<b.length;j++)a[i]=b[j],i++;return a}",
    )
    .unwrap();
    assert!(assign.code.contains("a[i++]=b[j]"), "{}", assign.code);
    assert!(!assign.code.contains(",i++"), "{}", assign.code);

    let read = optimize_generated_javascript(
        "function t(a){var i=0,n,s=\"\";for(;;){n=a[i],i++;if(n==null)break;s+=n}return s}",
    )
    .unwrap();
    assert!(read.code.contains("n=a[i++]"), "{}", read.code);

    let original = "function m(a,b){var i=a.length,j=0;for(;j<b.length;j++)a[i]=b[j],i++;a.length=i;return a}console.log(JSON.stringify(m([1], [2,3])))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "[1,2,3]");
}

#[test]
fn folds_conditional_push_into_a_singleton_array() {
    let optimized = optimize_generated_javascript(
        "function eq(f){var i=this.length;g=[];f>=0&&f<i&&g.push(this[f]);return this.pushStack(g)}",
    )
    .unwrap();
    assert!(
        optimized
            .code
            .contains("return this.pushStack(f>=0&&f<i?[this[f]]:[])"),
        "{}",
        optimized.code
    );

    let original = "function eq(f){var i=this.length;var g=[];f>=0&&f<i&&g.push(this[f]);return g}console.log(JSON.stringify(eq.call({0:\"a\",1:\"b\",length:2},1)))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "[\"b\"]");
}

#[test]
fn folds_guarded_boolean_and_into_an_addend() {
    let source = "function eq(f){var i=this.length,g=f<0;!g||(g=this.length);f+=g;return f}";
    let (direct, count) = super::fold_guarded_and_addends(source).unwrap();
    assert!(
        count > 0 && direct.contains("f+=f<0&&this.length"),
        "{count} {direct}"
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("f+=f<0&&this.length") || optimized.code.contains("f+=f<0&&i"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains(",g="), "{}", optimized.code);

    let original = "function eq(f){var g=f<0;!g||(g=this.length);f+=g;return f}console.log([eq.call({length:4},-1),eq.call({length:4},2)].join(\",\"))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "3,2");
}

#[test]
fn folds_statement_unit_updates_to_postfix() {
    let optimized =
        optimize_generated_javascript("function each(a,n){for(var i=0;i<n;i+=1)a[i]+=1;return a}")
            .unwrap();
    assert!(optimized.code.contains("i++"), "{}", optimized.code);
    assert!(!optimized.code.contains("i+=1"), "{}", optimized.code);
    assert!(optimized.code.contains("a[i]+=1"), "{}", optimized.code);

    let original = "function each(a,n){for(var i=0;i<n;i+=1)a[i]+=1;return a}console.log(each([1,2],2).join(\",\"))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "2,3");
}

#[test]
fn folds_coalesced_or_into_a_returned_disjunction() {
    let optimized = optimize_generated_javascript(
        "function end(){var s=this.prevObject;!s&&(s=this.constructor());return s}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function end(){return this.prevObject||this.constructor()}"
    );

    let original = "function end(){var s=this.prevObject;!s&&(s=this.constructor());return s}console.log(end.call({prevObject:0,constructor:()=>\"c\"}))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "c");
}

#[test]
fn folds_coalesced_or_into_a_method_receiver() {
    let optimized = optimize_generated_javascript(
        "function strip(e){e=e.match(o);!e&&(e=[]);return e.join(\" \")}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function strip(e){return(e.match(o)||[]).join(\" \")}"
    );
}

#[test]
fn flattens_parenthesized_string_concat_chains() {
    let optimized =
        optimize_generated_javascript("function tag(n){return \"[object \"+(n+\"]\")}").unwrap();
    assert_eq!(
        optimized.code,
        "function tag(n){return\"[object \"+n+\"]\"}"
    );
}

#[test]
fn flips_trailing_false_equality_without_dropping_the_callee() {
    let optimized = optimize_generated_javascript(
        "function each(t,n){for(r in t)if(n.call(t[r],r,t[r])===!1)break;return t}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("!1===n.call(t[r],r,t[r])"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("n.!1"), "{}", optimized.code);

    let apply = optimize_generated_javascript(
        "function fire(a,i,m){if(a[i].apply(m[0],m[1])===!1)a.length=0}",
    )
    .unwrap();
    assert!(
        apply.code.contains("!1===a[i].apply(m[0],m[1])"),
        "{}",
        apply.code
    );
    assert!(!apply.code.contains(".!1"), "{}", apply.code);
}

#[test]
fn folds_false_break_into_the_for_condition() {
    let optimized = optimize_generated_javascript(
        "function each(t,n){for(var r=0,e=t.length;r<e;r++){if(n.call(t[r],r,t[r])===!1)break}return t}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("!1!==n.call(t[r],r,t[r])")
            && optimized.code.contains("r++")
            && !optimized.code.contains("break"),
        "{}",
        optimized.code
    );
}

#[test]
fn folds_truthy_index_walks_into_for_header_assigns() {
    let optimized = optimize_generated_javascript(
        "function text(o){var d=0,t=\"\",M;for(;!0;){M=d+1;d=o[d];if(!d)break;t+=l(d);d=M}return t}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("M=o[d++]") && optimized.code.contains("t+=l(M);"),
        "{}",
        optimized.code
    );

    let original = "function l(n){return n.v||\"\"}function text(o){var d=0,t=\"\",M;for(;!0;){M=d+1;d=o[d];if(!d)break;t+=l(d);d=M}return t}console.log(text([{v:\"a\"},{v:\"b\"}]))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "ab");
}

#[test]
fn folds_self_minus_one_into_decrement() {
    let optimized = optimize_generated_javascript("function f(i){i=i-1;return i}").unwrap();
    assert!(optimized.code.contains("i--"), "{}", optimized.code);
    assert!(!optimized.code.contains("i=i-1"), "{}", optimized.code);
}

#[test]
fn copies_identifier_aliases_into_their_only_reads() {
    let optimized = optimize_generated_javascript(
        "function xml(f){var h=f&&f.nodeName,E=f&&f.namespaceURI;f=T;!E&&(E=h||\"HTML\");return!f.test(E)}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("return!T.test(") && !optimized.code.contains("f=T"),
        "{}",
        optimized.code
    );
}

#[test]
fn folds_temp_computed_keys_into_the_member() {
    let optimized = optimize_generated_javascript(
        "function add(h,E){let i=\"[object \"+E+\"]\";h[i]=E.toLowerCase();return h}",
    )
    .unwrap();
    assert!(
        optimized
            .code
            .contains("h[\"[object \"+E+\"]\"]=E.toLowerCase()"),
        "{}",
        optimized.code
    );

    let arrow = optimize_generated_javascript(
        "let add=(h,E)=>{let i=\"[object \"+E+\"]\";h[i]=E.toLowerCase()}",
    )
    .unwrap();
    assert!(
        arrow.code.contains("h[\"[object \"+E+\"]\"]"),
        "{}",
        arrow.code
    );
    assert!(arrow.code.contains("}"), "{}", arrow.code);
}

#[test]
fn folds_trailing_for_increments_into_the_header() {
    let optimized = optimize_generated_javascript(
        "function m(f,h){for(var r=+h.length,E=0,i=0;i<r;){f[E++]=h[i];i++;}return f}",
    )
    .unwrap();
    assert!(
        optimized
            .code
            .contains("for(var r=+h.length,E=0,i=0;i<r;i++)"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("i++;"), "{}", optimized.code);
    assert!(
        optimized.code.contains("h[i];return") || optimized.code.contains("h[i]; return"),
        "{}",
        optimized.code
    );
}

#[test]
fn does_not_comma_join_continue_into_an_expression() {
    let optimized = optimize_generated_javascript(
        "function f(){for(var n=0;n<3;n++){if(n==1){n++;continue}}return n}",
    )
    .unwrap();
    assert!(!optimized.code.contains(",continue"), "{}", optimized.code);
    assert_eq!(
        run_node(&format!("{};console.log(f())", optimized.code)).trim(),
        "3"
    );
}

#[test]
fn does_not_lift_trailing_increment_when_continue_already_increments() {
    let original = "function f(a){var n=0,s=0;for(;n<a.length;){if(a[n]==null){n++;continue}s+=a[n];n++;}return s}console.log(f([1,null,2]))";
    let optimized = optimize_generated_javascript(original).unwrap();
    assert!(
        !optimized.code.contains("for(;n<a.length;n++)"),
        "{}",
        optimized.code
    );
    assert_eq!(run_node(&optimized.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&optimized.code).trim(), "3");
}

#[test]
fn does_not_lift_trailing_increment_when_continue_skips_it() {
    let original =
        "function f(a){var n=0,s=0;for(;n<a.length;){if(a[n]==null)continue;s+=a[n];n++;}return s}";
    let optimized = optimize_generated_javascript(original).unwrap();
    assert!(
        !optimized.code.contains("for(;n<a.length;n++)"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("n++") || optimized.code.contains("++n"),
        "{}",
        optimized.code
    );
}

#[test]
fn uses_cached_length_in_the_for_condition() {
    let each = optimize_generated_javascript(
        "function each(j,t){for(var n=j.length,r=0;r<j.length&&t(j[r]);r++);return j}",
    )
    .unwrap();
    assert!(each.code.contains("r<n&&"), "{}", each.code);
    assert!(!each.code.contains("r<j.length"), "{}", each.code);

    let grep = optimize_generated_javascript(
        "function g(j,t,r){var n=[],i=j.length,o=!r;for(r=0;r<j.length;r++)n.push(j[r]);return n}",
    )
    .unwrap();
    assert!(grep.code.contains("r<i"), "{}", grep.code);

    let prefix = optimize_generated_javascript(
        "let v=(i,q,t)=>{var b=[],d=i.length,p=!t;for(t=0;t<i.length;++t)!q(i[t],t)!=p&&b.push(i[t]);return b}",
    )
    .unwrap();
    assert!(prefix.code.contains("t<d"), "{}", prefix.code);
}

#[test]
fn copies_module_aliases_past_nested_function_assigns() {
    let optimized =
        optimize_generated_javascript("let q=n;function x(){var n=1;return n}k={push:q}").unwrap();
    assert!(optimized.code.contains("push:n"), "{}", optimized.code);
    assert!(!optimized.code.contains("push:q"), "{}", optimized.code);
}

#[test]
fn copies_reads_before_a_later_reassignment() {
    let optimized =
        optimize_generated_javascript("let N=j;k={constructor:N};N=j;c.call(N)").unwrap();
    assert!(
        optimized.code.contains("constructor:j") && optimized.code.contains("c.call(j)"),
        "{}",
        optimized.code
    );
}

#[test]
fn prefers_a_cached_member_binding_over_rereading_the_property() {
    let text = optimize_generated_javascript(
        "function text(t){var r=t.nodeType;if(!t.nodeType){return\"\"}return r}",
    )
    .unwrap();
    assert!(
        text.code.contains("!r") && text.code.contains("return"),
        "{}",
        text.code
    );
    assert_eq!(text.code.matches("nodeType").count(), 1, "{}", text.code);

    let grep = optimize_generated_javascript(
        "let i=z=>z,l=(z,t,r)=>{var n=[],i=z.length,o=!r;for(r=0;r<z.length;r++)n.push(z[r]);return n}",
    )
    .unwrap();
    assert!(grep.code.contains("r<i"), "{}", grep.code);
}

#[test]
fn drops_unused_member_copy_declarators() {
    let optimized =
        optimize_generated_javascript("let q=k.push,T=k.sort;k={push:k.push,sort:k.sort}").unwrap();
    assert!(!optimized.code.contains("q="), "{}", optimized.code);
    assert!(!optimized.code.contains("T="), "{}", optimized.code);
    assert!(optimized.code.contains("push:k.push"), "{}", optimized.code);

    let used_before = optimize_generated_javascript(
        "let o=j=>b.call(j);var b=t.toString,q=k.push;k={push:k.push}",
    )
    .unwrap();
    assert!(
        used_before.code.contains("b=t.toString"),
        "{}",
        used_before.code
    );
    assert!(!used_before.code.contains("q="), "{}", used_before.code);
}

#[test]
fn removing_a_middle_declarator_keeps_one_separator() {
    let index_temp = optimize_generated_javascript(
        "function f(e){var S=e[2],R=e[5],n=e[1];if(R)use(S,n);return S}",
    )
    .unwrap();
    assert!(
        !index_temp.code.contains("[2]n=") && !index_temp.code.contains("[2]R="),
        "{}",
        index_temp.code
    );
    assert!(
        index_temp.code.contains("e[2]") && index_temp.code.contains("e[1]"),
        "{}",
        index_temp.code
    );

    let literal =
        optimize_generated_javascript("var M=Math.random,N=()=>{},V=\"length\";use(M(),V,N)")
            .unwrap();
    assert!(
        !literal.code.contains("randomV") && !literal.code.contains("randomN"),
        "{}",
        literal.code
    );
    assert!(
        literal.code.contains("Math.random") && literal.code.contains("\"length\""),
        "{}",
        literal.code
    );
}

#[test]
fn rematerializes_same_scope_single_use_empty_functions_and_regexes() {
    let noop = optimize_generated_javascript("let H=()=>{},o=1;k={noop:H,id:o}").unwrap();
    assert!(
        (noop.code.contains("noop(){}") || noop.code.contains("noop:()=>{}"))
            && !noop.code.contains("H="),
        "{}",
        noop.code
    );

    let regex = optimize_generated_javascript(
        "let O=/\\D/g;k={expando:(\"jQuery\"+Math.random()).replace(O,\"\")}",
    )
    .unwrap();
    assert!(
        regex.code.contains(".replace(/\\D/g,\"\")") && !regex.code.contains("O="),
        "{}",
        regex.code
    );

    let shared = optimize_generated_javascript("let H=()=>{};k={a:H,b:H}").unwrap();
    assert!(shared.code.contains("H=()=>{}"), "{}", shared.code);

    let nested = optimize_generated_javascript("let H=()=>{};function f(){return H}").unwrap();
    assert!(
        nested.code.contains("H=()=>{}") && nested.code.contains("return H"),
        "{}",
        nested.code
    );
}

#[test]
fn reuses_existing_tostring_aliases_for_prototype_calls() {
    let object = optimize_generated_javascript(
        "let K=a=>k[Object.prototype.toString.call(a)];var k={},C=k.toString",
    )
    .unwrap();
    assert!(
        object.code.contains("C.call(a)") && !object.code.contains("Object.prototype.toString"),
        "{}",
        object.code
    );

    let function = optimize_generated_javascript(
        "let q=a=>D.call(a)==w;var w=Function.prototype.toString.call(Object),k={},t=k.hasOwnProperty,D=t.toString",
    )
    .unwrap();
    assert!(
        function.code.contains("D.call(a)==D.call(Object)")
            && !function.code.contains("Function.prototype.toString")
            && !function.code.contains("w="),
        "{}",
        function.code
    );

    let later_alias = optimize_generated_javascript(
        "function f(){var T=void 0;return T}var w=Function.prototype.toString.call(Object),k={},t=k.hasOwnProperty,D=t.toString;let q=a=>D.call(a)==w",
    )
    .unwrap();
    assert!(
        !later_alias.code.contains("w=D.call(Object)")
            && !later_alias.code.contains("w=A.call(Object)"),
        "{}",
        later_alias.code
    );
    assert!(
        later_alias
            .code
            .contains("Function.prototype.toString.call(Object)")
            || later_alias.code.contains("D.call(a)==D.call(Object)")
            || later_alias.code.contains("t.toString.call(a)==D.call(Object)"),
        "{}",
        later_alias.code
    );

    let eager_alias = optimize_generated_javascript(
        "function f(){var E=void 0;return E}var E=D.call(Object),k={},t=k.hasOwnProperty,D=t.toString;let q=a=>D.call(a)==E",
    )
    .unwrap();
    assert!(
        eager_alias
            .code
            .contains("Function.prototype.toString.call(Object)"),
        "{}",
        eager_alias.code
    );
    assert!(
        !eager_alias.code.contains("=D.call(Object)"),
        "{}",
        eager_alias.code
    );
}

#[test]
fn chains_repeated_member_assigns_of_the_same_binding() {
    let optimized = optimize_generated_javascript("k={};j.fn=k;j.prototype=k;j.extend=c").unwrap();
    assert!(
        optimized.code.contains("j.fn=j.prototype=k"),
        "{}",
        optimized.code
    );
}

#[test]
fn folds_coalesced_or_into_a_regex_test() {
    let optimized = optimize_generated_javascript(
        "function xml(e,t,o){var d=e&&e.namespaceURI;!d&&(d=t&&t.nodeName||\"HTML\");return!o.test(d)}",
    )
    .unwrap();
    assert_eq!(
        optimized.code,
        "function xml(e,t,o){return!o.test(e&&e.namespaceURI||t&&t.nodeName||\"HTML\")}"
    );
}

#[test]
fn drops_void_undefined_before_an_immediate_reassign() {
    let optimized =
        optimize_generated_javascript("function f(){var a;a=void 0;a=g();return a}").unwrap();
    assert!(
        optimized.code.contains("a=g()") && !optimized.code.contains("void 0"),
        "{}",
        optimized.code
    );

    let comma =
        optimize_generated_javascript("function f(){var e=()=>1,a=void 0;a={n:e};return a}")
            .unwrap();
    assert!(
        comma.code.contains("a={n:") && !comma.code.contains("void 0"),
        "{}",
        comma.code
    );
}

#[test]
fn drops_unused_void_undefined_declarators() {
    let unused = optimize_generated_javascript(
        "function when(n){var r=n[0];var w=void 0;if(c(r&&r.then))return n.then();return n}",
    )
    .unwrap();
    assert!(!unused.code.contains("void 0"), "{}", unused.code);
    assert!(!unused.code.contains("var w"), "{}", unused.code);

    let kept = optimize_generated_javascript("function f(){var T=void 0;return T}").unwrap();
    assert!(
        kept.code.contains("void 0") || kept.code.contains("return"),
        "{}",
        kept.code
    );
    assert!(
        kept.code.contains("T") || kept.code.contains("return"),
        "{}",
        kept.code
    );

    let unused_first = optimize_generated_javascript(
        "function hook(e,r){var n,t=window.console;(t&&t.warn)&&e&&_.test(e.name)&&t.warn(e)}",
    )
    .unwrap();
    assert!(
        !unused_first.code.contains("var n"),
        "{}",
        unused_first.code
    );
    assert!(
        unused_first.code.contains("window.console"),
        "{}",
        unused_first.code
    );

    let self_ref = optimize_generated_javascript(
        "function Deferred(){var e,i=[],t=\"pending\",r={};e={catch(n){return e.then(n)},state(){return t}};return e}",
    )
    .unwrap();
    assert!(
        self_ref.code.contains("var i=[],t=\"pending\",r={},e={")
            || self_ref.code.contains("var i=[],t=\"pending\",e={"),
        "{}",
        self_ref.code
    );
    assert!(!self_ref.code.contains("var e,"), "{}", self_ref.code);

    let comma_follow = optimize_generated_javascript(
        "function Deferred(i){var e,r={};e={state(){return 1},promise(n){return n||e}};each(i,e);return e}",
    )
    .unwrap();
    assert!(
        comma_follow.code.contains("each(") && !comma_follow.code.contains("var e,{"),
        "{}",
        comma_follow.code
    );
    assert!(
        !comma_follow.code.contains(",each(") || comma_follow.code.contains(";each("),
        "{}",
        comma_follow.code
    );
}

#[test]
fn folds_guarded_assign_into_a_call_predicate() {
    let optimized = optimize_generated_javascript(
        "function then(ge){var d=typeof ge,l=void 0;ge&&(\"object\"==d||\"function\"==d)&&(l=ge.then);if(c(l)){l.call(ge)}else{done(ge)}}",
    )
    .unwrap();
    assert!(
        (optimized.code.contains("if(c(l=") || optimized.code.contains("c(l="))
            && optimized.code.contains("&&ge.then)"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("&&(l="), "{}", optimized.code);

    let disjunction = optimize_generated_javascript(
        "function then(a,b){var l=void 0;a||b&&(l=a.then);if(c(l)){l.call(a)}return l}",
    )
    .unwrap();
    assert!(disjunction.code.contains("&&(l="), "{}", disjunction.code);
    assert!(
        !disjunction.code.contains("if(c(l="),
        "{}",
        disjunction.code
    );
}

#[test]
fn folds_braced_if_else_expression_sequences() {
    let optimized = optimize_generated_javascript(
        "function run(n,p){if(0!=n){p()}else{hook?p.error=hook():ready||(p.error=stack()),setTimeout(p)}}",
    )
    .unwrap();
    assert!(optimized.code.contains("?"), "{}", optimized.code);
    assert!(!optimized.code.contains("if(0!=n)"), "{}", optimized.code);
}

#[test]
fn rematerializes_single_use_index_chains() {
    let optimized = optimize_generated_javascript(
        "function then(i,e,A,F){var o=i[0][3];o.add(e(0,A,F,A.notifyWith));i[1][3].add(e(0,A,F,null))}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("i[0][3].add(") && !optimized.code.contains("var o="),
        "{}",
        optimized.code
    );

    let key = optimize_generated_javascript(
        "function each(s,r,o){var F=s[0];r[F]=function(){return r[s[0]+\"With\"](this)},r[s[0]+\"With\"]=o.fireWith;return r}",
    )
    .unwrap();
    assert!(
        key.code.contains("r[s[0]]=") && !key.code.contains("var F="),
        "{}",
        key.code
    );

    let thenable = optimize_generated_javascript(
        "function when(r,i,t,U,n){if(e<=1&&(q(r,n.resolve,n.reject,!e),r=i[t],\"pending\"==n.state()||U(r&&r.then)))return n.then();return n}",
    )
    .unwrap();
    assert!(
        thenable.code.contains("i[t]&&i[t].then") && !thenable.code.contains("r=i[t]"),
        "{}",
        thenable.code
    );
}

#[test]
fn rematerializes_nested_single_use_literals_and_expression_calls() {
    let regex = optimize_generated_javascript(
        "let A=/x/g;function f(e){return e.match(A)}",
    )
    .unwrap();
    assert!(
        regex.code.contains("e.match(/x/g)") && !regex.code.contains("A="),
        "{}",
        regex.code
    );

    let object = optimize_generated_javascript(
        "let N={type:!0};function f(){for(var i in N)return i}",
    )
    .unwrap();
    assert!(
        (object.code.contains("for(var i in {type:!0})")
            || object.code.contains("for(var i in{type:!0})"))
            && !object.code.contains("N="),
        "{}",
        object.code
    );

    let document = optimize_generated_javascript(
        "var T=document;function f(n){n=n||T;return n}",
    )
    .unwrap();
    assert!(
        document.code.contains("n=n||document") && !document.code.contains("T="),
        "{}",
        document.code
    );

    let ty = optimize_generated_javascript("let M=e=>e+1;function f(i){return M(i)}").unwrap();
    assert!(
        ty.code.contains("(e=>e+1)(i)") && !ty.code.contains("M="),
        "{}",
        ty.code
    );

    let factory = optimize_generated_javascript("let G=e=>{var t=e+1;return t};use(G(2))").unwrap();
    assert!(factory.code.contains("G="), "{}", factory.code);

    let procedure = optimize_generated_javascript(
        "let P=(e,t,n)=>{n.head.appendChild(e)};function globalEval(e,t,n){P(e,t,n)}",
    )
    .unwrap();
    assert!(
        procedure.code.contains("((e,t,n)=>{") && !procedure.code.contains("P="),
        "{}",
        procedure.code
    );

    let nonce = optimize_generated_javascript(
        "function globalEval(e,t,n){var r={};r.nonce=t&&t.nonce,P(e,r,n)}",
    )
    .unwrap();
    assert!(
        nonce.code.contains("P(e,{nonce:t&&t.nonce},n)") && !nonce.code.contains("var r="),
        "{}",
        nonce.code
    );

    let nonce_iife = optimize_generated_javascript(
        "function globalEval(e,t,n){var r={};r.nonce=t&&t.nonce,((e,t,n)=>{var r=n.createElement(\"script\");r.text=e})(e,r,n)}",
    )
    .unwrap();
    assert!(
        nonce_iife.code.contains("{nonce:t&&t.nonce}") && !nonce_iife.code.contains("var r={}"),
        "{}",
        nonce_iife.code
    );

    let later_regex = optimize_generated_javascript(
        "function f(e){return e.match(A)}var A=/x/g;",
    )
    .unwrap();
    assert!(
        later_regex.code.contains("e.match(/x/g)") && !later_regex.code.contains("A="),
        "{}",
        later_regex.code
    );

    let later_object = optimize_generated_javascript(
        "let P=()=>{for(var i in N)return i},N={type:!0};",
    )
    .unwrap();
    assert!(
        (later_object.code.contains("in {type:!0}") || later_object.code.contains("in{type:!0}"))
            && !later_object.code.contains("N="),
        "{}",
        later_object.code
    );

    let later_document = optimize_generated_javascript(
        "let P=(n)=>{n=n||R;return n};var R=document;",
    )
    .unwrap();
    assert!(
        later_document.code.contains("n=n||document") && !later_document.code.contains("R="),
        "{}",
        later_document.code
    );

    let shadowed = optimize_generated_javascript(
        "function each(C,s){return C}var C=/x/;a.hook=()=>C.test(\"x\")",
    )
    .unwrap();
    assert!(
        shadowed.code.contains("/x/.test") && !shadowed.code.contains("var C="),
        "{}",
        shadowed.code
    );

    let unused_document = optimize_generated_javascript(
        "var l=[],E=Function.prototype.toString.call(Object),p={},T=document;var g=l.slice;function f(n){return n||document}",
    )
    .unwrap();
    assert!(
        !unused_document.code.contains("T="),
        "{}",
        unused_document.code
    );

    let ty_long = optimize_generated_javascript(
        "let M=e=>e==null?e+\"\":\"object\"==typeof e||\"function\"==typeof e?(e=p[O.call(e)])?e:\"object\":typeof e,c=e=>1;a.Callbacks=a=>\"string\"!=M(i)",
    )
    .unwrap();
    assert!(
        ty_long.code.contains("(e=>e==null") && !ty_long.code.contains("M="),
        "{}",
        ty_long.code
    );

    let de = optimize_generated_javascript(
        "let de=e=>e==null?e+\"\":\"object\"==typeof e||\"function\"==typeof e?(e=D[ne.call(e)])?e:\"object\":typeof e,he=e=>{var t={};C(e.match(/x/g)||[],(i,n)=>{t[n]=!0});return t};a.Callbacks=a=>\"string\"!=de(i)",
    )
    .unwrap();
    assert!(
        de.code.contains("(e=>e==null") && !de.code.contains("de="),
        "{}",
        de.code
    );
}

#[test]
fn rematerializes_single_use_predicate_ternaries() {
    let optimized = optimize_generated_javascript(
        "function then(z,h,i,e,A){var F=c(z)?z:h;i[0][3].add(e(0,A,F,A.notifyWith))}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("e(0,A,c(z)?z:h,A.notifyWith)"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("var F="), "{}", optimized.code);

    let assigned = optimize_generated_javascript(
        "function then(r,s,h,f,i,e,A){var C,G;i[0][3].add(e(0,A,c(z)?z:h,A.notifyWith)),C=c(r)?r:h,i[1][3].add(e(0,A,C,null)),G=c(s)?s:f,i[2][3].add(e(0,A,G,null))}",
    )
    .unwrap();
    assert!(
        assigned.code.contains("e(0,A,c(r)?r:h,null)")
            && assigned.code.contains("e(0,A,c(s)?s:f,null)"),
        "{}",
        assigned.code
    );
    assert!(
        !assigned.code.contains("C=c(r)") && !assigned.code.contains("G=c(s)"),
        "{}",
        assigned.code
    );

    let calls = optimize_generated_javascript(
        "function then(e,t,r,h,s,f,l,ge){++t;var me=e(t,r,h,s),he=e(t,r,f,s);l.call(ge,me,he,e(t,r,h,r.notifyWith))}",
    )
    .unwrap();
    assert!(
        calls
            .code
            .contains("l.call(ge,e(t,r,h,s),e(t,r,f,s),e(t,r,h,r.notifyWith))"),
        "{}",
        calls.code
    );
    assert!(
        !calls.code.contains("var he") && !calls.code.contains("var me"),
        "{}",
        calls.code
    );

    let method = optimize_generated_javascript(
        "function when(n,s,e){var E=n.done(s(e)),u=E.resolve;use(u)}",
    )
    .unwrap();
    assert!(
        method.code.contains("n.done(s(e)).resolve"),
        "{}",
        method.code
    );
}

#[test]
fn folds_copied_receiver_method_and_false_phi() {
    let optimized = optimize_generated_javascript(
        "function hook(n,e,C){var r;(n&&n.warn)&&e?(r=C,r=r.test(e.name)):r=!1,r&&n.warn(e)}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("e&&C.test(e.name)&&n.warn")
            || optimized.code.contains("&&C.test(e.name)&&"),
        "{}",
        optimized.code
    );
    assert!(
        !optimized.code.contains("r=C,r=r.test") && !optimized.code.contains("r=!1"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("var r"), "{}", optimized.code);

    let remat = optimize_generated_javascript(
        "var C=/x/;function hook(n,e){var r;(n&&n.warn)&&e?(r=C,r=r.test(e.name)):r=!1,r&&n.warn(e)}",
    )
    .unwrap();
    assert!(
        remat.code.contains("/x/.test") || remat.code.contains("(/x/).test"),
        "{}",
        remat.code
    );
    assert!(!remat.code.contains("var C="), "{}", remat.code);
    assert!(!remat.code.contains("r=/x/"), "{}", remat.code);

    let outer = optimize_generated_javascript(
        "var p={};function then(J){!hook||(p=J,p.error=hook());return J}",
    )
    .unwrap();
    assert!(outer.code.contains("J.error="), "{}", outer.code);
    assert!(!outer.code.contains("p=J"), "{}", outer.code);
}

#[test]
fn folds_arguments_length_countdown_for() {
    let optimized = optimize_generated_javascript(
        "function when(){var t=arguments.length;for(;t>0;)--t,use(t);return t}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("for(;t--;)")
            || optimized.code.contains("for(var t=arguments.length;t--;)"),
        "{}",
        optimized.code
    );
    let postfix = optimize_generated_javascript(
        "function when(){var t=arguments.length;for(;t>0;)t--,use(t);return t}",
    )
    .unwrap();
    assert!(
        postfix.code.contains("for(;t--;)")
            || postfix.code.contains("for(var t=arguments.length;t--;)"),
        "{}",
        postfix.code
    );
    let array_len = optimize_generated_javascript(
        "function when(){var c=[],b=c.length;for(;b>0;){b=b-1;delete d[c[b]]}return b}",
    )
    .unwrap();
    assert!(
        array_len.code.contains("b--;") && !array_len.code.contains("b>0"),
        "{}",
        array_len.code
    );

    let float_like =
        optimize_generated_javascript("function when(t){t=1.5;for(;t>0;)--t,use(t);return t}")
            .unwrap();
    assert!(
        !float_like.code.contains("t--;") && float_like.code.contains("t>0"),
        "{}",
        float_like.code
    );

    let zero_and = optimize_generated_javascript(
        "function when(){var e=arguments.length;return function(){e--,0==e&&done()}}",
    )
    .unwrap();
    assert!(zero_and.code.contains("--e||"), "{}", zero_and.code);
    assert!(!zero_and.code.contains("0==e"), "{}", zero_and.code);

    let method = optimize_generated_javascript(
        "a.extend({when(r){var e=arguments.length;return function(p){a=p,e--,0==e&&done();return a}}})",
    )
    .unwrap();
    assert!(method.code.contains("--e||"), "{}", method.code);

    let deferred_when = optimize_generated_javascript(
        "a.extend({when(r){var e=arguments.length,t=e,o=[],i=g.call(arguments),n=a.Deferred(),s=d=>function(p){o[d]=this,i[d]=arguments.length>1?g.call(arguments):p,e--,0==e&&n.resolveWith(o,i)};return n.promise()}})",
    )
    .unwrap();
    assert!(deferred_when.code.contains("--e||"), "{}", deferred_when.code);

    let comma_stmt = optimize_generated_javascript(
        "function when(){var e=arguments.length;return function(p){a=p,e--,0==e&&done();return a}}",
    )
    .unwrap();
    assert!(comma_stmt.code.contains("--e||"), "{}", comma_stmt.code);

    let call_arg = optimize_generated_javascript(
        "function when(){var e=arguments.length;return function(){foo(e--,0==e&&done())}}",
    )
    .unwrap();
    assert!(!call_arg.code.contains("--e||"), "{}", call_arg.code);

    let for_header = optimize_generated_javascript(
        "function when(){var e=arguments.length;for(;e--,0==e&&done(););return e}",
    )
    .unwrap();
    assert!(!for_header.code.contains("--e||"), "{}", for_header.code);

    let not_length =
        optimize_generated_javascript("function when(e){e=1.5;e--,0==e&&done();return e}").unwrap();
    assert!(
        not_length.code.contains("0==e") || not_length.code.contains("e==0"),
        "{}",
        not_length.code
    );

    let omit_false = optimize_generated_javascript(
        "function when(){var y=(e,n,r,i)=>{n.apply(void 0,[e].slice(i))};y(a,b,c,!1);y(a,b,c,!e);return y}",
    )
    .unwrap();
    assert!(
        omit_false.code.contains("y(a,b,c)") && omit_false.code.contains("y(a,b,c,!e)"),
        "{}",
        omit_false.code
    );
    assert!(
        !omit_false.code.contains("y(a,b,c,!1)"),
        "{}",
        omit_false.code
    );
}

#[test]
fn keeps_semicolon_between_var_and_if() {
    let kept = optimize_generated_javascript(
        "function f(e){if(e.length>=2){var r=e.charAt(0),t=e.charAt(e.length-1);if(r==t)return e.slice(1)}return e}",
    )
    .unwrap();
    assert!(
        !kept.code.contains(",if(") && kept.code.contains("if("),
        "{}",
        kept.code
    );
}

#[test]
fn flips_member_false_equality_without_splitting_the_property() {
    let flipped = optimize_generated_javascript(
        "function f(e){return e.disabled===!1}",
    )
    .unwrap();
    assert!(
        flipped.code.contains("!1===e.disabled") && !flipped.code.contains("e.!"),
        "{}",
        flipped.code
    );
}

#[test]
fn does_not_rewrite_property_names_when_folding_a_same_named_local() {
    let folded = optimize_generated_javascript(
        "function f(e){var disabled=!1;return e.disabled===disabled}",
    )
    .unwrap();
    assert!(
        folded.code.contains("e.disabled") && !folded.code.contains("e.!"),
        "{}",
        folded.code
    );
}

#[test]
fn parenthesizes_or_conditions_when_folding_if_to_and() {
    let cache = optimize_generated_javascript(
        "function B(e,a){var r=a[e];if(!r){r={};if(1==a.nodeType||9==a.nodeType||!a.nodeType){a.nodeType?a[e]=r:Object.defineProperty(a,e,{configurable:!0,value:r})}}return r}",
    )
    .unwrap();
    assert!(
        cache.code.contains("(1==a.nodeType||9==a.nodeType||!a.nodeType)&&")
            || cache.code.contains("(1==a.nodeType||9==a.nodeType||!a.nodeType) &&"),
        "{}",
        cache.code
    );
    assert!(
        !cache.code.contains("||!a.nodeType&&") && !cache.code.contains("||9==a.nodeType||!a.nodeType&&"),
        "{}",
        cache.code
    );

    let or_body = optimize_generated_javascript("function f(a,b,c){if(a)b||c;return b}").unwrap();
    assert!(
        or_body.code.contains("a&&(b||c)") || or_body.code.contains("if(a)"),
        "{}",
        or_body.code
    );
    assert!(!or_body.code.contains("a&&b||c"), "{}", or_body.code);
}

#[test]
fn drops_empty_ternary_then_comma() {
    let repaired = optimize_generated_javascript(
        r#"function when(e,t,n,r,c){if(e<=1&&(M(r,t.done(s(e)).resolve,t.reject,!e),r=i[n],"pending"==t.state()?,!0:(!r||(r=r.then),r=c(r))))return t.then();return t}"#,
    )
    .unwrap();
    assert!(
        !repaired.code.contains("?,") && repaired.code.contains("?"),
        "{}",
        repaired.code
    );
}

#[test]
fn keeps_snapshot_arrow_iifes_that_close_over_a_nested_function() {
    let kept = optimize_generated_javascript(
        "function f(d){var Y=((e)=>()=>{try{e()}catch(c){}})(d);d=other;Y();return Y}",
    )
    .unwrap();
    assert!(
        kept.code.contains("=>") && !kept.code.contains("try{d()}"),
        "{}",
        kept.code
    );
}

#[test]
fn beta_reduces_identity_arrow_iifes() {
    let reduced = optimize_generated_javascript(
        "function has(t){t=(r=>K(f,r))(t)||(r=>K(p,r))(t);return t}function data(e,t,n){return((r,e,s)=>V(f,r,e,s))(e,t,n)}",
    )
    .unwrap();
    assert!(
        reduced.code.contains("K(f,t)")
            && reduced.code.contains("K(p,t)")
            && reduced.code.contains("V(f,e,t,n)")
            && !reduced.code.contains("=>K")
            && !reduced.code.contains("=>V"),
        "{}",
        reduced.code
    );
}

#[test]
fn folds_ident_ternary_to_or_and_length_not_gt_zero() {
    let queue = optimize_generated_javascript(
        "function y(a){var b=a;return b?b:[]}function z(){var c=[];var d=c.length;c.shift();return !(d>0)&&c}",
    )
    .unwrap();
    assert!(
        queue.code.contains("||[]") && !queue.code.contains("?"),
        "{}",
        queue.code
    );
    assert!(
        queue.code.contains("!d") && !queue.code.contains("d>0"),
        "{}",
        queue.code
    );
}

#[test]
fn folds_if_return_chains_when_regex_literals_are_present() {
    let isolated = optimize_generated_javascript(
        r#"function oa(a){if("true"==a)return !0;if("false"==a)return !1;if("null"==a)return null;if(a==+a+"")return +a;return da.test(a)?JSON.parse(a+""):a}var da=/^(?:\{[\w\W]*\}|\[[\w\W]*\])$/"#,
    )
    .unwrap();
    assert!(
        isolated.code.contains("?") && !isolated.code.contains("if(\"true\""),
        "{}",
        isolated.code
    );
}

#[test]
fn folds_redundant_null_or_undefined_checks() {
    let null_or_void = optimize_generated_javascript(
        "function when(c){if(null==c||c===void 0)return T(a,b);return c}",
    )
    .unwrap();
    assert!(
        null_or_void.code.contains("c==null") && !null_or_void.code.contains("void 0"),
        "{}",
        null_or_void.code
    );
}

#[test]
fn folds_proven_integer_neq_zero_in_boolean() {
    let depth = optimize_generated_javascript(
        "function then(){var t=0,e=(n,r)=>function(){0!=n?run(n):wait()};e(0,x);t++;e(t,x);return e}",
    )
    .unwrap();
    assert!(
        depth.code.contains("n?run") || depth.code.contains("n?run("),
        "{}",
        depth.code
    );
    assert!(!depth.code.contains("0!=n"), "{}", depth.code);

    let float_depth =
        optimize_generated_javascript("function then(n){n=1.5;return 0!=n?run():wait()}").unwrap();
    assert!(float_depth.code.contains("0!=n"), "{}", float_depth.code);

    let shadowed_callee = optimize_generated_javascript(
        "function other(){var e=foo;e(1.5)}function then(){var t=0,e=(n)=>function(){0!=n?run():wait()};e(0);t++;e(t);return e}",
    )
    .unwrap();
    assert!(
        !shadowed_callee.code.contains("0!=n"),
        "{}",
        shadowed_callee.code
    );

    let pending_shadow = optimize_generated_javascript(
        "function B(n){var t=\"pending\",e={state(){return t},then(r,s,z){var t=0,e=(n,r,i,s)=>function(){0!=n?run(n):wait();t++;foo(e(t,r,h,s))};e(0,x);return e}}}",
    )
    .unwrap();
    assert!(
        !pending_shadow.code.contains("0!=n"),
        "{}",
        pending_shadow.code
    );

    let if_then_else = optimize_generated_javascript(
        "function then(){var t=0,e=(n)=>function(){if(0!=n){run(n)}else{wait()}};e(0);t++;e(t);return e}",
    )
    .unwrap();
    assert!(!if_then_else.code.contains("0!=n"), "{}", if_then_else.code);
    assert!(
        if_then_else.code.contains("n?run") || if_then_else.code.contains("if(n)"),
        "{}",
        if_then_else.code
    );
}

#[test]
fn folds_predicate_reassign_and_if_prefix_return() {
    let pred = optimize_generated_javascript(
        "function pipe(e,i){var k=c(e[i[4]]);k&&(k=e[i[4]]),use(k);return k}",
    )
    .unwrap();
    assert!(pred.code.contains("c(e[i[4]])&&e[i[4]]"), "{}", pred.code);
    assert!(!pred.code.contains("&&e[i[4]],use("), "{}", pred.code);

    let guard = optimize_generated_javascript(
        "function when(r,i,t){if(e<=1){var d=s.reject;y(r,s.done(a(r)).resolve,d,0==e),r=i[t];if(\"pending\"==n.state()||c(r&&r.then))return n.then()}return n}",
    )
    .unwrap();
    assert!(
        guard.code.contains("e<=1&&(")
            && (guard.code.contains("return n.then()") || guard.code.contains("?n.then():n")),
        "{}",
        guard.code
    );
    assert!(!guard.code.contains("if(\"pending\""), "{}", guard.code);
    assert!(
        guard.code.contains("s.reject") && !guard.code.contains("var d="),
        "{}",
        guard.code
    );

    let after_for = optimize_generated_javascript(
        "function when(){if(e<=1&&(y(),c))return n.then();for(;i--;)y();return n}",
    )
    .unwrap();
    assert!(after_for.code.contains("for("), "{}", after_for.code);
    assert!(!after_for.code.contains("then()for"), "{}", after_for.code);

    let apply = optimize_generated_javascript(
        "function pipe(x){var i;x&&(i=x.apply(this,arguments));return i&&i.promise}",
    )
    .unwrap();
    assert!(
        apply.code.contains("i=x&&x.apply(") || apply.code.contains("var i=x&&x.apply("),
        "{}",
        apply.code
    );

    let comma_src = "(s=x?[i]:arguments,o[u](this,s))";
    let (comma_direct, comma_count) =
        super::fold_comma_assign_into_trailing_call_arg(comma_src).unwrap();
    assert!(
        comma_count > 0 && comma_direct.contains("o[u](this,x?[i]:arguments)"),
        "{comma_count} {comma_direct}"
    );
    let comma_arg = optimize_generated_javascript(
        "function pipe(x,i,o,u){return i?o.p():(s=x?[i]:arguments,o[u](this,s))}",
    )
    .unwrap();
    assert!(
        comma_arg.code.contains("o[u](this,x?[i]:arguments)"),
        "{}",
        comma_arg.code
    );
}

#[test]
fn chained_comma_assigns_do_not_steal_a_var_declarator() {
    let (code, count) = super::fold_chained_comma_assigns(
        "function f(){var _=this,$=arguments,d=()=>{},p=d;return [p,d]}",
    )
    .unwrap();
    assert_eq!(count, 0, "{code}");
    assert!(
        code.contains("var _=this,$=arguments,d=()=>{},p=d"),
        "{code}"
    );

    let (expr, count) = super::fold_chained_comma_assigns("f((d=()=>{},p=d))").unwrap();
    assert!(count > 0, "{expr}");
    assert!(expr.contains("p=d=()=>{}"), "{expr}");
}

#[test]
fn chains_adjacent_identifier_copies_into_one_assign() {
    let (code, count) = super::fold_chained_identifier_assigns(
        "function f(x){var a,b;a=x;b=a;a=null;return [a,b]}",
    )
    .unwrap();
    assert!(count > 0, "{code}");
    assert!(code.contains("b=a=x"), "{code}");

    let (lists, count) = super::fold_chained_identifier_assigns(
        "function f(){var u,o,i,e;u=[];o=u;e=\"\";i=e;return [o,i,u,e]}",
    )
    .unwrap();
    assert!(count >= 2, "{lists}");
    assert!(
        lists.contains("o=u=[]") && lists.contains("i=e=\"\""),
        "{lists}"
    );
}

#[test]
fn does_not_chain_an_identifier_into_a_member_or_index() {
    let (index, count) = super::fold_chained_identifier_assigns(
        "function f(s,o){var u,d;u=s[o];d=u[0];return [u,d]}",
    )
    .unwrap();
    assert_eq!(count, 0, "{index}");
    assert!(index.contains("u=s[o];d=u[0]"), "{index}");
    assert!(!index.contains("d=u=s[o][0]"), "{index}");

    let (member, count) = super::fold_chained_identifier_assigns(
        "function f(e,a){var b,f;b=e[a];f=b.action;return [b,f]}",
    )
    .unwrap();
    assert_eq!(count, 0, "{member}");
    assert!(member.contains("b=e[a];f=b.action"), "{member}");
    assert!(!member.contains("f=b=e[a].action"), "{member}");
}

#[test]
fn folds_preincrement_plus_for_into_prefix_condition() {
    let optimized =
        optimize_generated_javascript("function f(a,i){i++;for(;i<a.length;i++)a[i]();return i}")
            .unwrap();
    assert!(
        optimized.code.contains("for(;++i<a.length;)"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("i++;for"), "{}", optimized.code);
}

#[test]
fn folds_assigned_index_break_into_the_for_condition() {
    let optimized = optimize_generated_javascript(
        "function rem(a,v){for(var i=0;!0;){i=a.indexOf(v,i);if(i<0)break;a.splice(i,1)}return a}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("(i=a.indexOf(v,i))>-1"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("if(i<0)"), "{}", optimized.code);
    assert!(
        !optimized.code.contains("var i=0") && !optimized.code.contains(">=0"),
        "{}",
        optimized.code
    );
}

#[test]
fn absorbs_prior_assigns_into_a_nonempty_for_init() {
    let chained = optimize_generated_javascript(
        "function f(q){var n,c;n=!0;for(c=!0;q.length;c=-1)q.pop();return n}",
    )
    .unwrap();
    assert!(
        chained.code.contains("n=c=!0") || chained.code.contains("for(n=c=!0"),
        "{}",
        chained.code
    );

    let empty_init = optimize_generated_javascript(
        "function f(q){var o,n;o=o||q;n=!0;for(;q.length;)q.pop();return o}",
    )
    .unwrap();
    assert!(
        empty_init.code.contains("for(o=o||q,n=!0;")
            || empty_init.code.contains("for(n=!0,o=o||q;"),
        "{}",
        empty_init.code
    );
}

#[test]
fn folds_statement_or_assign_into_for_init() {
    let and_form = optimize_generated_javascript(
        "function f(o,e,r,n,c,i){!r&&(r=o.once);for(c=n=!0;e.length;i=-1)e.pop();return r}",
    )
    .unwrap();
    assert!(
        and_form.code.contains("for(r=r||o.once,c=n=!0;") || and_form.code.contains("r=r||o.once"),
        "{}",
        and_form.code
    );
    assert!(!and_form.code.contains("!r&&(r="), "{}", and_form.code);

    let if_temp = optimize_generated_javascript(
        "function f(o,e,r,n,c,i){if(!r){var t=o.once;r=t}for(c=n=!0;e.length;i=-1)e.pop();return r}",
    )
    .unwrap();
    assert!(if_temp.code.contains("r=r||o.once"), "{}", if_temp.code);
    assert!(!if_temp.code.contains("if(!r)"), "{}", if_temp.code);

    let original = "function f(o,e,r,n,c,i){!r&&(r=o.once);for(c=n=!0;e.length;i=-1)e.pop();return r}console.log(f({once:7},[1],0,0,0,0)+','+f({once:8},[],1,0,0,0))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "7,1");

    let truthy_clear =
        optimize_generated_javascript("function empty(t){t&&(t=[]);return t}").unwrap();
    assert!(
        truthy_clear.code.contains("t&&(t=[])") || truthy_clear.code.contains("t&&(t=[]),"),
        "{}",
        truthy_clear.code
    );
    assert!(
        !truthy_clear.code.contains("t=t||[]"),
        "{}",
        truthy_clear.code
    );
}

#[test]
fn unwraps_expression_bodies_of_nested_for_loops() {
    let optimized = optimize_generated_javascript(
        "function f(t,e,o,i,a){for(c=n=!0;e.length;i=-1){for(a=e.shift();++i<t.length;){!1===t[i].apply(a[0],a[1])&&o.stopOnFalse&&(i=t.length,a=!1)}}o.memory||(a=!1);return t}",
    )
    .unwrap();
    assert!(
        optimized.code.contains(")for(") && optimized.code.contains(";)!1==="),
        "{}",
        optimized.code
    );
    assert!(
        !optimized.code.contains("length;){") && !optimized.code.contains(";){!1==="),
        "{}",
        optimized.code
    );

    let original = "function f(n){var s=0;for(var i=0;i<n;i++){for(var j=0;j<n;j++){s+=1}}return s}console.log(f(3))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "9");

    let once_unwrapped = optimize_generated_javascript(
        "function d(o,e,t,i,n,c,r,a){for(r=r||o.once,c=n=!0;e.length;i=-1){for(a=e.shift();++i<t.length;)!1===t[i].apply(a[0],a[1])&&o.stopOnFalse&&(i=t.length,a=!1);}o.memory||(a=!1)}",
    )
    .unwrap();
    assert!(
        once_unwrapped.code.contains(")for("),
        "{}",
        once_unwrapped.code
    );
}

#[test]
fn folds_fire_tail_into_comma_statements() {
    let optimized = optimize_generated_javascript(
        "function f(b,a,e){for(;e;)e--;i.memory||(e=!1);d=!1;!b||(a=e?[]:\"\");return a}",
    )
    .unwrap();
    assert!(
        optimized
            .code
            .contains("i.memory||(e=!1),d=!1,b&&(a=e?[]:\"\")")
            || optimized.code.contains(",d=!1,b&&("),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("b&&(") && !optimized.code.contains("!b||("),
        "{}",
        optimized.code
    );

    let lock = optimize_generated_javascript("function lock(e,d,a){!e&&!d&&(e=\"\",a=e);return a}")
        .unwrap();
    assert!(lock.code.contains("a=e=\"\""), "{}", lock.code);

    let fire_tail = optimize_generated_javascript(
        "function d(){for(r=r||o.once,c=n=!0;e.length;i=-1)for(a=e.shift();++i<t.length;)!1===t[i].apply(a[0],a[1])&&o.stopOnFalse&&(i=t.length,a=!1);o.memory||(a=!1);n=!1;r&&(t=a?[]:\"\")}",
    )
    .unwrap();
    assert!(
        fire_tail.code.contains("o.memory||(a=!1),n=!1,r&&("),
        "{}",
        fire_tail.code
    );

    let removed = optimize_generated_javascript(
        "function rem(t,i,n){for(var p=0;(p=l(n,t,p))>=0;){t.splice(p,1);p<=i&&--i}return t}",
    )
    .unwrap();
    assert!(
        removed.code.contains("for(var p;(p=l(n,t,p))>-1;)")
            && (removed.code.contains("t.splice(p,1),p<=i&&i--")
                || removed.code.contains("splice(p,1),p<=i&&i--")),
        "{}",
        removed.code
    );

    let nested_call = optimize_generated_javascript(
        "function rem(t,i,n){return f(arguments,(e,n)=>{for(var p=0;(p=l(n,t,p))>=0;){t.splice(p,1);p<=i&&--i}}),this}",
    )
    .unwrap();
    assert!(
        nested_call.code.contains("t.splice(p,1),p<=i&&i--")
            || nested_call.code.contains("splice(p,1),p<=i&&i--"),
        "{}",
        nested_call.code
    );

    let already_gt = optimize_generated_javascript(
        "function rem(t,i,n){for(var p=0;(p=l(n,t,p))>-1;)t.splice(p,1),p<=i&&i--;return t}",
    )
    .unwrap();
    assert!(
        already_gt.code.contains("for(var p;(p=l(n,t,p))>-1;)"),
        "{}",
        already_gt.code
    );

    let method_unused = optimize_generated_javascript(
        "function make(){var f=-1;return{lock(){var f;return this},read(){return f}}}",
    )
    .unwrap();
    assert!(
        !method_unused.code.contains("lock(){var f;")
            && !method_unused.code.contains("lock(){var f}"),
        "{}",
        method_unused.code
    );
}

#[test]
fn folds_increment_infinite_for_into_prefix_condition() {
    let optimized = optimize_generated_javascript(
        "function f(a,i){i++;for(;;i++){var t=i;if(i>=a.length){break}t=a[i];t()}}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("for(;++i<a.length;)"),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("i++;for"), "{}", optimized.code);
    assert!(!optimized.code.contains("if(i>="), "{}", optimized.code);
    assert!(
        optimized.code.contains("a[i]()") || optimized.code.contains("a[i];"),
        "{}",
        optimized.code
    );
}

#[test]
fn keeps_index_copies_that_are_reassigned_later() {
    let source = "function ext(t,a,r,s,o){var u=t[a];s&&!Array.isArray(u)?u=[]:!s&&!o(u)&&(u={});t[a]=e(u);return t}";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("u=t[a]") || optimized.code.contains("u=t[a];"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("e(u)") || optimized.code.contains("e(u)"),
        "{}",
        optimized.code
    );
}

#[test]
fn inlines_single_use_index_temps_in_a_fire_loop() {
    let optimized = optimize_generated_javascript(
        "function f(a,i,m){i++;for(;;i++){var t=i;if(i>=a.length){break}t=a[i];var c=m[0];!1===t.apply(c,m[1])&&(i=+a.length)}}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("a[i].apply(m[0],m[1])"),
        "{}",
        optimized.code
    );
}

#[test]
fn folds_trailing_return_this_into_a_comma() {
    let fire =
        optimize_generated_javascript("function fire(){a.fireWith(this,arguments);return this}")
            .unwrap();
    assert!(
        fire.code.contains("return a.fireWith(this,arguments),this"),
        "{}",
        fire.code
    );

    let add =
        optimize_generated_javascript("function add(){if(i){e&&u.push(e);n();e&&f()}return this}")
            .unwrap();
    assert!(
        add.code.contains("return i&&(") && add.code.contains(",this"),
        "{}",
        add.code
    );

    let unbound = optimize_generated_javascript(
        "function Deferred(){var r={};r.x=function(){var i=this==r?void 0:this;r.y(i,arguments);return this};return r}",
    )
    .unwrap();
    assert!(
        unbound.code.contains("return r.y(this==r?void 0:this,arguments),this")
            || unbound.code.contains("r.y(this==r?void 0:this,arguments)"),
        "{}",
        unbound.code
    );
    assert!(!unbound.code.contains("var i="), "{}", unbound.code);
}

#[test]
fn rematerializes_single_use_function_values_and_object_methods() {
    let deferred = optimize_generated_javascript(
        "let B=n=>{var r={id:n};return r};a.extend({Deferred:B,when(r){return r}})",
    )
    .unwrap();
    assert!(
        deferred.code.contains("Deferred(n){") && !deferred.code.contains("let B="),
        "{}",
        deferred.code
    );

    let callbacks = optimize_generated_javascript(
        "let Y=a=>{var e=[];return {add(){e.push(a)}}};a.Callbacks=Y",
    )
    .unwrap();
    assert!(
        callbacks.code.contains("a.Callbacks=a=>{") && !callbacks.code.contains("let Y="),
        "{}",
        callbacks.code
    );

    let called = optimize_generated_javascript("let G=e=>{var t=e+1;return t};use(G(2))").unwrap();
    assert!(called.code.contains("G="), "{}", called.code);

    let then_wrapper = optimize_generated_javascript(
        "function then(s,d){var J=d;s||(J=()=>{try{d()}catch(P){hook(P)}});return J}",
    )
    .unwrap();
    assert!(
        then_wrapper.code.contains("J=s?d:") && !then_wrapper.code.contains("J=d;s||"),
        "{}",
        then_wrapper.code
    );

    let console_cache = optimize_generated_javascript(
        "a.hook=(e,t)=>{var n=window.console;(n&&n.warn)&&e&&n.warn(e,t);return e}",
    )
    .unwrap();
    assert!(
        console_cache.code.contains("window.console")
            && !console_cache.code.contains("var n=window.console"),
        "{}",
        console_cache.code
    );

    let shadowed_inner = optimize_generated_javascript(
        "let F=(e,t)=>{t=t||[];return t};a.extend({makeArray:F,Deferred(n){var F=n[0];return F}})",
    )
    .unwrap();
    assert!(
        (shadowed_inner.code.contains("makeArray(e,t){")
            || shadowed_inner.code.contains("makeArray:(e,t)=>{"))
            && !shadowed_inner.code.contains("let F="),
        "{}",
        shadowed_inner.code
    );
}

#[test]
fn reuses_member_aliases_inside_unshadowed_nested_functions() {
    let when = optimize_generated_javascript(
        "var l=[],g=l.slice;a.extend({when(r){return l.slice.call(arguments)}})",
    )
    .unwrap();
    assert!(
        when.code.contains("g.call(arguments)") && !when.code.contains("l.slice.call"),
        "{}",
        when.code
    );

    let shadowed = optimize_generated_javascript(
        "var l=[],g=l.slice;function when(){var l=[1];return l.slice.call(arguments)}",
    )
    .unwrap();
    assert!(
        shadowed.code.contains("l.slice.call"),
        "{}",
        shadowed.code
    );

    let inner_arguments = optimize_generated_javascript(
        "function when(){var e=arguments.length;return function(p){return arguments.length>1?slice.call(arguments):p}}",
    )
    .unwrap();
    assert!(
        inner_arguments.code.contains("arguments.length>1"),
        "{}",
        inner_arguments.code
    );
}

#[test]
fn rematerializes_typeof_identifier_caches_when_the_operand_is_stable() {
    let thenable = optimize_generated_javascript(
        "function then(ge){var d=typeof ge;return \"object\"==d||\"function\"==d}",
    )
    .unwrap();
    assert!(
        thenable.code.contains("typeof ge") && !thenable.code.contains("var d="),
        "{}",
        thenable.code
    );

    let mutated = optimize_generated_javascript(
        "function f(n){var s=typeof n;return \"object\"==s||\"function\"==s?(n=\"x\",s):s}",
    )
    .unwrap();
    assert!(
        mutated.code.contains("var s=typeof n") || mutated.code.contains("s=typeof n"),
        "{}",
        mutated.code
    );
}

#[test]
fn drops_double_not_in_the_middle_of_a_boolean_chain() {
    let optimized =
        optimize_generated_javascript("function f(a,b,c){a&&!!b&&(c=1);return c}").unwrap();
    assert!(
        optimized.code.contains("a&&b&&") || optimized.code.contains("a&&b&&("),
        "{}",
        optimized.code
    );
    assert!(!optimized.code.contains("!!b"), "{}", optimized.code);

    let grouped =
        optimize_generated_javascript("function hook(n,e){(n&&!!n.warn)&&e&&n.warn(e)}").unwrap();
    assert!(
        grouped.code.contains("n&&n.warn&&e") || grouped.code.contains("n&&n.warn"),
        "{}",
        grouped.code
    );

    let window_console = optimize_generated_javascript(
        "a.hook=(e,t)=>{(window.console&&window.console.warn)&&e&&window.console.warn(e,t);return e}",
    )
    .unwrap();
    assert!(
        window_console.code.contains("window.console&&window.console.warn&&e")
            && !window_console.code.contains("(window.console&&window.console.warn)&&"),
        "{}",
        window_console.code
    );
    assert!(grouped.code.contains("n&&n.warn"), "{}", grouped.code);
    assert!(!grouped.code.contains("!!"), "{}", grouped.code);
}

#[test]
fn folds_same_lvalue_ternary_assigns() {
    let optimized = optimize_generated_javascript(
        "function when(d,p){arguments.length>1?i[d]=g.call(arguments):i[d]=p;return i}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("i[d]=arguments.length>1?g.call(arguments):p"),
        "{}",
        optimized.code
    );
}

#[test]
fn inlines_single_use_if_assigns_into_the_call() {
    let optimized = optimize_generated_javascript(
        "function each(C,s,i,o,t){if(s[5]){var P=i[3-C][2].disable;C=i[3-C][3].disable,u=i[0][2].lock,o.add(()=>{t=s[5]},P,C,u,i[0][3].lock)}o.add(s[3].fire);return o}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("s[5]&&o.add(")
            && optimized.code.contains("i[3-C][2].disable")
            && !optimized.code.contains("var P="),
        "{}",
        optimized.code
    );

    let text = optimize_generated_javascript(
        "function text(e){var t=e.nodeType;if(!t){for(var r,t=\"\",n=0;r=e[n++];)t+=a.text(r);return t}return t}",
    )
    .unwrap();
    assert!(
        text.code.contains("r=e[n++]") || text.code.contains("for("),
        "{}",
        text.code
    );
    assert!(
        !text.code.contains("for(var r,t=\"\",n=0;)"),
        "{}",
        text.code
    );
}

#[test]
fn rematerializes_a_single_use_object_after_aliasing_index_writes() {
    let optimized = optimize_generated_javascript(
        "let V={jquery:\"3.7.1\",length:0};a.fn=V;a.prototype=V;V[Symbol.iterator]=Array.prototype[Symbol.iterator];use(a)",
    )
    .unwrap();
    assert!(
        optimized.code.contains("a.fn=a.prototype={")
            && optimized.code.contains("a.fn[Symbol.iterator]")
            && !optimized.code.contains("let V="),
        "{}",
        optimized.code
    );

    let prototype = optimize_generated_javascript(
        "let V={jquery:\"3.7.1\",constructor:a,length:0,toArray(){return g.call(this)},get(e){return e==null?g.call(this):e<0?this[e+this.length]:this[e]},pushStack(e){e=w(this.constructor(),e),e.prevObject=this;return e},each(e){return d(this,e)},map(e){return this.pushStack(z(this,(t,n)=>e.call(t,n,t),null))},slice(){return this.pushStack(g.apply(this,arguments))},first(){return this.eq(0)},last(){return this.eq(-1)},even(){return this.pushStack(k(this,(e,t)=>(t+1)%2,!1))},odd(){return this.pushStack(k(this,(e,t)=>t%2,!1))},eq(e){var n=this.length,t=e<0&&n;e+=t;return this.pushStack(e>=0&&e<n?[this[e]]:[])},end(){return this.prevObject||this.constructor()},push:l.push,sort:l.sort,splice:l.splice,extend:j};a.fn=a.prototype=V;a.extend=j;a.fn[Symbol.iterator]=Array.prototype[Symbol.iterator];",
    )
    .unwrap();
    assert!(
        prototype.code.contains("a.fn=a.prototype={") && !prototype.code.contains("let V="),
        "{}",
        prototype.code
    );

    let emit_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/benchmarks/popular/build/jquery-layers/deferred/lilscript.raw.js"
    );
    if let Ok(emit) = std::fs::read_to_string(emit_path) {
        if emit.contains("let V=") {
            let again = optimize_generated_javascript(&emit).unwrap();
            assert!(
                !again.code.contains("let V="),
                "second peephole pass must rematerialize V: {}",
                &again.code[again.code.find("a.fn.init").unwrap_or(0)
                    ..again.code.find("a.extend=j").map(|i| i + 20).unwrap_or(again.code.len())]
            );
        }
    }
}

#[test]
fn folds_try_if_return_thenable_alternatives() {
    let optimized = optimize_generated_javascript(
        "function adopt(e,t,n,r){try{if(e){var E=e.promise;if(c(E)){E.call(e).done(t).fail(n);return}E=e.then;if(c(E)){E.call(e,t,n);return}}t.apply(void 0,[e].slice(r))}catch(v){n.apply(void 0,[v])}}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("e&&c(E=e.promise)")
            && optimized.code.contains("e&&c(E=e.then)")
            && !optimized.code.contains("if(c(E))"),
        "{}",
        optimized.code
    );
}

fn run_node(source: &str) -> String {
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
