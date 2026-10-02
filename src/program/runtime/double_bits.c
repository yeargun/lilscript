static LS_NATIVE_UNUSED inline double ls_f64_bits(uint64_t bits) {
    double value;
    memcpy(&value, &bits, sizeof value);
    return value;
}
