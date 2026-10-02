#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
void ls_native_collect_cycles(void);

#include <stdbool.h>
bool ls_native_exception_pending(void);
