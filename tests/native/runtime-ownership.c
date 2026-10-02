/* Independent owner/graph observations of the production C runtime recipes. */
#include <assert.h>
#include "../../src/program/runtime/prologue.h"
#include "../../src/program/runtime/string.h"
#define LS_NATIVE_CYCLE_THRESHOLD 1
#include "../../src/program/runtime/memory.c"
#include "../../src/program/runtime/from_u32.c"
#include "../../src/program/runtime/string_equal.c"
#include "../../src/program/runtime/strings.c"
#include "../../src/program/runtime/string_builder.c"
#include "../../src/program/runtime/value.h"
#include "../../src/program/runtime/value.c"
#include "../../src/program/runtime/collections.c"
#include "../../src/program/runtime/binary.c"

typedef struct node { ls_native_object owner; struct node *left, *right; ls_string text; } node;
static void node_drop(ls_native_object *owner) {
    node *n = (node *)owner;
    ls_native_release(n->left); ls_native_release(n->right); ls_string_release(n->text);
}
static void node_trace(ls_native_object *owner, ls_native_visit visit, void *context) {
    node *n = (node *)owner;
    visit(n->left, context); visit(n->right, context); visit(n->text.owner, context);
}
static node *make_node(void) { return ls_native_allocate(sizeof(node), node_drop, node_trace); }
static void edge(node **slot, node *child) { ls_native_retain(child); ls_native_release(*slot); *slot = child; }
static void assert_empty(void) { ls_native_collect_cycles(); assert(ls_native_live_objects == 0); }
int main(void) {
    assert(ls_runtime_init());
    assert(ls_native_fresh_identity() != ls_native_fresh_identity());
    /* Two references to the same child are two edges, not a set of children. */
    node *a = make_node(), *b = make_node();
    edge(&a->left,b); edge(&a->right,b); edge(&b->left,a);
    a->text = ls_string_ascii("kept through collection", 23);
    ls_native_release(b); ls_native_collect_cycles();
    assert(ls_native_live_objects == 3 && a->left == a->right);
    ls_native_release(a); assert_empty();
    /* An unreachable cycle can point to a separately rooted descendant. */
    a = make_node(); b = make_node(); node *root = make_node();
    edge(&a->left,b); edge(&b->left,a); edge(&b->right,root);
    ls_native_release(a); ls_native_release(b); ls_native_collect_cycles();
    assert(ls_native_live_objects == 1 && root->owner.references == 1);
    ls_native_release(root); assert_empty();
    /* Zero-count destruction and gray/black walks must not recurse in C. */
    root = make_node(); node *tail = root;
    for (int i=0;i<100000;i++) { node *next=make_node(); edge(&tail->left,next); ls_native_release(next); tail=next; }
    ls_native_collect_cycles(); assert(ls_native_live_objects == 100001);
    ls_native_release(root); assert_empty();
    /* A retained slice owns its allocation after the original is released. */
    ls_string whole = ls_string_ascii("  abcdef  ",10);
    ls_string slice = ls_string_slice(whole,2,true,8);
    ls_string trimmed = ls_string_trim(whole,true,true);
    ls_string_release(whole); ls_native_collect_cycles();
    assert(ls_string_equal(slice,trimmed) && ls_native_live_objects == 1);
    ls_string_clear(&slice); assert(trimmed.data[5] == 'f');
    ls_string_clear(&trimmed); assert_empty();
    ls_string joined=ls_string_join_owned(3,(ls_string[]){ls_int_to_string(42),ls_string_ascii("/",1),ls_bool_to_string(true)});
    assert(joined.length==7 && joined.data[6]=='e');
    ls_string empty_concat=ls_string_concat(joined,(ls_string){0});
    ls_string_release(joined); assert(empty_concat.data[0]=='4');
    ls_string_release(empty_concat); assert_empty();
    /* Dynamic string keys and values retain storage. Maps can own themselves. */
    ls_native_object *map=ls_map_new();
    ls_string key=ls_string_ascii("key",3), text=ls_string_ascii("value",5);
    ls_map_set(map,ls_value_string(key),ls_value_string(text));
    ls_string_release(text); ls_string_release(key);
    ls_native_object *alias=map; ls_native_retain(alias);
    ls_map_set(map,ls_value_int(0),ls_value_object(map));
    ls_native_release(map); ls_native_collect_cycles();
    assert(ls_map_size(alias)==2);
    ls_native_release(alias); assert_empty();
    /* Typed views retain a sliced buffer after the original owner is gone. */
    ls_native_object *buffer=ls_buffer_new(16), *view=ls_typed_view(buffer,0,4);
    ls_native_release(buffer); ls_native_collect_cycles();
    assert(ls_native_live_objects==2);
    ls_typed_set_int32(view,2,42); assert(ls_typed_get_int32(view,2)==42);
    ls_native_release(view); assert_empty();
    puts("runtime ownership: passed");
}
