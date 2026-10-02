/* Every returned handle/value owns a reference; arguments borrow. Constructors
   and mutation use the same runtime as source operations. Handle parameters
   must be live handles from this library, never fabricated addresses. */
/* For a new C callback after initialization. Copying a callback preserves its
   identity; creating an independent one requests another identity. */
uint64_t ls_native_new_identity(void);
ls_native_object *ls_native_record_new(void);
ls_value ls_native_record_get(ls_native_object *, ls_string key);
void ls_native_record_set(ls_native_object *, ls_string key, ls_value value);
#ifdef LS_NATIVE_API_ARRAYS
typedef struct ls_native_array ls_native_array;
ls_native_array *ls_native_array_new(void);
size_t ls_native_array_length(ls_native_array *);
ls_value ls_native_array_get(ls_native_array *, int32_t index);
void ls_native_array_set(ls_native_array *, int32_t index, ls_value);
#endif
#ifdef LS_NATIVE_API_TASKS
/* State: 0 pending, 1 fulfilled, 2 rejected. Observing a settled task's result
   marks rejection handled and returns an owned payload (also for rejection).
   Resolve/reject settle at most once; callbacks run at the next drain. */
ls_native_object *ls_native_task_new(void);
void ls_native_task_resolve(ls_native_object *, ls_value);
void ls_native_task_reject(ls_native_object *, ls_value);
unsigned ls_native_task_state(ls_native_object *);
ls_value ls_native_task_result(ls_native_object *);
#endif
#ifdef LS_NATIVE_API_ITERATORS
/* false means exhausted or an exception (check pending); on true, *value is
   owned. Explicit close executes pending finally blocks; release alone does not. */
bool ls_native_generator_next(ls_native_object *, ls_value *value);
void ls_native_generator_close(ls_native_object *);
#endif
