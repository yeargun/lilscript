# SolidLil Web verified surfaces

The release comparison is the 46-export client-rendering bundle. SSR and hydration are explicitly excluded from that scope. The complete 73-export browser compatibility bundle remains measured separately. Both surfaces have exact export ledgers and executable behavior parity.

| Surface | Metric | Solid | SolidLil | Ratio |
| --- | --- | ---: | ---: | ---: |
| Client rendering | Brotli-11 | 10859 B | 10643 B | 0.980 |
| Client rendering | Gzip-9 | 12103 B | 11939 B | 0.986 |
| Client rendering | Raw | 36814 B | 36980 B | 1.005 |
| Full compatibility | Brotli-11 | 11655 B | 11667 B | 1.001 |
| Full compatibility | Gzip-9 | 12977 B | 13074 B | 1.007 |
| Full compatibility | Raw | 39583 B | 40467 B | 1.022 |
