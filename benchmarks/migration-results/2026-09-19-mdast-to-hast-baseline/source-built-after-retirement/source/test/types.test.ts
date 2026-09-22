import {
  defaultFootnoteBackContent,
  defaultFootnoteBackLabel,
  defaultHandlers,
  toHast
} from '@itslil/mdast-util-to-hast'
import type {
  FootnoteBackContentTemplate,
  FootnoteBackLabelTemplate,
  Handler,
  Handlers,
  Options,
  Raw,
  RawData,
  State
} from '@itslil/mdast-util-to-hast'
import type {Root as HastRoot} from 'hast'
import type {Root as MdastRoot} from 'mdast'

const tree: MdastRoot = {type: 'root', children: []}
const options: Options = {allowDangerousHtml: true}
const result = toHast(tree, options)
const content: ReturnType<FootnoteBackContentTemplate> =
  defaultFootnoteBackContent(0, 2)
const label: ReturnType<FootnoteBackLabelTemplate> =
  defaultFootnoteBackLabel(0, 2)
const handler: Handler = (state, node) => state.applyData(node, {
  type: 'text',
  value: label
})
const handlers: Handlers = {text: handler}

const raw: Raw = {type: 'raw', value: String(content)}
const rawData: RawData = {}
const state = null as State | null
const hast = result as HastRoot

void handlers
void defaultHandlers
void raw
void rawData
void state
void hast
