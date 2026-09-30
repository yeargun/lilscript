let rows;
export function install() { rows=[]; console.log = (...args) => rows.push(args.map(String).join(" ")); }
export function observe() { return rows; }
