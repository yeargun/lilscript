export interface MarkedOptions {
  gfm?: boolean
  breaks?: boolean
  pedantic?: boolean
  silent?: boolean
  async?: boolean
}

export interface Marked {
  (src: string, options?: MarkedOptions | null): string
  parse(src: string, options?: MarkedOptions | null): string
  parseInline(src: string, options?: MarkedOptions | null): string
  setOptions(options: MarkedOptions): Marked
  options(options: MarkedOptions): Marked
  getDefaults(): MarkedOptions
  defaults: MarkedOptions
}

export function parse(src: string, options?: MarkedOptions | null): string
export function parseInline(src: string, options?: MarkedOptions | null): string
export function setOptions(options: MarkedOptions): Marked
export const options: typeof setOptions
export function getDefaults(): MarkedOptions
export const defaults: MarkedOptions
export const marked: Marked
export default marked
