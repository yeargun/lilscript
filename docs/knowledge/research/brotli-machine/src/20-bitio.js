/* 1.5 / 2.  Bit packing.
   A brotli stream is a little-endian bit stream: bit 0 of byte 0 comes first,
   and a multi-bit value is filled from its least significant bit up. Prefix
   codes are the one exception — they are stored most-significant-bit-first, so
   a decoder walks them one bit at a time. Both classes below keep a "map": the
   bit range every field occupied, which is what the page paints over the hex. */
(function (BM) {
  "use strict";

  class BitReader {
    constructor(bytes) {
      this.bytes = bytes;
      this.bitPos = 0;
      this.map = [];
      this._mark = null;
    }
    get byteLength() { return this.bytes.length; }
    get bitLength() { return this.bytes.length * 8; }
    atEnd() { return this.bitPos >= this.bitLength; }

    readBit() {
      const p = this.bitPos;
      if (p >= this.bitLength) throw new BM.StreamError("ran off the end of the stream");
      this.bitPos = p + 1;
      return (this.bytes[p >> 3] >> (p & 7)) & 1;
    }
    /* n <= 24: value assembled least-significant bit first. */
    readBits(n) {
      let v = 0;
      for (let i = 0; i < n; i++) v |= this.readBit() << i;
      return v >>> 0;
    }
    peekBits(n) {
      const save = this.bitPos;
      let v = 0;
      for (let i = 0; i < n && this.bitPos < this.bitLength; i++) v |= this.readBit() << i;
      this.bitPos = save;
      return v >>> 0;
    }
    skipToByteBoundary() {
      const pad = (8 - (this.bitPos & 7)) & 7;
      if (pad) this.readBits(pad);
      return pad;
    }
    /* field(label, fn) records the bit span fn consumed. */
    field(label, fn, detail) {
      const start = this.bitPos;
      const value = fn();
      this.map.push({ start, end: this.bitPos, label, value, detail });
      return value;
    }
    bits(label, n, detail) {
      return this.field(label, () => this.readBits(n), detail);
    }
  }

  class BitWriter {
    constructor() {
      this.bytes = [];
      this.bitPos = 0;
      this.map = [];
      this._depth = [];
    }
    writeBits(n, value) {
      for (let i = 0; i < n; i++) {
        const bit = (value >>> i) & 1;
        const byteIdx = this.bitPos >> 3;
        if (byteIdx >= this.bytes.length) this.bytes.push(0);
        this.bytes[byteIdx] |= bit << (this.bitPos & 7);
        this.bitPos++;
      }
    }
    /* Prefix-code symbols are written with the code's bits reversed, so that a
       decoder reading LSB-first sees them most-significant-bit-first. */
    writeCode(code) { this.writeBits(code.length, code.reversed); }
    field(label, fn, detail) {
      const start = this.bitPos;
      const value = fn();
      this.map.push({ start, end: this.bitPos, label, value, detail });
      return value;
    }
    bits(label, n, value, detail) {
      return this.field(label, () => { this.writeBits(n, value); return value; }, detail);
    }
    alignToByte() {
      const pad = (8 - (this.bitPos & 7)) & 7;
      if (pad) this.writeBits(pad, 0);
      return pad;
    }
    finish() {
      if (this.bitPos & 7) this.bytes[this.bitPos >> 3] |= 0; /* trailing bits stay 0 */
      return Uint8Array.from(this.bytes);
    }
  }

  class StreamError extends Error {}

  BM.BitReader = BitReader;
  BM.BitWriter = BitWriter;
  BM.StreamError = StreamError;

  /* Shared helpers for the views. */
  BM.hex = (b) => b.toString(16).padStart(2, "0");
  BM.bin = (v, n) => v.toString(2).padStart(n, "0");
})(globalThis.BM || (globalThis.BM = {}));
