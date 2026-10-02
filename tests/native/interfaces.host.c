#include "interfaces.h"
int32_t host_provider(int32_t value) {
    if(value==88) {
        /* Shutdown cannot invalidate an active compiled activation. */
        demo_shutdown();
        return 0;
    }
    return value*3;
}
