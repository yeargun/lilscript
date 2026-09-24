(() => {
  let seen = '';
  globalThis.hooksOf = function (elem) { return { set(e, v, n) { seen = e + ':' + v + ':' + n; return 'hooked'; } }; };
  globalThis.read = function () { return 'x'; };
})();
