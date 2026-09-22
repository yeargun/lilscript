import type {
  Data,
  Element,
  ElementContent,
  Literal,
  Nodes as HastNodes,
  Parents as HastParents,
  Properties,
  RootContent,
  Text
} from 'hast'
import type {
  Blockquote,
  Break,
  Code,
  Delete,
  Definition,
  Emphasis,
  FootnoteDefinition,
  FootnoteReference,
  Heading,
  Html,
  Image,
  ImageReference,
  InlineCode,
  Link,
  LinkReference,
  List,
  ListItem,
  Nodes as MdastNodes,
  Paragraph,
  Parents as MdastParents,
  Root as MdastRoot,
  Strong,
  Table,
  TableCell,
  TableRow,
  Text as MdastText,
  ThematicBreak
} from 'mdast'
import type {VFile} from 'vfile'

export type FootnoteBackContentTemplate = (
  referenceIndex: number,
  rereferenceIndex: number
) => Array<ElementContent> | ElementContent | string

export type FootnoteBackLabelTemplate = (
  referenceIndex: number,
  rereferenceIndex: number
) => string

export type Handler = (
  state: State,
  node: any,
  parent: MdastParents | undefined
) => Array<ElementContent> | ElementContent | undefined

export type Handlers = Partial<Record<MdastNodes['type'], Handler>>

export type Options = {
  allowDangerousHtml?: boolean | null | undefined
  clobberPrefix?: string | null | undefined
  file?: VFile | null | undefined
  footnoteBackContent?: FootnoteBackContentTemplate | string | null | undefined
  footnoteBackLabel?: FootnoteBackLabelTemplate | string | null | undefined
  footnoteLabel?: string | null | undefined
  footnoteLabelProperties?: Properties | null | undefined
  footnoteLabelTagName?: string | null | undefined
  handlers?: Handlers | null | undefined
  passThrough?: Array<MdastNodes['type']> | null | undefined
  unknownHandler?: Handler | null | undefined
}

export type State = {
  all: (node: MdastNodes) => Array<ElementContent>
  applyData: <Type extends HastNodes>(
    from: MdastNodes,
    to: Type
  ) => Element | Type
  definitionById: Map<string, Definition>
  footnoteById: Map<string, FootnoteDefinition>
  footnoteCounts: Map<string, number>
  footnoteOrder: Array<string>
  handlers: Handlers
  one: (
    node: MdastNodes,
    parent: MdastParents | undefined
  ) => Array<ElementContent> | ElementContent | undefined
  options: Options
  patch: (from: MdastNodes, node: HastNodes) => undefined
  wrap: <Type extends RootContent>(
    nodes: Array<Type>,
    loose?: boolean | undefined
  ) => Array<Text | Type>
}

type DefaultHandlers = {
  blockquote: (state: State, node: Blockquote) => Element
  break: (state: State, node: Break) => Array<Element | Text>
  code: (state: State, node: Code) => Element
  definition: () => undefined
  delete: (state: State, node: Delete) => Element
  emphasis: (state: State, node: Emphasis) => Element
  footnoteDefinition: () => undefined
  footnoteReference: (state: State, node: FootnoteReference) => Element
  heading: (state: State, node: Heading) => Element
  html: (state: State, node: Html) => Element | Raw | undefined
  image: (state: State, node: Image) => Element
  imageReference: (
    state: State,
    node: ImageReference
  ) => Array<ElementContent> | ElementContent
  inlineCode: (state: State, node: InlineCode) => Element
  link: (state: State, node: Link) => Element
  linkReference: (
    state: State,
    node: LinkReference
  ) => Array<ElementContent> | ElementContent
  list: (state: State, node: List) => Element
  listItem: (
    state: State,
    node: ListItem,
    parent: MdastParents | undefined
  ) => Element
  paragraph: (state: State, node: Paragraph) => Element
  root: (state: State, node: MdastRoot) => HastParents
  strong: (state: State, node: Strong) => Element
  table: (state: State, node: Table) => Element
  tableCell: (state: State, node: TableCell) => Element
  tableRow: (
    state: State,
    node: TableRow,
    parent: MdastParents | undefined
  ) => Element
  text: (state: State, node: MdastText) => Element | Text
  thematicBreak: (state: State, node: ThematicBreak) => Element
  toml: () => undefined
  yaml: () => undefined
}

export const defaultHandlers: DefaultHandlers

export function defaultFootnoteBackContent(
  referenceIndex: number,
  rereferenceIndex: number
): Array<ElementContent>

export function defaultFootnoteBackLabel(
  referenceIndex: number,
  rereferenceIndex: number
): string

export function toHast(
  tree: MdastNodes,
  options?: Options | null | undefined
): HastNodes

export interface Raw extends Literal {
  type: 'raw'
  data?: RawData | undefined
}

export interface RawData extends Data {}

declare module 'hast' {
  interface ElementData {
    meta?: string | null | undefined
  }

  interface ElementContentMap {
    raw: Raw
  }

  interface RootContentMap {
    raw: Raw
  }
}

declare module 'mdast' {
  interface Data {
    hChildren?: ElementContent[] | undefined
    hName?: string | undefined
    hProperties?: Properties | undefined
  }
}
