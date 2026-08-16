export function runJqueryContract($) {
  let passed = 0;

  if ($.isPlainObject({ a: 1 }) && !$.isPlainObject([])) passed += 1;
  if ($.isEmptyObject({}) && !$.isEmptyObject({ a: 1 })) passed += 1;

  const merged = $.extend(true, { a: { b: 1 } }, { a: { c: 2 }, d: 3 });
  if (merged.a.b === 1 && merged.a.c === 2 && merged.d === 3) passed += 1;

  const grepped = $.grep([1, 2, 3, 4, 5], (n) => n % 2 === 1);
  if (grepped.join(",") === "1,3,5") passed += 1;

  const mapped = $.map([1, 2, 3], (n) => n * 10);
  if (mapped.join(",") === "10,20,30") passed += 1;

  const made = $.makeArray("ab");
  if (made.length === 1 && made[0] === "ab") passed += 1;

  let sum = 0;
  $.each({ x: 2, y: 5 }, (_k, v) => {
    sum += v;
  });
  if (sum === 7) passed += 1;

  if ($.inArray(3, [1, 2, 3, 4]) === 2) passed += 1;

  console.log(`jquery:${passed}:ok`);
  return passed;
}
