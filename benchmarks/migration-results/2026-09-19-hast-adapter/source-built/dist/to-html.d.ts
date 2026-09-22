export interface CharacterReferences {
  omitOptionalSemicolons?: boolean
  useNamedReferences?: boolean
  useShortestReferences?: boolean
}

export type Quote = '"' | "'"
export type Space = "html" | "svg"

export interface Options {
  allowDangerousCharacters?: boolean | null
  allowDangerousHtml?: boolean | null
  allowParseErrors?: boolean | null
  bogusComments?: boolean | null
  characterReferences?: CharacterReferences | null
  closeEmptyElements?: boolean | null
  closeSelfClosing?: boolean | null
  collapseEmptyAttributes?: boolean | null
  omitOptionalTags?: boolean | null
  preferUnquoted?: boolean | null
  quote?: Quote | null
  quoteSmart?: boolean | null
  space?: Space | null
  tightAttributes?: boolean | null
  tightCommaSeparatedLists?: boolean | null
  tightDoctype?: boolean | null
  tightSelfClosing?: boolean | null
  upperDoctype?: boolean | null
  voids?: ReadonlyArray<string> | null
}

export function toHtml(tree: unknown[] | unknown, options?: Options | null): string
