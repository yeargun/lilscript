#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void) {
    return ls_native_live_objects;
}
#endif
