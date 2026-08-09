import clsx from "clsx";

const parts = [];
let passed = 0;

function check(actual, expected) {
  parts.push(actual);
  if (actual === expected) passed += 1;
}

function dict(entries) {
  const out = {};
  for (let i = 0; i < entries.length; i += 1) {
    out[entries[i][0]] = entries[i][1];
  }
  return out;
}

check(clsx("foo", true && "bar", "baz"), "foo bar baz");
check(clsx(true, false, "", null, undefined, 0), "");
check(clsx(0, 1, 2), "1 2");
check(clsx(dict([["foo", true], ["bar", false], ["baz", true]])), "foo baz");
check(
  clsx(
    dict([["foo", true]]),
    dict([["bar", false]]),
    null,
    dict([["--foobar", "hello"]]),
  ),
  "foo --foobar",
);
check(clsx(["foo", 0, false, "bar"]), "foo bar");
check(
  clsx(["foo"], ["", 0, false, "bar"], [["baz", [["hello"], "there"]]]),
  "foo bar baz hello there",
);
check(
  clsx(
    "foo",
    [1 && "bar", dict([["baz", false], ["bat", null]]), ["hello", ["world"]]],
    "cya",
  ),
  "foo bar hello world cya",
);
check(clsx("foo", "foo", dict([["foo", true]]), ["foo"]), "foo foo foo foo");
check(clsx("hello", dict([["world", 1], ["push", true]])), "hello world push");
check(clsx(1, "a", 12), "1 a 12");
check(clsx(dict([])), "");
check(clsx([[[]]]), "");

console.log(`clsx:${passed}:${parts.join("|")}`);
