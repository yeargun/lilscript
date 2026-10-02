#include "interfaces.h"
#include "peer.h"
#include <assert.h>
#include <stdio.h>
int main(void) {
    assert(demo_initialize(0,NULL));assert(peer_initialize(0,NULL));
    assert(demo_e_interfaces__add(2,1,(demo_ls_native_arguments){2,NULL})==6);
    assert(peer_e_interfaces_2dpeer__add(2)==1002);
    peer_ls_string label=peer_e_interfaces_2dpeer__label_get();
    assert(label.length==4);peer_ls_string_release(label);
    peer_shutdown();assert(demo_e_interfaces__total_get()==6);demo_shutdown();
    assert(demo_ls_native_owned_objects()==0 && peer_ls_native_owned_objects()==0);
    puts("two libraries coexist");
}
