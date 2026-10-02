"""Exact binary64 rounding boundaries, independently constructed with Decimal.

Pipe the generated tokens to both json-numbers.c and json-numbers.oracle.mjs.
Every exponent transition, random significands and the overflow boundary are
tested at the midpoint and immediately on either side, including signed values.
The perturbation is ten decimal places beyond the exact midpoint's last digit.
"""
from decimal import Decimal, localcontext
import math
import random
import struct

rng = random.Random(0x31415926)
patterns = {0, 0x7fefffffffffffff}
for exponent in range(2047):
    bits = exponent << 52
    patterns.update((bits, bits + 1))
    if bits:
        patterns.add(bits - 1)
for _ in range(512):
    patterns.add(rng.randrange(0x7ff0000000000000))

with localcontext() as context:
    context.prec = 1200
    for bits in sorted(patterns):
        number = struct.unpack('>d', bits.to_bytes(8, 'big'))[0]
        lower = Decimal.from_float(number)
        upper = (Decimal(2) ** 1024 if bits == 0x7fefffffffffffff
                 else Decimal.from_float(math.nextafter(number, math.inf)))
        midpoint = (lower + upper) / 2
        delta = Decimal(1).scaleb(midpoint.as_tuple().exponent - 10)
        for value in (midpoint - delta, midpoint, midpoint + delta):
            print(format(value, 'f'))
            print(format(-value, 'f'))
