function scale(value) {
  function bump(inner) {
    return inner + 3 | 0;
  }
  return bump(value) * 2 | 0;
}
console.log(scale(5));
