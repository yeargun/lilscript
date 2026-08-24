use super::{
    late_generated_javascript_cleanup, late_generated_javascript_cleanup_local_variants,
    late_generated_javascript_cleanup_pass, optimize_generated_javascript,
    LateJavaScriptCleanupPass,
};

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

fn asserts_parses(code: &str) {
    let out = std::process::Command::new("node")
        .args(["-e", "new Function(process.argv[1])", code])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "invalid JS: {}\n{}",
        code,
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn keeps_return_separated_from_a_nullish_ternary() {
    let optimized = optimize_generated_javascript("function f(e){return e==null?!1:1}").unwrap();
    assert!(
        optimized.code.contains("return e") && !optimized.code.contains("returne"),
        "{}",
        optimized.code
    );
    asserts_parses(&optimized.code);
}

#[test]
fn keeps_return_separated_from_a_logical_ternary() {
    let optimized = optimize_generated_javascript(
        r#"function f(s,i,c){var a,t;arguments.length>1&&(a=i);arguments.length>2&&(t=c);return a&&typeof a.kind=="string"?1:0}"#,
    )
    .unwrap();
    assert!(
        (optimized.code.contains("return a") || optimized.code.contains("return i"))
            && !optimized.code.contains("returna")
            && !optimized.code.contains("returni"),
        "fused return: {}",
        optimized.code
    );
    asserts_parses(&optimized.code);
}

#[test]
fn splits_an_already_fused_return_identifier() {
    let optimized = optimize_generated_javascript("function f(e){returne==null?!1:1}").unwrap();
    assert!(
        optimized.code.contains("return e") && !optimized.code.contains("returne"),
        "{}",
        optimized.code
    );
    asserts_parses(&optimized.code);
}

#[test]
fn does_not_split_returned_as_a_function_name() {
    let optimized = optimize_generated_javascript("function returned(e){return e}").unwrap();
    assert!(
        optimized.code.contains("function returned"),
        "{}",
        optimized.code
    );
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

    let empty = "function m(a,b){var i=a.length,j=0;for(;j<b.length;j++)a[i]=b[j],i++;a.length=i;return a}console.log(JSON.stringify(m([],[]))+JSON.stringify(m([1],[])))";
    let folded_empty = optimize_generated_javascript(empty).unwrap();
    assert_eq!(run_node(&folded_empty.code).trim(), run_node(empty).trim());
    assert_eq!(run_node(&folded_empty.code).trim(), "[][1]");
}

#[test]
fn preserves_conditional_array_push() {
    let optimized = optimize_generated_javascript(
        "function eq(f){var i=this.length;g=[];f>=0&&f<i&&g.push(this[f]);return this.pushStack(g)}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("g.push(this[f])"),
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
fn cached_member_reads_do_not_drop_this_on_calls() {
    let optimized = optimize_generated_javascript(
        "function prepare(b){var a=this.enhancer_,d=this.value_;return this.enhancer_(b,d,this.name_)}",
    )
    .unwrap();
    assert!(
        optimized.code.contains(".enhancer_("),
        "cached method values must not become bare callees:\n{}",
        optimized.code
    );

    let original = "function run(){var r=this.dispose;this.dispose();return this.x}console.log(run.call({x:1,dispose(){this.x=2}}))";
    let folded = optimize_generated_javascript(original).unwrap();
    assert!(
        folded.code.contains(".dispose("),
        "dispose must stay a member call:\n{}",
        folded.code
    );
    assert_eq!(run_node(&folded.code).trim(), run_node(original).trim());
    assert_eq!(run_node(&folded.code).trim(), "2");
}

#[test]
fn preserves_unread_member_assignments_for_getter_effects() {
    let optimized = optimize_generated_javascript(
        "function prepare(a){var t=this.enhancer_,r=this.value_;a=this.enhancer_(a,r,this.name_),t=this.equals_;return this.equals_(r,a)}",
    )
    .unwrap();
    assert!(
        optimized.code.contains(".enhancer_(") && optimized.code.contains(".equals_("),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("t=this.enhancer_"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("t=this.equals_"),
        "{}",
        optimized.code
    );
}

#[test]
fn does_not_strip_member_reads_that_are_not_the_whole_assignment() {
    let optimized = optimize_generated_javascript(
        "function name(a){if(a.name){var t=a.name+\"\";return t}return \"x\"}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("name") && !optimized.code.contains("var+\""),
        "{}",
        optimized.code
    );
    assert_eq!(
        run_node(&format!(
            "{};console.log(name({{name:\"ok\"}}))",
            optimized.code
        ))
        .trim(),
        "ok"
    );
}

#[test]
fn keeps_member_temps_read_by_the_next_assignment_rhs() {
    let optimized = optimize_generated_javascript(
        "var t=function(){return \"spy\"};function get(a){var t=this.target_;t=g?!Ne.call(t,a):!1;if(t)this.has_(a);return this.target_[a]}",
    )
    .unwrap();
    asserts_parses(&optimized.code);
    assert!(
        optimized.code.contains("target_") && optimized.code.contains("Ne.call("),
        "{}",
        optimized.code
    );
    let result = run_node(&format!(
        "{};var g=true,Ne={{call:function(o,k){{return Object.prototype.hasOwnProperty.call(o,k)}}}};var o={{target_:{{a:1}},has_:function(){{}},get:get}};o.get(\"a\");console.log(typeof t+\",\"+o.get(\"a\"))",
        optimized.code
    ));
    assert_eq!(result.trim(), "function,1", "{result}\n{}", optimized.code);
}

#[test]
fn keeps_var_when_a_dead_member_temp_is_reassigned_and_returned() {
    let optimized =
        optimize_generated_javascript("function f(){var t=this.x;t=this.y;return t}").unwrap();
    asserts_parses(&optimized.code);
    let result = run_node(&format!(
        "var t=\"outer\";{};console.log(f.call({{x:1,y:2}})+\",\"+t)",
        optimized.code
    ));
    assert_eq!(result.trim(), "2,outer", "{result}\n{}", optimized.code);
}

#[test]
fn keeps_member_assignments_that_carry_state_across_loop_backedges() {
    let source = "function siblings(e){var n=[],k=0;while(e&&k++<4){n.push(e.value);e=e.nextSibling}return n.join(\",\")}";
    let optimized = optimize_generated_javascript(source).unwrap();
    asserts_parses(&optimized.code);
    assert!(
        optimized.code.contains("nextSibling"),
        "loop-carried update was dropped: {}",
        optimized.code
    );
    let result = run_node(&format!(
        "{};var b={{value:\"b\",nextSibling:null}},a={{value:\"a\",nextSibling:b}};console.log(siblings(a))",
        optimized.code
    ));
    assert_eq!(result.trim(), "a,b", "{result}\n{}", optimized.code);

    let unbraced = optimize_generated_javascript(
        "function walk(e){var k=0;while(e&&k++<4)e=e.nextSibling;return k}",
    )
    .unwrap();
    assert!(
        unbraced.code.contains("nextSibling"),
        "unbraced loop-carried update was dropped: {}",
        unbraced.code
    );
}

#[test]
fn keeps_a_statement_after_unbraced_if_when_dropping_a_dead_assign() {
    let optimized =
        optimize_generated_javascript("function f(a){if(a){var t=a.x;if(t)i=t.value}return 1}")
            .unwrap();
    asserts_parses(&optimized.code);
    assert!(!optimized.code.contains("if(t)}"), "{}", optimized.code);
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
fn keeps_multi_statement_index_walk_bodies_inside_the_loop() {
    let original = "function walk(s,l,n){var t=0,e;for(;!0;){e=s[t++];if(!e)break;if(n){if(e===n)continue}l.appendChild(e)}return l}";
    let folded = optimize_generated_javascript(original).unwrap();
    assert!(
        folded.code.contains("e=s[t++]")
            && folded.code.contains("){if(n)")
            && folded.code.contains("l.appendChild(e)}return"),
        "{}",
        folded.code
    );
    assert!(!folded.code.contains("e=s[t++];)if"), "{}", folded.code);
    let runtime = "let frag={nodes:[],appendChild(child){this.nodes.push(child);return child}};walk([{id:1},{id:2}],frag,null);console.log(frag.nodes.map(n=>n.id).join(','))";
    let original_run = format!("{original};{runtime}");
    let folded_run = format!("{};{runtime}", folded.code);
    assert_eq!(run_node(&folded_run).trim(), run_node(&original_run).trim());
    assert_eq!(run_node(&folded_run).trim(), "1,2");
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
fn defers_dependent_identifier_copies_until_the_source_rewrite_is_visible() {
    let source = "var defaults={},marked=(0,function(){}),slot;slot=defaults;Object.assign(slot,{gfm:!0});slot=marked;Object.assign(slot,{parse:marked});var exported=slot;console.log(exported===marked,exported.parse===marked)";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_node(&optimized.code),
        run_node(source),
        "{}",
        optimized.code
    );
    assert_eq!(
        run_node(&optimized.code),
        "true true\n",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_a_base_prototype_alias_for_unrecognized_set_prototype_factories() {
    let source = concat!(
        "let Z=()=>globalThis.Object;",
        "var a=globalThis.Symbol.toPrimitive,h;",
        "var oa=(0,function(g){this.g=g;return this});",
        "var ea=(0,function(e){oa.call(this,e);this.e=e;return this});",
        "h=ea.prototype;a=oa.prototype;Z().setPrototypeOf(h,a);",
        "h=ea.prototype;h.constructor=ea;h=ea.prototype;",
        "h.get=function(){return this.e};",
        "let value=new ea(7);console.log(value instanceof oa,value.get(),typeof a)",
    );
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(run_node(source), "true 7 object\n");
    assert_eq!(
        run_node(&optimized.code),
        "true 7 object\n",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_temp_computed_key_evaluation_order() {
    let optimized = optimize_generated_javascript(
        "function add(h,E){let i=\"[object \"+E+\"]\";h[i]=E.toLowerCase();return h}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("i=\"[object \"+E+\"]\"")
            && optimized.code.contains("h[i]=E.toLowerCase()"),
        "{}",
        optimized.code
    );

    let arrow = optimize_generated_javascript(
        "let add=(h,E)=>{let i=\"[object \"+E+\"]\";h[i]=E.toLowerCase()}",
    )
    .unwrap();
    assert!(
        arrow.code.contains("i=\"[object \"+E+\"]\"") && arrow.code.contains("h[i]="),
        "{}",
        arrow.code
    );
    assert!(arrow.code.contains("}"), "{}", arrow.code);

    let order = "function assign(h,other){let i=(h=other,'x');h[i]=1;return other.x}console.log(assign({},{}))";
    let order_optimized = optimize_generated_javascript(order).unwrap();
    assert_eq!(run_node(&order_optimized.code).trim(), "1");
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
fn does_not_comma_join_continue_into_a_switch_case() {
    let source =
        "function f(a){for(;;)switch(a){case 0:a=1;continue;case 1:return 2}}console.log(f(0))";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("continue,") && !optimized.code.contains("return 2,case"),
        "{}",
        optimized.code
    );
    assert_eq!(run_node(&optimized.code).trim(), "2");
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
fn preserves_length_reads_in_for_conditions() {
    let each = optimize_generated_javascript(
        "function each(j,t){for(var n=j.length,r=0;r<j.length&&t(j[r]);r++);return j}",
    )
    .unwrap();
    assert!(each.code.contains("r<j.length&&"), "{}", each.code);

    let grep = optimize_generated_javascript(
        "function g(j,t,r){var n=[],i=j.length,o=!r;for(r=0;r<j.length;r++)n.push(j[r]);return n}",
    )
    .unwrap();
    assert!(grep.code.contains("r<j.length"), "{}", grep.code);

    let prefix = optimize_generated_javascript(
        "let v=(i,q,t)=>{var b=[],d=i.length,p=!t;for(t=0;t<i.length;++t)!q(i[t],t)!=p&&b.push(i[t]);return b}",
    )
    .unwrap();
    assert!(prefix.code.contains("t<i.length"), "{}", prefix.code);
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
fn keeps_identifier_copies_across_nested_function_source_writes() {
    let source = "function f(){var value=1;return function(){var oldValue=value;g(function(){value=2});return[value,oldValue]}}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!(
        "function g(fn){{fn()}}\n{}\nconsole.log(JSON.stringify(f()()));",
        optimized.code
    );
    let original = format!("function g(fn){{fn()}}\n{source}\nconsole.log(JSON.stringify(f()()));");
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "[2,1]");
}

#[test]
fn keeps_identifier_copies_across_sibling_closure_calls() {
    let source = "function f(){var n=1;var t=function(){n=2};var c=function(fn){fn()};var o=n;c(t);return[n,o]}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("{}\nconsole.log(JSON.stringify(f()));", optimized.code);
    let original = format!("{source}\nconsole.log(JSON.stringify(f()));");
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "[2,1]");
    assert!(
        optimized.code.contains("o=") || optimized.code.contains("var o"),
        "snapshot copy must survive a sibling closure call: {}",
        optimized.code
    );

    let nested = "(function(){var n=1;let t=(0,function(){n=2}),c=(0,function(fn){fn()});(0,function(){var o=n;c(t);consume(o,n)})()})()";
    let nested_opt = optimize_generated_javascript(nested).unwrap();
    let nested_script = format!(
        "function consume(a,b){{console.log(JSON.stringify([a,b]))}}\n{}",
        nested_opt.code
    );
    assert_eq!(run_node(&nested_script).trim(), "[1,2]");
}

#[test]
fn still_copies_unmutated_locals_across_calls() {
    let optimized =
        optimize_generated_javascript("function f(){var n=1;var o=n;g();return o}").unwrap();
    assert!(
        !optimized.code.contains("var o") && !optimized.code.contains("o="),
        "unmutated local copies may fold across calls: {}",
        optimized.code
    );
}

#[test]
fn still_copies_unassigned_module_function_aliases_across_calls() {
    let source = "function action(n,fn){return fn}function run(gen,val){var C=action;return next(C.call(void 0,\"s\",gen.next),gen,val)}";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("var C") && !optimized.code.contains("C=action"),
        "module function aliases may fold across calls: {}",
        optimized.code
    );
}

#[test]
fn keeps_call_temps_across_reassignment_of_a_callee_in_the_rhs() {
    let source = "function Q(n,s,a){return n.call(s,a)}function F(){}function action(n,fn){return fn}function run(W,i,o){var C=\"s\",V=action;var ret=Q(V.call(F(),C,i.next),i,o);V=W;return Q(V,F(),ret)}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!(
        "{}\nfunction nextStep(ret){{return ret.value}}\nconst gen={{next:function(x){{return {{value:x,done:true}}}}}}\nconsole.log(run(nextStep,gen,7));",
        optimized.code
    );
    let original = format!(
        "{source}\nfunction nextStep(ret){{return ret.value}}\nconst gen={{next:function(x){{return {{value:x,done:true}}}}}}\nconsole.log(run(nextStep,gen,7));"
    );
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "7");
}

#[test]
fn keeps_captured_identifier_assign_from_a_nested_function() {
    let source = "function f(){var rejector;function inner(rejector2){rejector=rejector2}function outer(){return rejector}return[inner,outer]}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!(
        "{}\nconst p=f();p[0](7);console.log(p[1]());",
        optimized.code
    );
    let original = format!("{source}\nconst p=f();p[0](7);console.log(p[1]());");
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "7");
}

#[test]
fn keeps_member_copies_across_intervening_calls() {
    let source = "function f(){this.x=1;this.mutate=function(){this.x=2};var was=this.x;this.mutate();return was}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("{}\nconsole.log(f.call({{}}));", optimized.code);
    let original = format!("{source}\nconsole.log(f.call({{}}));");
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "1");
    assert!(
        optimized.code.contains("was=") || optimized.code.contains("var was"),
        "member copy must survive an intervening call: {}",
        optimized.code
    );

    let helper = optimize_generated_javascript(
        "function f(){this.x=1;this.compute=function(){this.x=2;return 3};var was=this.x;var m=this.compute;call1(m,this,true);return was}",
    )
    .unwrap();
    let helper_script = format!(
        "function call1(fn,self){{return fn.call(self)}}\n{}\nconsole.log(f.call({{}}));",
        helper.code
    );
    assert_eq!(run_node(&helper_script).trim(), "1");
}

#[test]
fn keeps_call_argument_member_copies_across_intervening_calls() {
    let source = "function f(){this.x=1;this.compute=function(){this.x=2;return 3};var v9=this.x;var v12=this.compute;var v15=call1(v12,this,true);if(toInt(v9)==1)return 1;return 0}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!(
        "function call1(fn,self){{return fn.call(self)}}function toInt(v){{return v|0}}\n{}\nconsole.log(f.call({{}}));",
        optimized.code
    );
    assert_eq!(run_node(&script).trim(), "1");
    assert!(
        optimized.code.contains("v9=") || optimized.code.contains("var v9"),
        "call-argument member copy must survive an intervening call: {}",
        optimized.code
    );
}

#[test]
fn keeps_call_argument_member_copies_after_the_receiver_is_rebound() {
    let source = "function f(b,a,i){var d=b.href;b=b.title;var e=a[0]+\"\";return i(a,d,b,e)}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!(
        "{}\nconsole.log(f({{href:\"/url\",title:\"title\"}},[\"x\"],(cap,href,title)=>href+\"|\"+title));",
        optimized.code
    );
    let original = format!(
        "{source}\nconsole.log(f({{href:\"/url\",title:\"title\"}},[\"x\"],(cap,href,title)=>href+\"|\"+title));"
    );
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "/url|title");
    assert!(
        !optimized.code.contains("b.href")
            || optimized
                .code
                .find("b=b.title")
                .is_none_or(|assign| optimized
                    .code
                    .find("b.href")
                    .is_some_and(|href| href < assign)),
        "must not rematerialize obj.href after obj was rebound:\n{}",
        optimized.code
    );
}

#[test]
fn rematerializes_stable_receiver_call_argument_members() {
    let source = "function f(b,i){var d=b.href;return i(d,b.title)}";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("b.href") && optimized.code.contains("b.title"),
        "a stable receiver should keep direct JS members:\n{}",
        optimized.code
    );
    let script = format!(
        "{}\nconsole.log(f({{href:\"/url\",title:\"title\"}},(href,title)=>href+\"|\"+title));",
        optimized.code
    );
    assert_eq!(run_node(&script).trim(), "/url|title");
}

#[test]
fn keeps_self_based_member_assignments_as_value_updates() {
    let source = "function f(e,g){var c=e[0];c=c[2];c=c.disable;return g(c)}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let original =
        format!("{source}\nconsole.log(f([[0,0,{{disable:function ok(){{}}}}]],x=>x.name));");
    let script = format!(
        "{}\nconsole.log(f([[0,0,{{disable:function ok(){{}}}}]],x=>x.name));",
        optimized.code
    );
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "ok");
    assert!(
        !optimized.code.contains("disable.disable"),
        "self-based member copies must not recursively expand: {}",
        optimized.code
    );
}

#[test]
fn single_use_function_inlining_does_not_capture_a_free_helper() {
    let source =
        "function build(S,D){let r=(r,t)=>t[r],t=t=>r(D,t),n=t=>r(S,t);return r=>n(r)||t(r)}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let original = format!("{source}\nconsole.log(build('user','priv')({{user:1,priv:2}}));");
    let script = format!(
        "{}\nconsole.log(build('user','priv')({{user:1,priv:2}}));",
        optimized.code
    );
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "1");
}

#[test]
fn keeps_object_literal_across_intervening_calls() {
    let source = "function f(){this.x=1;this.mutate=function(){this.x=2};var snapshot={wasSuspended:this.x==1};this.mutate();if(snapshot.wasSuspended)return 1;return 0}";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("{}\nconsole.log(f.call({{}}));", optimized.code);
    let original = format!("{source}\nconsole.log(f.call({{}}));");
    assert_eq!(run_node(&script).trim(), run_node(&original).trim());
    assert_eq!(run_node(&script).trim(), "1");
    assert!(
        optimized.code.contains("snapshot=")
            && !optimized.code.contains("return{wasSuspended")
            && !optimized.code.contains("if({wasSuspended"),
        "object initializer must stay assigned before mutate: {}",
        optimized.code
    );
}

#[test]
fn preserves_repeated_member_reads_for_getter_effects() {
    let text = optimize_generated_javascript(
        "function text(t){var r=t.nodeType;if(!t.nodeType){return\"\"}return r}",
    )
    .unwrap();
    assert!(
        text.code.contains("!t.nodeType") && text.code.contains("return"),
        "{}",
        text.code
    );
    assert!(text.code.matches("nodeType").count() >= 2, "{}", text.code);

    let grep = optimize_generated_javascript(
        "let i=z=>z,l=(z,t,r)=>{var n=[],i=z.length,o=!r;for(r=0;r<z.length;r++)n.push(z[r]);return n}",
    )
    .unwrap();
    assert!(grep.code.contains("r<z.length"), "{}", grep.code);
}

#[test]
fn preserves_unused_member_copy_declarators_for_getter_effects() {
    let optimized =
        optimize_generated_javascript("let q=k.push,T=k.sort;k={push:k.push,sort:k.sort}").unwrap();
    assert!(optimized.code.contains("q=k.push"), "{}", optimized.code);
    assert!(optimized.code.contains("T=k.sort"), "{}", optimized.code);
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
    assert!(
        used_before.code.contains("q=k.push"),
        "{}",
        used_before.code
    );
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
fn preserves_inferred_names_for_single_use_function_values() {
    let noop = optimize_generated_javascript("let H=()=>{},o=1;k={noop:H,id:o}").unwrap();
    assert!(
        noop.code.contains("H=()=>{}") && noop.code.contains("noop:H"),
        "{}",
        noop.code
    );

    let reflected_source = concat!(
        "let H=()=>{};let k={noop:H};",
        "console.log(H.name+'|'+k.noop.name+'|'+Object.hasOwn(H,'prototype'))"
    );
    let reflected = optimize_generated_javascript(reflected_source).unwrap();
    assert!(reflected.code.contains("H=()=>{}"), "{}", reflected.code);
    assert_eq!(run_node(reflected_source).trim(), "H|H|false");
    assert_eq!(
        run_node(&reflected.code).trim(),
        "H|H|false",
        "{}",
        reflected.code
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
fn preserves_prototype_tostring_lookup_and_evaluation_points() {
    let object = optimize_generated_javascript(
        "let K=a=>k[Object.prototype.toString.call(a)];var k={},C=k.toString",
    )
    .unwrap();
    assert!(
        object.code.contains("Object.prototype.toString.call(a)"),
        "{}",
        object.code
    );

    let function = optimize_generated_javascript(
        "let q=a=>D.call(a)==w;var w=Function.prototype.toString.call(Object),k={},t=k.hasOwnProperty,D=t.toString",
    )
    .unwrap();
    assert!(
        function.code.contains("D.call(a)==w")
            && function
                .code
                .contains("Function.prototype.toString.call(Object)")
            && function.code.contains("w="),
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
            || later_alias
                .code
                .contains("t.toString.call(a)==D.call(Object)"),
        "{}",
        later_alias.code
    );

    let eager_alias = optimize_generated_javascript(
        "function f(){var E=void 0;return E}var E=D.call(Object),k={},t=k.hasOwnProperty,D=t.toString;let q=a=>D.call(a)==E",
    )
    .unwrap();
    assert!(
        eager_alias.code.contains("E=D.call(Object)"),
        "{}",
        eager_alias.code
    );
    assert!(
        !eager_alias
            .code
            .contains("Function.prototype.toString.call(Object)"),
        "{}",
        eager_alias.code
    );

    let throwing = concat!(
        "var outcome;try{var value=missing.call(Object),holder={},",
        "method=holder.hasOwnProperty,missing=method.toString;outcome='no-throw'}",
        "catch(error){outcome=error.name}console.log(outcome)"
    );
    let throwing_optimized = optimize_generated_javascript(throwing).unwrap();
    assert_eq!(run_node(throwing).trim(), "TypeError");
    assert_eq!(run_node(&throwing_optimized.code).trim(), "TypeError");
}

#[test]
fn preserves_repeated_member_assignment_order() {
    let optimized = optimize_generated_javascript("k={};j.fn=k;j.prototype=k;j.extend=c").unwrap();
    assert!(
        optimized.code.contains("j.fn=k") && optimized.code.contains("j.prototype=k"),
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
fn keeps_void_reset_read_by_the_reassign_rhs() {
    let direct = optimize_generated_javascript(
        "function f(c){var x={bad:1};x=void 0;x=c?function(){return 1}:x;return x===void 0}",
    )
    .unwrap();
    assert_eq!(
        run_node(&format!("{};console.log(f(!1))", direct.code)),
        "true\n",
        "{}",
        direct.code
    );

    // A `var` declarator inside a loop is also an executable reset on every
    // iteration; reducing it to `var x` would retain the preceding value.
    let loop_reset = optimize_generated_javascript(
        "function f(c){var x=1;for(var i=0;i<2;i++){var x=void 0;x=c[i]?2:x;if(i==1)return x===void 0}}",
    )
    .unwrap();
    assert_eq!(
        run_node(&format!("{};console.log(f([!0,!1]))", loop_reset.code)),
        "true\n",
        "{}",
        loop_reset.code
    );
}

#[test]
fn keeps_member_factory_copy_before_self_reassign_call() {
    let source = concat!(
        "let O={Callbacks:x=>({value:x,add(fn){fn()}})};",
        "function f(F,t,n,se,M,le){var e='fx';'string'==typeof n&&(e=n);",
        "n=e+'queueHooks';var r=F(t,n);if(r)return r;",
        "r=O.Callbacks,r=r('once memory'),",
        "r.add(()=>{se(M,t,[e+'queue',n])});return le(M,t,n,{empty:r})}",
        "console.log(f(()=>null,0,0,()=>{},0,(M,t,n,x)=>x.empty).value)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert_eq!(
        run_node(&optimized.code).trim(),
        "once memory",
        "{}",
        optimized.code
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
fn preserves_single_use_index_chain_snapshots() {
    let optimized = optimize_generated_javascript(
        "function then(i,e,A,F){var o=i[0][3];o.add(e(0,A,F,A.notifyWith));i[1][3].add(e(0,A,F,null))}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("o=i[0][3]") && optimized.code.contains("o.add("),
        "{}",
        optimized.code
    );

    let key = optimize_generated_javascript(
        "function each(s,r,o){var F=s[0];r[F]=function(){return r[s[0]+\"With\"](this)},r[s[0]+\"With\"]=o.fireWith;return r}",
    )
    .unwrap();
    assert!(
        key.code.contains("F=s[0]") && key.code.contains("r[F]="),
        "{}",
        key.code
    );

    let thenable = optimize_generated_javascript(
        "function when(r,i,t,U,n){if(e<=1&&(q(r,n.resolve,n.reject,!e),r=i[t],\"pending\"==n.state()||U(r&&r.then)))return n.then();return n}",
    )
    .unwrap();
    assert!(
        thenable.code.contains("r=i[t]") && thenable.code.contains("r&&r.then"),
        "{}",
        thenable.code
    );
}

#[test]
fn rematerializes_nested_single_use_literals_and_expression_calls() {
    let regex =
        optimize_generated_javascript("let A=/x/g;function f(e){return e.match(A)}").unwrap();
    assert!(
        regex.code.contains("e.match(/x/g)") && !regex.code.contains("A="),
        "{}",
        regex.code
    );

    let object =
        optimize_generated_javascript("let N={type:!0};function f(){for(var i in N)return i}")
            .unwrap();
    assert!(
        object.code.contains("N={type:!0}") && object.code.contains("for(var i in N)"),
        "a nested function may run repeatedly, so its object must retain one shared identity: {}",
        object.code
    );

    let document =
        optimize_generated_javascript("var T=document;function f(n){n=n||T;return n}").unwrap();
    assert!(
        document.code.contains("n=n||document") && !document.code.contains("T="),
        "{}",
        document.code
    );

    let ty = optimize_generated_javascript("let M=e=>e+1;function f(i){return M(i)}").unwrap();
    assert!(
        (ty.code.contains("(e=>e+1)(i)") || ty.code.contains("return i+1"))
            && !ty.code.contains("M="),
        "{}",
        ty.code
    );

    let factory = optimize_generated_javascript("let G=e=>{var t=e+1;return t};use(G(2))").unwrap();
    assert!(
        factory.code.contains("use((e=>{") && !factory.code.contains("G="),
        "{}",
        factory.code
    );

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
        nonce.code.contains("r={}") && nonce.code.contains("r.nonce=t&&t.nonce"),
        "{}",
        nonce.code
    );

    let nonce_iife = optimize_generated_javascript(
        "function globalEval(e,t,n){var r={};r.nonce=t&&t.nonce,((e,t,n)=>{var r=n.createElement(\"script\");r.text=e})(e,r,n)}",
    )
    .unwrap();
    assert!(
        nonce_iife.code.contains("r={}") && nonce_iife.code.contains("r.nonce=t&&t.nonce"),
        "{}",
        nonce_iife.code
    );

    let later_regex =
        optimize_generated_javascript("function f(e){return e.match(A)}var A=/x/g;").unwrap();
    assert!(
        later_regex.code.contains("e.match(/x/g)") && !later_regex.code.contains("A="),
        "{}",
        later_regex.code
    );

    let later_object =
        optimize_generated_javascript("let P=()=>{for(var i in N)return i},N={type:!0};").unwrap();
    assert!(
        later_object.code.contains("N={type:!0}") && later_object.code.contains("for(var i in N)"),
        "a reusable arrow must observe one shared object rather than allocate per call: {}",
        later_object.code
    );

    let later_document =
        optimize_generated_javascript("let P=(n)=>{n=n||R;return n};var R=document;").unwrap();
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
fn preserves_single_use_predicate_ternary_evaluation_points() {
    let optimized = optimize_generated_javascript(
        "function then(z,h,i,e,A){var F=c(z)?z:h;i[0][3].add(e(0,A,F,A.notifyWith))}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("F=c(z)?z:h") && optimized.code.contains("e(0,A,F,A.notifyWith)"),
        "{}",
        optimized.code
    );
    assert!(optimized.code.contains("var F="), "{}", optimized.code);

    let assigned = optimize_generated_javascript(
        "function then(r,s,h,f,i,e,A){var C,G;i[0][3].add(e(0,A,c(z)?z:h,A.notifyWith)),C=c(r)?r:h,i[1][3].add(e(0,A,C,null)),G=c(s)?s:f,i[2][3].add(e(0,A,G,null))}",
    )
    .unwrap();
    assert!(
        assigned.code.contains("C=c(r)?r:h")
            && assigned.code.contains("e(0,A,C,null)")
            && assigned.code.contains("G=c(s)?s:f")
            && assigned.code.contains("e(0,A,G,null)"),
        "{}",
        assigned.code
    );
    let calls = optimize_generated_javascript(
        "function then(e,t,r,h,s,f,l,ge){++t;var me=e(t,r,h,s),he=e(t,r,f,s);l.call(ge,me,he,e(t,r,h,r.notifyWith))}",
    )
    .unwrap();
    assert!(
        calls.code.contains("me=e(t,r,h,s)")
            && calls.code.contains("he=e(t,r,f,s)")
            && calls
                .code
                .contains("l.call(ge,me,he,e(t,r,h,r.notifyWith))"),
        "{}",
        calls.code
    );

    let method = optimize_generated_javascript(
        "function when(n,s,e){var E=n.done(s(e)),u=E.resolve;use(u)}",
    )
    .unwrap();
    assert!(
        method.code.contains("E=n.done(s(e))")
            && method.code.contains("u=E.resolve")
            && method.code.contains("use(u)"),
        "{}",
        method.code
    );
}

#[test]
fn preserves_copied_receiver_snapshots_and_false_phi() {
    let optimized = optimize_generated_javascript(
        "function hook(n,e,C){var r;(n&&n.warn)&&e?(r=C,r=r.test(e.name)):r=!1,r&&n.warn(e)}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("r=C,r=r.test(e.name)"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("r=!1") && optimized.code.contains("var r"),
        "{}",
        optimized.code
    );

    let remat = optimize_generated_javascript(
        "var C=/x/;function hook(n,e){var r;(n&&n.warn)&&e?(r=C,r=r.test(e.name)):r=!1,r&&n.warn(e)}",
    )
    .unwrap();
    assert!(
        remat.code.contains("var C=/x/") && remat.code.contains("r=C,r=r.test(e.name)"),
        "{}",
        remat.code
    );

    let outer = optimize_generated_javascript(
        "var p={};function then(J){!hook||(p=J,p.error=hook());return J}",
    )
    .unwrap();
    assert!(
        outer.code.contains("p=J,p.error=") && !outer.code.contains("J.error="),
        "{}",
        outer.code
    );
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
    assert!(
        deferred_when.code.contains("--e||"),
        "{}",
        deferred_when.code
    );

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
    let flipped = optimize_generated_javascript("function f(e){return e.disabled===!1}").unwrap();
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
        cache
            .code
            .contains("(1==a.nodeType||9==a.nodeType||!a.nodeType)&&")
            || cache
                .code
                .contains("(1==a.nodeType||9==a.nodeType||!a.nodeType) &&"),
        "{}",
        cache.code
    );
    assert!(
        !cache.code.contains("||!a.nodeType&&")
            && !cache.code.contains("||9==a.nodeType||!a.nodeType&&"),
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
fn beta_reduced_arrow_iife_keeps_low_precedence_body_grouped() {
    let reduced =
        optimize_generated_javascript("function f(c){if(!(e=>tag(e)&&e===e)(c))return 1;return 0}")
            .unwrap();
    assert!(
        reduced.code.contains("!(tag(c)&&c===c)"),
        "{}",
        reduced.code
    );
    assert!(!reduced.code.contains("!tag(c)&&c===c"), "{}", reduced.code);
}

#[test]
fn reduces_zero_argument_return_only_function_iifes() {
    let reduced = late_generated_javascript_cleanup_pass(
        "var api=(function(){return{run:work,stop:halt}})();use(api)",
        LateJavaScriptCleanupPass::ZeroArgumentReturnIife,
    )
    .unwrap();
    assert!(reduced.contains("{run:work,stop:halt}"), "{reduced}");
    assert!(!reduced.contains("function(){return"), "{reduced}");

    for source in [
        "var value=(function(){return this.value})()",
        "var value=(function(){return arguments.length})()",
        "var value=(function(){return new.target})()",
        "var value=(function(){return()=>1})()",
    ] {
        let kept = late_generated_javascript_cleanup_pass(
            source,
            LateJavaScriptCleanupPass::ZeroArgumentReturnIife,
        )
        .unwrap();
        assert!(kept.contains("function"), "{source} -> {kept}");
    }
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
fn does_not_fold_ident_ternary_into_an_unparenthesized_or_assignment() {
    let optimized = optimize_generated_javascript(
        "function nameOf(m){var a=isObj(m)?!0:isMap(m);a?a:a=isSet(m)?adm(m):atom(m);return a}",
    )
    .unwrap();
    assert!(
        !optimized.code.contains("||a=") && !optimized.code.contains("||a ="),
        "{}",
        optimized.code
    );
    let parsed = std::process::Command::new("node")
        .arg("-e")
        .arg("Function(process.argv[1])")
        .arg("--")
        .arg(&optimized.code)
        .output()
        .expect("Node.js is required");
    assert!(
        parsed.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&parsed.stderr),
        optimized.code
    );
}

#[test]
fn parenthesizes_or_followed_by_an_assignment() {
    let optimized = optimize_generated_javascript(
        "function nameOf(m){var a=isObj(m)?!0:isMap(m);a||a=isSet(m)?adm(m):atom(m);return a}",
    )
    .unwrap();
    assert!(
        !optimized.code.contains("||a=") && optimized.code.contains("||("),
        "{}",
        optimized.code
    );
    let parsed = std::process::Command::new("node")
        .arg("-e")
        .arg("Function(process.argv[1])")
        .arg("--")
        .arg(&optimized.code)
        .output()
        .expect("Node.js is required");
    assert!(
        parsed.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&parsed.stderr),
        optimized.code
    );
}

#[test]
fn parenthesizes_and_followed_by_an_assignment() {
    let optimized = optimize_generated_javascript(
        "function nameOf(e,a,r){var i;if(!e||a){i=r.name_}observe(i);return i}",
    )
    .unwrap();
    assert!(
        !optimized.code.contains("&&i=") && !optimized.code.contains("&&i ="),
        "{}",
        optimized.code
    );
    let parsed = std::process::Command::new("node")
        .arg("-e")
        .arg("Function(process.argv[1])")
        .arg("--")
        .arg(&optimized.code)
        .output()
        .expect("Node.js is required");
    assert!(
        parsed.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&parsed.stderr),
        optimized.code
    );

    let already_and = late_generated_javascript_cleanup(
        "function nameOf(e,a,r){var i;(!e||a)&&i=r.name_,observe(i);return i}",
    )
    .unwrap();
    assert!(
        !already_and.contains("&&i=") && already_and.contains("&&("),
        "{already_and}"
    );
    let parsed_and = std::process::Command::new("node")
        .arg("-e")
        .arg("Function(process.argv[1])")
        .arg("--")
        .arg(&already_and)
        .output()
        .expect("Node.js is required");
    assert!(
        parsed_and.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&parsed_and.stderr),
        already_and
    );

    let member_or = late_generated_javascript_cleanup(
        "function asObject(e){var m=e[U];m.proxy_||m.proxy_=new Proxy(e,h);return m.proxy_}",
    )
    .unwrap();
    assert!(
        !member_or.contains("||m.proxy_=") && member_or.contains("||("),
        "{member_or}"
    );
    let parsed_member = std::process::Command::new("node")
        .arg("-e")
        .arg("Function(process.argv[1])")
        .arg("--")
        .arg(&member_or)
        .output()
        .expect("Node.js is required");
    assert!(
        parsed_member.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&parsed_member.stderr),
        member_or
    );
}

#[test]
fn declares_an_unbound_name_moved_into_for_init() {
    let optimized = optimize_generated_javascript(
        "function walk(a){r=a.target;for(;;){if(!r)break;r=r.proto}return r}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("for(var r=")
            || optimized.code.contains("var r=")
            || optimized.code.contains("var r;"),
        "{}",
        optimized.code
    );
    let script = format!(
        "\"use strict\";{}\nconsole.log(walk({{target:null}}));",
        optimized.code
    );
    assert_eq!(run_node(&script).trim(), "null");
}

#[test]
fn does_not_var_shadow_outer_bindings_folded_into_for_init() {
    let source = "function factory(ls){var os,ns,ss,ts,es=[],rs=[],is=-1;var us=()=>{ts=ts||ls.once;ss=os=!0;for(;rs.length;is=-1){ns=rs.shift();for(;++is<es.length;)es[is].apply(ns[0],ns[1])}os=!1};return {add(fn){es.push(fn);return this},fireWith(ctx,args){ts||(rs.push([ctx,args]),os||us());return this},fired:()=>!!ss}}var c=factory({});c.add(function(a,b){});c.fireWith({ok:1},[1,2]);console.log([c.fired()].join())";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        !optimized.code.contains("for(var ts=")
            && !optimized.code.contains("for(var ss=")
            && !optimized.code.contains("for(var ns="),
        "{}",
        optimized.code
    );
    assert_eq!(run_node(source).trim(), "true");
    assert_eq!(run_node(&optimized.code).trim(), "true");
}

#[test]
fn declares_implicit_loop_temps_in_strict_mode() {
    let optimized = optimize_generated_javascript(
        "function walk(a){for(r=a.target;;){k=r&&r.proto;if(!k)break;r=k}return r}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("for(var r=")
            || optimized.code.contains("var r=")
            || optimized.code.contains("var r;"),
        "{}",
        optimized.code
    );
    assert!(
        optimized.code.contains("var k=")
            || optimized.code.contains("var k,")
            || optimized.code.contains("var k;"),
        "{}",
        optimized.code
    );
    let script = format!(
        "\"use strict\";{}\nconsole.log(walk({{target:null}}));",
        optimized.code
    );
    assert_eq!(run_node(&script).trim(), "null");
}

#[test]
fn does_not_var_shadow_module_bindings() {
    let source = "var pred;function set(name,value){if(name==\"x\"){pred=value;return}pred=null}set(\"x\",function(){return 1});console.log(pred())";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(run_node(&optimized.code).trim(), "1");
    assert!(
        !optimized.code.contains("var pred=value") && !optimized.code.contains("var pred;pred="),
        "{}",
        optimized.code
    );
}

#[test]
fn does_not_var_shadow_module_bindings_in_expression_guards() {
    let keep = "var canMerge=true;var state=(function(){var count=0;count&&(canMerge=false);if(!canMerge)return\"isolated\";return\"merged\"})();console.log(state+\":\"+canMerge)";
    let flip = "var canMerge=true;var state=(function(){var count=1;count&&(canMerge=false);if(!canMerge)return\"isolated\";return\"merged\"})();console.log(state+\":\"+canMerge)";
    let keep_out = optimize_generated_javascript(keep).unwrap();
    let flip_out = optimize_generated_javascript(flip).unwrap();
    assert!(
        !keep_out.code.contains("var canMerge;"),
        "{}",
        keep_out.code
    );
    assert_eq!(
        run_node(&keep_out.code).trim(),
        "merged:true",
        "{}",
        keep_out.code
    );
    assert_eq!(
        run_node(&flip_out.code).trim(),
        "isolated:false",
        "{}",
        flip_out.code
    );
}

#[test]
fn does_not_var_shadow_module_bindings_in_comma_sequences() {
    let source = "var gs={old:1};function isolate(flag){flag&&(gs={neu:1});return gs}var next=isolate(true);console.log(next.neu+\":\"+gs.neu)";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert_eq!(
        run_node(&optimized.code).trim(),
        "1:1",
        "{}",
        optimized.code
    );
}

#[test]
fn shadows_module_function_helpers_used_as_inner_temps() {
    let source = "function k(a,b,c){return a.call(b,c)}function step(flag){k=!!flag;return k}console.log(step(true));console.log(typeof k)";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("\"use strict\";{}", optimized.code);
    assert_eq!(run_node(&script).trim(), "true\nfunction");
}

#[test]
fn shadows_module_function_helpers_used_as_comma_sequence_temps() {
    let source = "function K(){return 7}function step(Pk,Jk,ck){function nested(){var P;P=arguments,K=Pk[0],Jk[K+ck](this,P,K)}nested(3)}var seen;step(['done'],{done(self,args,key){seen=args[0]+':'+key}},'');console.log(seen);console.log(K())";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("\"use strict\";{}", optimized.code);
    assert_eq!(run_node(&script).trim(), "3:done\n7", "{}", optimized.code);
    assert!(
        optimized.code.contains("var K;") || optimized.code.contains("var K,"),
        "{}",
        optimized.code
    );
}

#[test]
fn declares_implicit_temps_after_a_switch_case() {
    let source = "function k(a,b,c){return a.call(b,c)}function step(flag){for(;;){switch(0){case 0:k=!!flag;return k}}}console.log(step(true));console.log(typeof k)";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("\"use strict\";{}", optimized.code);
    assert_eq!(
        run_node(&script).trim(),
        "true\nfunction",
        "{}",
        optimized.code
    );
}

#[test]
fn declares_implicit_temps_in_chained_assignment() {
    let source = "function k(a,b,c){return a.call(b,c)}function step(flag){for(;;){switch(0){case 0:flag=k=!!flag;return k}}}console.log(step(true));console.log(typeof k)";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("\"use strict\";{}", optimized.code);
    assert_eq!(run_node(&script).trim(), "true\nfunction");
}

#[test]
fn chained_assignment_temps_do_not_clobber_module_vars() {
    let source = "var l=function(){return 1};function step(n2){for(;;){switch(0){case 0:n2=l=!!n2[2];return n2}}}console.log(step([0,0,true]));console.log(typeof l)";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("\"use strict\";{}", optimized.code);
    assert_eq!(
        run_node(&script).trim(),
        "true\nfunction",
        "{}",
        optimized.code
    );
}

#[test]
fn chained_assignments_reuse_captured_enclosing_function_bindings() {
    let source =
        "function transport(){var b,a=null;return{send(){prop({src:url});a=b=function(){return 1};return b()}}}var url=\"x\";function prop(){};console.log(transport().send())";
    let optimized = optimize_generated_javascript(source).unwrap();
    let script = format!("\"use strict\";{}", optimized.code);
    assert_eq!(run_node(&script).trim(), "1", "{}", optimized.code);
    assert!(
        !optimized.code.contains("src:url}var b;"),
        "{}",
        optimized.code
    );
}

#[test]
fn keeps_a_narrowed_value_distinct_from_its_guard_condition() {
    let optimized = optimize_generated_javascript(
        "function fallback(current){if(null!=current)return current;return 0}",
    )
    .unwrap();

    assert!(
        optimized.code.contains("null!=current?current:0"),
        "{}",
        optimized.code
    );
    assert!(
        !optimized.code.contains("null!=current||0"),
        "{}",
        optimized.code
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
fn inverts_bare_return_guards_over_the_remaining_function_body() {
    let source = "function visit(a,b,out){out.push('head');if(a==null)return;if(!a&&b){return;}out.push('tail')}var left=[],right=[],done=[];visit(null,!1,left);visit(!1,!0,right);visit(1,!1,done);console.log(left.join(','));console.log(right.join(','));console.log(done.join(','))";
    let optimized =
        late_generated_javascript_cleanup_pass(source, LateJavaScriptCleanupPass::EarlyExitGuards)
            .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("return;"), "{optimized}");
    assert!(optimized.contains("a!=null"), "{optimized}");
    assert!(optimized.contains("!(!a&&b)"), "{optimized}");
    assert!(optimized.matches("if(").count() >= 2, "{optimized}");
}

#[test]
fn folds_guarded_returns_over_expression_only_suffixes() {
    let source = "function warn(){}function clamp(value){if(typeof value!='number')return 3e3;value=+value;warn(value);return value>5e3?5e3:value}console.log(clamp('x'));console.log(clamp(7))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::GuardReturnExpressionSuffixes,
    )
    .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(
        optimized.contains("return typeof value!='number'?3e3:(value=+value,warn(value),"),
        "{optimized}"
    );
    assert!(!optimized.contains("if("), "{optimized}");
}

#[test]
fn guarded_return_suffix_folding_rejects_declarations_and_nested_statements() {
    for source in [
        "function f(c){if(c)return 1;var x=read();return x}",
        "function f(c){if(c)return 1;let x=read();return x}",
        "function f(c,d){if(c)return 1;if(d)use();return 2}",
        "function f(c){if(c)return 1;try{use()}finally{done()}return 2}",
    ] {
        let optimized = late_generated_javascript_cleanup_pass(
            source,
            LateJavaScriptCleanupPass::GuardReturnExpressionSuffixes,
        )
        .unwrap();
        assert_eq!(optimized, source, "{source} -> {optimized}");
    }
}

#[test]
fn folds_expression_suffixes_into_terminal_returns() {
    let source = "function touch(){}function f(x){var y=x;touch(y);y+=2;return y}function g(){'use strict';touch();return this}";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::ExpressionSuffixReturns,
    )
    .unwrap();

    assert!(optimized.contains("return touch(y),y+=2,y"), "{optimized}");
    assert!(
        optimized.contains("'use strict';return touch(),this"),
        "{optimized}"
    );
    assert_eq!(
        run_node(&format!("{source};console.log(f(1))")),
        run_node(&format!("{optimized};console.log(f(1))"))
    );

    let declaration = "function f(){var x={a:1};x.a=2;return x}console.log(f().a)";
    let declaration_optimized = late_generated_javascript_cleanup_pass(
        declaration,
        LateJavaScriptCleanupPass::ExpressionSuffixReturns,
    )
    .unwrap();
    assert!(
        declaration_optimized.contains("var x={a:1};return x.a=2,x"),
        "{declaration_optimized}"
    );
    assert_eq!(run_node(declaration), run_node(&declaration_optimized));
}

#[test]
fn exposes_return_sequences_as_independent_objective_candidates() {
    let source = "function f(){a();return 1}function g(){b();return 2}";
    let variants = late_generated_javascript_cleanup_local_variants(
        source,
        LateJavaScriptCleanupPass::ExpressionSuffixReturns,
    )
    .unwrap();

    assert_eq!(variants.len(), 2, "{variants:?}");
    assert!(variants
        .iter()
        .any(|code| { code.contains("return a(),1") && code.contains("b();return 2") }));
    assert!(variants
        .iter()
        .any(|code| { code.contains("a();return 1") && code.contains("return b(),2") }));
}

#[test]
fn terminal_return_sequences_do_not_enter_for_heads() {
    let source = "function use(){}function f(a){var i=0;for(;i<a.length;i++){use(a[i])}use(i);return i}console.log(f([1,2]))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::ExpressionSuffixReturns,
    )
    .unwrap();

    assert!(optimized.contains("for(;i<a.length;i++)"), "{optimized}");
    assert!(optimized.contains("return use(i),i"), "{optimized}");
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn folds_boolean_conditional_values_without_leaking_operand_values() {
    let source = "function a(x){return x?true:false}function b(x,y){return x?true:!!y}function c(x,y){return x?false:!!y}function d(x,y){return x?!!y:false}function e(x,y){return x?!!y:true}for(const x of [0,1,'yes'])console.log(a(x),b(x,0),c(x,1),d(x,1),e(x,0))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::BooleanConditionalValues,
    )
    .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("?true:false"), "{optimized}");
    assert!(optimized.contains("!!x"), "{optimized}");
    assert!(optimized.contains("||"), "{optimized}");
    assert!(optimized.contains("&&"), "{optimized}");
}

#[test]
fn folds_false_conditional_with_a_boolean_sequence_tail() {
    let source =
        "function f(x){return\"number\"!=typeof x?false:(x=+x,Number.isFinite(x)&&x>=0&&x<=1)}";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::BooleanConditionalValues,
    )
    .unwrap();

    assert!(!optimized.contains("?false:"), "{optimized}");
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn folds_boolean_arms_against_the_complete_logical_condition() {
    let source = "function f(x){return x===void 0||!Array.isArray(x)?false:x.length>0}console.log(f(),f([]),f([1]))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::BooleanConditionalValues,
    )
    .unwrap();

    assert!(
        !optimized.contains("x===void 0||!!Array.isArray"),
        "{optimized}"
    );
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn boolean_conditional_fold_stays_inside_an_arrow_body() {
    let source = r#"let v=Symbol(),Re=e=>e!=null&&"object"==typeof e,_=(e,a)=>Re(e)&&!0===e[a],M=e=>!Re(e)?!1:_(e[v],"array"),C=e=>!Re(e)?!1:_(e[v],"object");console.log(M(null),M({[v]:{array:true}}),C({[v]:{object:true}}))"#;
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(!optimized.code.contains("!(e=>"), "{}", optimized.code);
    assert_eq!(run_node(source), "false true true\n");
    assert_eq!(
        run_node(&optimized.code),
        run_node(source),
        "{}",
        optimized.code
    );
}

#[test]
fn swaps_negated_conditional_arms_without_reordering_values() {
    let source =
        "function hit(x){return x}function f(x){return!x?hit(1):hit(2)}console.log(f(0),f(1))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::NegatedConditionalArms,
    )
    .unwrap();

    assert!(optimized.contains("x?hit(2):hit(1)"), "{optimized}");
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn keeps_negated_terms_inside_larger_logical_conditions() {
    let source = "function f(a,b){return a||!b?1:2}function g(a,b){return a&&!b?3:4}function h(a,b){return a??!b?5:6}console.log(f(0,0),f(1,1),g(1,0),g(0,0),h(null,0),h(0,0))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::NegatedConditionalArms,
    )
    .unwrap();

    assert_eq!(optimized, source);
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn swaps_invertible_disjunction_conditions_with_demorgan() {
    let source = "function clamp(x){return\"number\"!=typeof x||!Number.isFinite(+x)?3000:x>5000?5000:x}function custom(x){return x===void 0||!Array.isArray(x)?false:x.length>0}console.log(clamp('x'),clamp(6000),custom(),custom([]),custom([1]))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::NegatedConditionalArms,
    )
    .unwrap();

    assert!(
        optimized.contains("\"number\"==typeof x&&Number.isFinite(+x)?"),
        "{optimized}"
    );
    assert!(
        optimized.contains("x!==void 0&&Array.isArray(x)?"),
        "{optimized}"
    );
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn compounds_expression_position_identifier_updates() {
    let source =
        "function f(x){return(x=x+1)}function g(x){return(x=x+'-')}console.log(f(2),g('a'))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::UnitCounterUpdates,
    )
    .unwrap();

    assert!(optimized.contains("x+=1"), "{optimized}");
    assert!(optimized.contains("x+='-'"), "{optimized}");
    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
}

#[test]
fn factors_repeated_conditional_arms_without_reordering_tests() {
    let source = "function f(a,b){return a?hit(1):b?hit(1):hit(2)}function g(a,b){return a?(b?hit(1):hit(2)):hit(2)}";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::CommonConditionalArms,
    )
    .unwrap();

    assert!(optimized.contains("a||b?hit(1):hit(2)"), "{optimized}");
    assert!(optimized.contains("a&&b?hit(1):hit(2)"), "{optimized}");

    let sequence = "function f(a,x){return a?x:(log(),x>0?x:0)}";
    let sequence_optimized = late_generated_javascript_cleanup_pass(
        sequence,
        LateJavaScriptCleanupPass::CommonConditionalArms,
    )
    .unwrap();
    assert_eq!(
        sequence_optimized,
        "function f(a,x){return a||(log(),x>0)?x:0}"
    );
}

#[test]
fn folds_effectful_return_branches_into_conditional_sequences() {
    let source = "function f(x){if(x>1){warn('high');return 1}touch();return 2}function g(x){if(x){a();b();return 3}return 4}";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::ExpressionReturnBranches,
    )
    .unwrap();

    assert!(
        optimized.contains("return x>1?(warn('high'),1):(touch(),2)"),
        "{optimized}"
    );
    assert!(optimized.contains("return x?(a(),b(),3):4"), "{optimized}");
}

#[test]
fn sinks_sequence_assignments_only_across_inert_test_prefixes() {
    let source = "function f(e,k,r){return e==null?r:(e=e[k],\"number\"==typeof e?+e:r)}function g(e,k){return ok&&(e=e[k],\"boolean\"==typeof e)}";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::SequenceAssignmentFirstUse,
    )
    .unwrap();

    assert!(
        optimized.contains("\"number\"==typeof(e=e[k])?+e:r"),
        "{optimized}"
    );
    assert!(
        optimized.contains("ok&&\"boolean\"==typeof(e=e[k])"),
        "{optimized}"
    );

    let effectful = "function f(e,k){return(e=e[k],probe()==e)}";
    assert_eq!(
        late_generated_javascript_cleanup_pass(
            effectful,
            LateJavaScriptCleanupPass::SequenceAssignmentFirstUse,
        )
        .unwrap(),
        effectful
    );
}

#[test]
fn shortens_self_strict_equality_only_for_proven_generated_bindings() {
    let local = late_generated_javascript_cleanup_pass(
        "function valid(value){return value===value}",
        LateJavaScriptCleanupPass::SameBindingStrictEquality,
    )
    .unwrap();
    assert!(local.contains("value==value"), "{local}");

    let unresolved = late_generated_javascript_cleanup_pass(
        "function valid(){return ambient===ambient}",
        LateJavaScriptCleanupPass::SameBindingStrictEquality,
    )
    .unwrap();
    assert!(unresolved.contains("ambient===ambient"), "{unresolved}");
}

#[test]
fn inverts_continue_guards_over_the_remaining_loop_body() {
    let source = "function scan(){var out=[];for(var i=0;i<5;i++){if(i%2){continue;}out.push(i);out.push(i+10)}return out.join(',')}console.log(scan())";
    let optimized =
        late_generated_javascript_cleanup_pass(source, LateJavaScriptCleanupPass::EarlyExitGuards)
            .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("continue"), "{optimized}");
    assert!(optimized.contains("if(!(i%2))"), "{optimized}");
}

#[test]
fn folds_side_effecting_continue_guards_into_loop_tail_else_chains() {
    let source = "function scan(){var out=[];for(var i=0;i<5;i++){if(i<2){out.push('low'+i);continue}if(i==3){out.push('three');continue}out.push('tail'+i)}return out.join(',')}console.log(scan())";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::ContinueTailGuards,
    )
    .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("continue"), "{optimized}");
    assert_eq!(optimized.matches("else{").count(), 2, "{optimized}");
    assert!(optimized.len() < source.len(), "{optimized}");
}

#[test]
fn inverts_side_effecting_continue_guards_as_an_independent_spelling() {
    let source = "function scan(){var out=[];for(var i=0;i<5;i++){if(i<2){out.push('low'+i);continue}if(i==3){out.push('three');continue}out.push('tail'+i)}return out.join(',')}console.log(scan())";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::InvertedContinueTailGuards,
    )
    .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("continue"), "{optimized}");
    assert_eq!(optimized.matches("else{").count(), 2, "{optimized}");
    assert!(optimized.contains("i!=3"), "{optimized}");
}

#[test]
fn side_effecting_continue_folding_preserves_scope_and_loop_boundaries() {
    for source in [
        "function f(a){for(;;){if(a){use(a);continue;}let value=1;use(value)}}",
        "function f(a,b){for(;;){if(a){if(b){use(b);continue;}use(a)}use(b)}}",
        "function f(a,b){for(;;){if(a){if(b)continue}use(a)}}",
        "function f(a){for(;;){if(a){continue;}use(a)}}",
        "function f(a){for(;;){if(a){use(a);continue;}else use(a);use(a)}}",
        "function f(a){for(;;){if(a){use(a);continue outer;}use(a)}}",
    ] {
        let optimized = late_generated_javascript_cleanup_pass(
            source,
            LateJavaScriptCleanupPass::ContinueTailGuards,
        )
        .unwrap();
        assert_eq!(optimized, source, "{source} -> {optimized}");
    }
}

#[test]
fn restores_semicolon_when_unwrapping_object_literal_before_else() {
    for source in [
        "function f(c){var x;if(c){var o={a:1}}else{o={a:0}}return o.a}",
        "function f(c){if(c){return {a:1}}else{return {a:0}}}",
        "function f(c,x){if(c){x={a:1}}else{x={a:0}}return x.a}",
        "function f(issue){if(typeof issue==\"string\"){var next={message:issue}}else{next=issue}return next}",
    ] {
        let isolated = late_generated_javascript_cleanup_pass(
            source,
            LateJavaScriptCleanupPass::SingleStatementControlBraces,
        )
        .unwrap();
        asserts_parses(&isolated);
        assert!(
            isolated.contains("};else") || isolated.contains("}; else"),
            "object-ending consequent needs a semicolon before else: {isolated}"
        );

        let optimized = optimize_generated_javascript(source).unwrap();
        asserts_parses(&optimized.code);
    }

    let try_before_else = "function f(c){if(c){try{use(1)}catch(e){use(e)}}else{use(0)}}";
    let try_optimized = late_generated_javascript_cleanup_pass(
        try_before_else,
        LateJavaScriptCleanupPass::SingleStatementControlBraces,
    )
    .unwrap();
    asserts_parses(&try_optimized);
    assert!(
        try_optimized.contains("catch(e){use(e)}else") || try_optimized.contains("}else{use(0)}"),
        "{try_optimized}"
    );
    assert!(
        !try_optimized.contains("};else"),
        "try/catch already terminates before else: {try_optimized}"
    );
}

#[test]
fn elides_single_simple_control_body_braces() {
    let source = "function run(a){var out=[];if(a){out.push(1)}else{out.push(2);}for(var i=0;i<2;i++){out.push(i)}while(a){break}return out.join(',')}console.log(run(!1));console.log(run(!0))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::SingleStatementControlBraces,
    )
    .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("{out.push"), "{optimized}");
    assert!(!optimized.contains("{break}"), "{optimized}");
    assert!(optimized.len() < source.len(), "{optimized}");
}

#[test]
fn recursively_elides_single_control_statement_braces() {
    let source = "function run(a,b){var out=[];if(a){for(;b>0;b--){out.push(b)}}else{while(b>0){out.push(b--)}}if(a){if(b){out.push(7)}else{out.push(8)}}else{out.push(9)}return out.join(',')}console.log(run(!0,2));console.log(run(!1,2))";
    let optimized = late_generated_javascript_cleanup_pass(
        source,
        LateJavaScriptCleanupPass::SingleStatementControlBraces,
    )
    .unwrap();

    assert_eq!(run_node(source), run_node(&optimized), "{optimized}");
    assert!(!optimized.contains("{for("), "{optimized}");
    assert!(!optimized.contains("{while("), "{optimized}");
    assert!(optimized.contains("if(a)if(b)"), "{optimized}");
    assert!(optimized.len() < source.len(), "{optimized}");
}

#[test]
fn control_body_brace_elision_keeps_scope_and_dangling_else_boundaries() {
    for source in [
        "function f(a){if(a){let value=1}}",
        "function f(a){for(;;){const value=1}}",
        "function f(a){if(a){function value(){}}}",
        "function f(a){if(a){class Value{}}}",
        "function f(a,b){if(a){if(b)use(b)}else use(a)}",
        "function f(a){while(a){use(a);use(a)}}",
        "function f(a){if(a){use(a)\nuse(a)}}",
    ] {
        let optimized = late_generated_javascript_cleanup_pass(
            source,
            LateJavaScriptCleanupPass::SingleStatementControlBraces,
        )
        .unwrap();
        assert_eq!(optimized, source, "{source} -> {optimized}");
    }

    let nested_dangling = "function f(a,b,c){if(a){while(b){if(c){use(c)}}}else use(a)}";
    let optimized = late_generated_javascript_cleanup_pass(
        nested_dangling,
        LateJavaScriptCleanupPass::SingleStatementControlBraces,
    )
    .unwrap();
    assert!(optimized.contains("if(a){while(b)if(c)"), "{optimized}");
    assert!(optimized.contains("}else use(a)"), "{optimized}");
}

#[test]
fn early_exit_inversion_preserves_scope_and_control_boundaries() {
    for source in [
        "function f(a){if(a)return;let value=1;use(value)}",
        "function f(a){if(a)return;const value=1;use(value)}",
        "function f(a){if(a)return;function value(){}use(value)}",
        "function f(a,b){if(a)if(b)return;use(a)}",
        "function f(a){if(a)return 1;use(a)}",
        "function f(a){for(;;){if(a)continue outer;use(a)}}",
    ] {
        let optimized = late_generated_javascript_cleanup_pass(
            source,
            LateJavaScriptCleanupPass::EarlyExitGuards,
        )
        .unwrap();
        assert_eq!(optimized, source, "{source} -> {optimized}");
    }
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
        guard.code.contains("if(e<=1)") && guard.code.contains("return n.then()"),
        "{}",
        guard.code
    );
    assert!(guard.code.contains("if(\"pending\""), "{}", guard.code);
    assert!(guard.code.contains("var d=s.reject"), "{}", guard.code);

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

    let comma_arg = optimize_generated_javascript(
        "function pipe(x,i,o,u){return i?o.p():(s=x?[i]:arguments,o[u](this,s))}",
    )
    .unwrap();
    assert!(
        comma_arg.code.contains("s=x?[i]:arguments") && comma_arg.code.contains("o[u](this,s)"),
        "{}",
        comma_arg.code
    );

    let comma_order = concat!(
        "var order='',s;function rhs(){order+='rhs;';return 1}",
        "var o={get call(){order+='callee;';return function(){order+='call;'}}};",
        "(s=rhs(),o.call(s));console.log(order)"
    );
    let comma_order_optimized = optimize_generated_javascript(comma_order).unwrap();
    assert_eq!(
        run_node(&comma_order_optimized.code).trim(),
        "rhs;callee;call;"
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
fn keeps_member_reads_at_their_guarded_assignment_point() {
    let and_form = optimize_generated_javascript(
        "function f(o,e,r,n,c,i){!r&&(r=o.once);for(c=n=!0;e.length;i=-1)e.pop();return r}",
    )
    .unwrap();
    assert!(
        and_form.code.contains("r=r||o.once") || and_form.code.contains("!r&&(r=o.once)"),
        "{}",
        and_form.code
    );

    let if_temp = optimize_generated_javascript(
        "function f(o,e,r,n,c,i){if(!r){var t=o.once;r=t}for(c=n=!0;e.length;i=-1)e.pop();return r}",
    )
    .unwrap();
    assert!(
        (if_temp.code.contains("if(!r)") && if_temp.code.contains("t=o.once"))
            || if_temp.code.contains("r=r||o.once"),
        "{}",
        if_temp.code
    );

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
fn preserves_single_use_index_temps_in_a_fire_loop() {
    let optimized = optimize_generated_javascript(
        "function f(a,i,m){i++;for(;;i++){var t=i;if(i>=a.length){break}t=a[i];var c=m[0];!1===t.apply(c,m[1])&&(i=+a.length)}}",
    )
    .unwrap();
    assert!(
        optimized.code.contains("t=a[i]")
            && optimized.code.contains("c=m[0]")
            && optimized.code.contains("t.apply(c,m[1])"),
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
        unbound
            .code
            .contains("return r.y(this==r?void 0:this,arguments),this")
            || unbound.code.contains("r.y(this==r?void 0:this,arguments)"),
        "{}",
        unbound.code
    );
    assert!(!unbound.code.contains("var i="), "{}", unbound.code);
}

#[test]
fn rematerializes_only_function_values_whose_names_stay_stable() {
    let deferred = optimize_generated_javascript(
        "let B=n=>{var r={id:n};return r};a.extend({Deferred:B,when(r){return r}})",
    )
    .unwrap();
    assert!(
        deferred.code.contains("B=n=>") && deferred.code.contains("Deferred:B"),
        "{}",
        deferred.code
    );

    let callbacks = optimize_generated_javascript(
        "let Y=a=>{var e=[];return {add(){e.push(a)}}};a.Callbacks=Y",
    )
    .unwrap();
    assert!(
        callbacks.code.contains("Y=a=>{") && callbacks.code.contains("a.Callbacks=Y"),
        "{}",
        callbacks.code
    );

    let called = optimize_generated_javascript("let G=e=>{var t=e+1;return t};use(G(2))").unwrap();
    assert!(
        called.code.contains("use((e=>{") && !called.code.contains("G="),
        "{}",
        called.code
    );

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
        console_cache.code.contains("var n=window.console")
            && console_cache.code.contains("n.warn"),
        "{}",
        console_cache.code
    );

    let shadowed_inner = optimize_generated_javascript(
        "let F=(e,t)=>{t=t||[];return t};a.extend({makeArray:F,Deferred(n){var F=n[0];return F}})",
    )
    .unwrap();
    assert!(
        shadowed_inner.code.contains("F=(e,t)=>{") && shadowed_inner.code.contains("makeArray:F"),
        "{}",
        shadowed_inner.code
    );
}

#[test]
fn keeps_object_function_properties_constructible() {
    let source = "function mitt(){return{on:function(t,e){return t},off:function(t,e){return t}}};var e=mitt();console.log(typeof new e.on('x',0))";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("on:function(") && !optimized.code.contains("{on(t,e){"),
        "{}",
        optimized.code
    );
    assert_eq!(run_node(&optimized.code).trim(), "object");
}

#[test]
fn keeps_loop_carried_call_results_before_array_stores() {
    let source = "function a(x,g){return x+g}function b(){for(var g=0,j=0,i=[];j<3;j++){g=a(1,g);i[j]=g}return g+i[0]}";
    let optimized = optimize_generated_javascript(source).unwrap();
    assert!(
        optimized.code.contains("g=a(") && !optimized.code.contains("i[j]=a("),
        "{}",
        optimized.code
    );
    let script = format!("{}\nconsole.log(b());", optimized.code);
    assert_eq!(
        run_node(&script).trim(),
        run_node(&format!("{source}\nconsole.log(b());")).trim()
    );
}

#[test]
fn keeps_exported_bindings_out_of_export_clauses() {
    let aliased = optimize_generated_javascript(
        "let cubicBezier=(a,c,t,l)=>a+c+t+l;export{cubicBezier as cubicBezier}",
    )
    .unwrap();
    assert!(
        aliased.code.contains("export{cubicBezier") && !aliased.code.contains("export{("),
        "{}",
        aliased.code
    );

    let named =
        optimize_generated_javascript("let clamp=(a,b,c)=>a<b?b:a>c?c:a;export{clamp}").unwrap();
    assert!(
        named.code.contains("export{clamp") && !named.code.contains("export{("),
        "{}",
        named.code
    );

    let literal = optimize_generated_javascript("let version=1;export{version}").unwrap();
    assert!(
        literal.code.contains("export{version") && !literal.code.contains("export{1"),
        "{}",
        literal.code
    );

    let called = optimize_generated_javascript("let G=e=>{var t=e+1;return t};use(G(2))").unwrap();
    assert!(
        called.code.contains("use((e=>{") && !called.code.contains("G="),
        "{}",
        called.code
    );
}

#[test]
fn preserves_member_reads_inside_nested_functions() {
    let when = optimize_generated_javascript(
        "var l=[],g=l.slice;a.extend({when(r){return l.slice.call(arguments)}})",
    )
    .unwrap();
    assert!(
        when.code.contains("l.slice.call(arguments)"),
        "{}",
        when.code
    );

    let shadowed = optimize_generated_javascript(
        "var l=[],g=l.slice;function when(){var l=[1];return l.slice.call(arguments)}",
    )
    .unwrap();
    assert!(shadowed.code.contains("l.slice.call"), "{}", shadowed.code);

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
        window_console
            .code
            .contains("window.console&&window.console.warn&&e")
            && !window_console
                .code
                .contains("(window.console&&window.console.warn)&&"),
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
        optimized
            .code
            .contains("i[d]=arguments.length>1?g.call(arguments):p"),
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
    // P's initializer reads C, and C is reassigned before the call: the
    // binding must survive so the call still sees the pre-reassignment value.
    assert!(
        optimized.code.contains("var P=i[3-C][2].disable") && optimized.code.contains("P,C,u"),
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
fn chained_index_temp_folding_keeps_materialized_receivers_bound() {
    let source = concat!(
        "function deferredThen(e,a,W,t,d,f,Q,q){let outer=q.Deferred;return outer(function(X){",
        "var h,ia,Y,g=W,ka=t(f)?f:g;",
        "g=e[0],g=g[3],h=a,g.add(h(0,X,ka,X.notifyWith)),",
        "ka=W,ia=t(d)?d:ka,ka=e[1],ka=ka[3],ka.add(a(0,X,ia)),",
        "Y=e[2],Y=Y[3],Y.add(a(0,X,t(f)?f:Q))",
        "}).promise()}",
        "let hits=[];let rows=[[0,0,0,{add(v){hits.push(v)}}],[0,0,0,{add(v){hits.push(v)}}],[0,0,0,{add(v){hits.push(v)}}]];",
        "let q={Deferred(cb){cb({notifyWith:0});return{promise(){return hits}}}};",
        "console.log(deferredThen(rows,(depth,next,handler)=>handler,null,v=>v!=null,2,3,4,q).join(','))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert_eq!(
        run_node(&optimized.code).trim(),
        "3,2,3",
        "{}",
        optimized.code
    );
}

#[test]
fn overlapping_index_temp_chains_remain_bound() {
    let source = concat!(
        "function f(e,q){return q(M=>{",
        "var k=e[0],h=k[3];h.add(1);",
        "var o=e[1],g=o[3];g.add(2);",
        "var S=e[2][3];S.add(3)",
        "})}",
        "let hits=[],rows=[[0,0,0,{add(v){hits.push(v)}}],[0,0,0,{add(v){hits.push(v)}}],[0,0,0,{add(v){hits.push(v)}}]];",
        "console.log(f(rows,cb=>{cb({});return hits}).join(','))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert_eq!(
        run_node(&optimized.code).trim(),
        "1,2,3",
        "{}",
        optimized.code
    );
}

#[test]
fn preserves_a_single_use_object_across_aliasing_index_writes() {
    let optimized = optimize_generated_javascript(
        "let V={jquery:\"3.7.1\",length:0};a.fn=V;a.prototype=V;V[Symbol.iterator]=Array.prototype[Symbol.iterator];use(a)",
    )
    .unwrap();
    assert!(
        optimized.code.contains("let V={")
            && optimized.code.contains("a.fn=V")
            && optimized.code.contains("a.prototype=V")
            && optimized.code.contains("V[Symbol.iterator]")
            && optimized.code.contains("use(a)"),
        "{}",
        optimized.code
    );

    let prototype = optimize_generated_javascript(
        "let V={jquery:\"3.7.1\",constructor:a,length:0,toArray(){return g.call(this)},get(e){return e==null?g.call(this):e<0?this[e+this.length]:this[e]},pushStack(e){e=w(this.constructor(),e),e.prevObject=this;return e},each(e){return d(this,e)},map(e){return this.pushStack(z(this,(t,n)=>e.call(t,n,t),null))},slice(){return this.pushStack(g.apply(this,arguments))},first(){return this.eq(0)},last(){return this.eq(-1)},even(){return this.pushStack(k(this,(e,t)=>(t+1)%2,!1))},odd(){return this.pushStack(k(this,(e,t)=>t%2,!1))},eq(e){var n=this.length,t=e<0&&n;e+=t;return this.pushStack(e>=0&&e<n?[this[e]]:[])},end(){return this.prevObject||this.constructor()},push:l.push,sort:l.sort,splice:l.splice,extend:j};a.fn=V;a.prototype=V;a.extend=j;V[Symbol.iterator]=Array.prototype[Symbol.iterator];",
    )
    .unwrap();
    assert!(
        prototype.code.contains("let V={")
            && prototype.code.contains("a.fn=V")
            && prototype.code.contains("a.prototype=V")
            && prototype.code.contains("V[Symbol.iterator]"),
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
                again.code.contains("let V="),
                "the second peephole pass must retain V's creation point: {}",
                &again.code[again.code.find("a.fn.init").unwrap_or(0)
                    ..again
                        .code
                        .find("a.extend=j")
                        .map(|i| i + 20)
                        .unwrap_or(again.code.len())]
            );
        }
    }
}

#[test]
fn keeps_single_use_object_captures_at_outer_creation_time() {
    for (source, expected) in [
        (
            concat!(
                "function make(){",
                "let target={x:1,dispose(){this.x=2}};",
                "let snapshot={value:target.x};",
                "return function(){target.dispose();return snapshot.value}",
                "}",
                "let callback=make();console.log(callback(),callback())"
            ),
            "1 1\n",
        ),
        (
            concat!(
                "function probe(){",
                "let target={x:1,dispose(){this.x=2}};",
                "let snapshot={};snapshot.value=target.x;",
                "(function(){let value=snapshot.value;target.dispose();console.log(value)})();",
                "console.log(target.x)",
                "}probe()"
            ),
            "1\n2\n",
        ),
    ] {
        let optimized = optimize_generated_javascript(source).unwrap();
        assert_eq!(run_node(source), expected, "{source}");
        assert_eq!(run_node(&optimized.code), expected, "{}", optimized.code);
    }
}

#[test]
fn keeps_single_use_object_values_at_their_creation_point() {
    let source = concat!(
        "var order='';",
        "function read(v){order+='read-'+v+';';return v}",
        "function mutate(){order+='mutate;'}",
        "function f(y){let x={v:read(y)};y=2;mutate();return x}",
        "console.log(f(1).v+'|'+order)"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(
        optimized.code.contains("x={v:read(y)}"),
        "{}",
        optimized.code
    );
    assert_eq!(run_node(source).trim(), "1|read-1;mutate;");
    assert_eq!(
        run_node(&optimized.code).trim(),
        "1|read-1;mutate;",
        "{}",
        optimized.code
    );
}

#[test]
fn keeps_object_property_arrows_lexical() {
    let source = concat!(
        "class Base{get x(){return'base'}}",
        "class Derived extends Base{constructor(){super();this.newTarget={f:()=>new.target}.f}",
        "make(){return{f:()=>super.x}}}",
        "let value=new Derived;console.log(value.make().f()+'|'+(value.newTarget()===Derived))"
    );
    let optimized = optimize_generated_javascript(source).unwrap();

    assert!(optimized.code.contains("=>super.x"), "{}", optimized.code);
    assert!(
        optimized.code.contains("=>new.target"),
        "{}",
        optimized.code
    );
    assert_eq!(run_node(source).trim(), "base|true");
    assert_eq!(
        run_node(&optimized.code).trim(),
        "base|true",
        "{}",
        optimized.code
    );
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
