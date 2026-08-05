#include "../target/verification/extern_abi.c"

int32_t consumePoint(LilScriptStruct_506f696e74 point) {
    return point.f0 + point.f1;
}

int32_t consumeBox(LilScriptClass_426f78 value) {
    return value->f0;
}
