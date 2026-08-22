#include <stdio.h>
#include <stdint.h>
#include <stddef.h>

typedef struct BrotliDictionary {
  uint8_t size_bits_by_length[32];
  uint32_t offsets_by_length[32];
  size_t data_size;
  const uint8_t* data;
} BrotliDictionary;

const BrotliDictionary* BrotliGetDictionary(void);

int main(void) {
  const BrotliDictionary* d = BrotliGetDictionary();
  if (!d || !d->data || d->data_size != 122784) {
    fprintf(stderr, "unexpected Brotli dictionary\n");
    return 1;
  }
  fwrite(d->data, 1, d->data_size, stdout);
  return 0;
}
