/* Payloads own one reference while propagating through calls or finally.
   The implicit status belongs to the originating thread, matching callback
   ABI 2. No foreign unwinding or setjmp/longjmp crosses C providers. */
static _Thread_local ls_value ls_native_thrown;
typedef struct { bool raised; const char *name, *message; ls_value value; } ls_exception_state;
static LS_NATIVE_UNUSED inline void ls_native_throw(ls_value value) {
    ls_value_copy(&ls_native_thrown,value); ls_native_error_name=NULL; ls_native_error_message=NULL; ls_native_raised=true;
}
static LS_NATIVE_UNUSED inline ls_exception_state ls_exception_save(void) {
    ls_exception_state state={ls_native_raised,ls_native_error_name,ls_native_error_message,ls_native_thrown};
    ls_native_raised=false; ls_native_error_name=NULL; ls_native_error_message=NULL; ls_native_thrown=(ls_value){0};
    return state;
}
static LS_NATIVE_UNUSED inline void ls_exception_clear(ls_exception_state *state) {
    ls_value_release(state->value); *state=(ls_exception_state){0};
}
static LS_NATIVE_UNUSED inline void ls_exception_restore(ls_exception_state *state) {
    ls_value_clear(&ls_native_thrown);
    ls_native_raised=state->raised; ls_native_error_name=state->name; ls_native_error_message=state->message; ls_native_thrown=state->value;
    *state=(ls_exception_state){0};
}
static LS_NATIVE_UNUSED inline ls_value ls_exception_catch(void) {
    ls_exception_state state=ls_exception_save();
    if(!state.message) return state.value;
    ls_native_object *record=ls_record_new();
    ls_string name=ls_string_ascii("name",4),message=ls_string_ascii("message",7);
    ls_string kind=ls_string_ascii(state.name,strlen(state.name)),text=ls_string_ascii(state.message,strlen(state.message));
    ls_record_set(record,name,ls_value_string(kind)); ls_record_set(record,message,ls_value_string(text));
    ls_string_release(name); ls_string_release(message); ls_string_release(kind); ls_string_release(text);
    ls_exception_clear(&state); return ls_value_object(record);
}

void ls_native_exception_raise(ls_value value) { ls_native_throw(value); }
ls_value ls_native_exception_take(void) { return ls_exception_catch(); }
