#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void) {
    return ls_native_live_objects;
}
size_t ls_native_total_allocations(void) { return ls_native_allocation_count; }
#endif
