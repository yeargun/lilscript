static LS_NATIVE_UNUSED inline int32_t ls_to_i32(double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof bits);
    unsigned biased = (unsigned)((bits >> 52) & UINT64_C(2047));
    if (biased < 1023 || biased >= 1107) return 0;
    unsigned exponent = biased - 1023;
    uint64_t significand = (bits & UINT64_C(0x000fffffffffffff)) | UINT64_C(0x0010000000000000);
    uint32_t low = exponent < 52 ? (uint32_t)(significand >> (52 - exponent))
                                : (uint32_t)((uint32_t)significand << (exponent - 52));
    if (bits >> 63) low = (uint32_t)(UINT32_C(0) - low);
    return ls_from_u32(low);
}
