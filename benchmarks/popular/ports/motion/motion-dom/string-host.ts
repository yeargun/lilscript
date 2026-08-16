export function stringMatch(s: string, re: RegExp): RegExpMatchArray | null {
  return s.match(re)
}

export function stringReplace(
  s: string,
  re: RegExp,
  replacement: string | ((substring: string, ...args: any[]) => string)
): string {
  return s.replace(re, replacement as any)
}

export function stringSplit(s: string, sep: string): string[] {
  return s.split(sep)
}

export function stringSubstring(s: string, start: number, end?: number): string {
  if (end === undefined) return s.substring(start)
  return s.substring(start, end)
}

export function stringTrim(s: string): string {
  return s.trim()
}

export function isNaNNumber(v: any): boolean {
  return isNaN(v as any)
}

export function arrayFrom(list: ArrayLike<any>): any[] {
  return Array.from(list)
}
