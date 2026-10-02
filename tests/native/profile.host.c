#include "profile.h"
#include <stdlib.h>
int32_t host_profileIterations(void) {
    const char *text=getenv("LILSCRIPT_N2_ITERATIONS");
    return text ? (int32_t)strtol(text,NULL,10) : 100000;
}
