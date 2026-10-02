/* Callback ABI 3. Metadata is borrowed for the duration of a call. An absent
   entry may occur before a later supplied argument; no fixed-width mask limits
   the number of parameters. A null bitmap means every supplied value is present. */
typedef struct { size_t count; const bool *absent; } ls_native_arguments;
