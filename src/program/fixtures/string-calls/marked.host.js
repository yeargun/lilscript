const units = value => {
  const result = [];
  for (let index = 0; index < value.length; index++) result.push(value.charCodeAt(index));
  return result;
};
const unusual = "A\0\ud800\ud834\udd1e\udfffZ";
events.push(["empty", units(library.slice("", -8, 20)), units(library.sliceFrom("", 0))]);
events.push(["negative", units(library.slice("abcdef", -3, -1))]);
events.push(["clamp", units(library.slice("abcdef", -100, 100))]);
events.push(["reversed", units(library.slice("abcdef", 4, 2))]);
events.push(["oversized", units(library.sliceFrom("abcdef", 100))]);
events.push(["tail", units(library.sliceFrom("abcdef", -2))]);
events.push(["surrogates", units(library.slice(unusual, 1, 6)), units(library.sliceFrom(unusual, 4))]);
events.push(["separate-units", library.splitUsing(unusual, "").map(units)]);
events.push(["empty-split", library.splitUsing("", ""), library.splitUsing("", ",")]);
events.push(["literal-separator", library.splitUsing("a||b||||", "||")]);
for (const text of ["", "only", "\n", "a\n\nb\n", "\r\nnext", unusual + "\nlast\n"]) {
  events.push(["lines", units(text), library.splitLines(text).map(units), units(library.firstLine(text)), units(library.roundTrip(text))]);
}
events.push(["api", library.slice.length, library.sliceFrom.length, library.firstLine.length, library.splitLines.length]);
