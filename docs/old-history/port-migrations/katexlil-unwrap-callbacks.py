# unwrap.py <src-dir>: `X["handler"|"htmlBuilder"|"mathmlBuilder"] = JS.methodN((JsValue self, ...) => BODY);`
# becomes `X[...] = (...) => BODY;` when BODY never names `self`. Prints how many.
import sys, re, os
KEYS = ('"handler"', '"htmlBuilder"', '"mathmlBuilder"')
def skip_string(s, i):
    q = s[i]; i += 1
    while i < len(s):
        if s[i] == '\\': i += 2; continue
        if s[i] == q: return i + 1
        i += 1
    return i
def match(s, i, open_c, close_c):
    # s[i] == open_c; return index of the matching close_c
    depth = 0
    while i < len(s):
        c = s[i]
        if c in '"\'`': i = skip_string(s, i); continue
        if s.startswith('//', i): i = s.index('\n', i); continue
        if c == open_c: depth += 1
        elif c == close_c:
            depth -= 1
            if depth == 0: return i
        i += 1
    raise ValueError('unbalanced')
def uses_self(body):
    out = []; i = 0
    while i < len(body):
        c = body[i]
        if c in '"\'`': j = skip_string(body, i); out.append(' ' * (j - i)); i = j; continue
        out.append(c); i += 1
    return re.search(r'(?<![\w$])self(?![\w$])', ''.join(out)) is not None
total = 0
for root, _, files in os.walk(sys.argv[1]):
    for f in files:
        if not f.endswith('.lil'): continue
        p = os.path.join(root, f); s = open(p).read(); changed = 0
        pat = re.compile(r'\[(%s)\]\s*=\s*JS\.method(\d)\(' % '|'.join(re.escape(k) for k in KEYS))
        pos = 0
        while True:
            m = pat.search(s, pos)
            if not m: break
            call_open = m.end() - 1                     # '(' of JS.methodN(
            lam = call_open + 1
            while s[lam] in ' \n': lam += 1
            if s[lam] != '(' : pos = m.end(); continue
            params_end = match(s, lam, '(', ')')
            params = s[lam + 1:params_end]
            pm = re.match(r'\s*JsValue\s+self\s*(,|$)', params)
            if not pm: pos = m.end(); continue
            call_close = match(s, call_open, '(', ')')
            body = s[params_end + 1:call_close]
            if uses_self(body): pos = m.end(); continue
            rest = params[pm.end():].strip() if pm.group(1) == ',' else ''
            new = '(' + rest + ')' + body
            s = s[:call_open - len('JS.methodN')] + new + s[call_close + 1:]
            changed += 1
            pos = call_open - len('JS.methodN') + len(new)
        if changed:
            open(p, 'w').write(s); total += changed
print(total)
