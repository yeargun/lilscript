# Native Unicode and regular expressions

Pinned upstream: [bellard/quickjs at 535a7c250ff4a577ec36c3e103daab6dadeea650](https://github.com/bellard/quickjs/tree/535a7c250ff4a577ec36c3e103daab6dadeea650).
The included files are unchanged MIT-licensed sources; `manifest.json` records
their SHA-256 digests and Unicode version. The JavaScript interpreter is not
included or used by LilScript's native target.

Run `python3 scripts/vendor-native-text.py` from the repository to reproduce
the two C library recipes. The script verifies original digests, expands local
includes, gives exported declarations internal linkage, and separates one
colliding private table name. Opcode includes are expanded at both upstream
sites. File/line directives retain upstream diagnostic locations. The library's
unused/sign-compare warnings are locally isolated; source and provider warnings
are unaffected. Generated recipes retain the upstream license notices.

The owned UTF-16 interface, stateful RegExp behavior and resource controls live
in `src/program/runtime/unicode.c` and `regex.c`. Changes to those adapters must
be checked against an independent JavaScript runtime with matching Unicode data.
Case conversion/property escapes use Unicode 17.0.0. Source identity is part of
the compiler binary, and recipe changes advance its policy algorithm version.
