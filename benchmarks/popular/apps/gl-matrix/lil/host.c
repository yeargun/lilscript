#include "../../../build/gl-matrix-native.c"

LilScriptValue glMatrixCreateArray(int32_t size) {
  return (LilScriptValue){.tag = 8, .p = lilscript_f32_length(size)};
}
