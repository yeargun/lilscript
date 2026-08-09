#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <math.h>

typedef struct Buffer {
  uint8_t *data;
  int32_t len;
  bool shared;
} *Buffer;

typedef struct Uint8View {
  Buffer buffer;
  int32_t offset;
  int32_t len;
} *Uint8View;

Uint8View r(double value) {
  if (!isfinite(value) || value < 0 || value > INT32_MAX) abort();
  int32_t size = (int32_t)value;
  Buffer buffer = malloc(sizeof(*buffer));
  Uint8View view = malloc(sizeof(*view));
  if (!buffer || !view) abort();
  buffer->data = malloc((size_t)size);
  if (size != 0 && !buffer->data) abort();
  buffer->len = size;
  buffer->shared = false;
  view->buffer = buffer;
  view->offset = 0;
  view->len = size;
  static uint32_t state = 0x9e3779b9;
  for (int32_t index = 0; index < size; index += 1) {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    buffer->data[index] = (uint8_t)state;
  }
  return view;
}
