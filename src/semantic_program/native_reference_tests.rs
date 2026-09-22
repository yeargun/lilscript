//! Fixed language observations across direct C and JavaScript reference ABIs.
//! The shared harness executes GCC/Clang at O0/O2/UBSan and three JS naming
//! plans, measures complete JS artifacts, and checks lifetime/codec isolation.
use super::native_struct_tests::qualify;

#[test]
fn reference_scalar_aliases_write_immediately_and_copies_remain_values() {
    qualify(
        "reference-scalar-aliases",
        r#"
void mutate(ref int first,ref int second) {
    first=7;print(second);second+=2;print(first);
}
void payload(ref string text,ref bool enabled) {text="XY";enabled=false;}
int state=1;int copy=state;
mutate(ref state,ref state);print(state);print(copy);
string text="A";string saved=text;bool enabled=true;
payload(ref text,ref enabled);print(text.length);print(saved.length);print(enabled);
"#,
        "7\n9\n9\n1\n2\n1\nfalse\n",
    );
}

#[test]
fn reference_whole_and_nested_field_aliases_follow_root_replacement() {
    qualify(
        "reference-overlapping-products",
        r#"
struct Pair {int left;int right;}
struct Box {Pair pair;int sibling;}
void change(ref Box whole,ref int leaf) {
    whole=Box{Pair{10,20},30};print(leaf);
    leaf=40;print(whole.pair.left);
    whole.pair.right=50;print(leaf);
}
Pair update(ref Pair pair) {Pair saved=pair;pair.left=99;return saved;}
Box state=Box{Pair{1,2},3};Box saved=state;
change(ref state,ref state.pair.left);
print(state.pair.right);print(state.sibling);print(saved.pair.left);
Pair snapshot=update(ref state.pair);print(snapshot.left);print(state.pair.left);
snapshot.right=88;print(state.pair.right);
"#,
        "10\n40\n40\n50\n30\n1\n40\n99\n50\n",
    );
}

#[test]
fn reference_forwarding_composes_field_paths_and_recursive_calls_keep_the_location() {
    qualify(
        "reference-forwarding-recursion",
        r#"
struct Pair {int left;int right;}
struct Box {Pair pair;}
void add(ref int value,int amount){value+=amount;}
void descending(ref int value,int count) {
    if(count>0){add(ref value,count);descending(ref value,count-1);}
}
void pair(ref Pair value){descending(ref value.left,3);add(ref value.right,7);}
void box(ref Box value){pair(ref value.pair);}
Box state=Box{Pair{1,2}};box(ref state);
print(state.pair.left);print(state.pair.right);
"#,
        "7\n9\n",
    );
}

#[test]
fn reference_preparation_preserves_value_snapshots_and_later_argument_reentry() {
    qualify(
        "reference-argument-order",
        r#"
struct Pair {int left;int right;}
int change(ref Pair pair){pair=Pair{7,8};print(5);return 9;}
void observe(Pair snapshot,ref int field,int last) {
    print(snapshot.left);print(field);print(last);field=11;
}
Pair state=Pair{1,2};observe(state,ref state.left,change(ref state));
print(state.left);print(state.right);
"#,
        "5\n1\n7\n9\n11\n8\n",
    );
}
