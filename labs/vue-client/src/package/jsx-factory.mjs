export function createJsx(h) {
  return function jsx(type, props, key) {
    const {children, ...rest} = props ?? {};
    if (arguments.length > 2) rest.key = key;
    return h(type, rest, children);
  };
}
