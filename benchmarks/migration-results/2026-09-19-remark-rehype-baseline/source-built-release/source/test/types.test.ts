import remarkRehype, {
  defaultFootnoteBackContent,
  defaultFootnoteBackLabel,
  defaultHandlers
} from '@itslil/remark-rehype'
import type {Options} from '@itslil/remark-rehype'
import type {Root as HastRoot} from 'hast'
import type {Data as MdastData} from 'mdast'
import remarkParse from 'remark-parse'
import {unified} from 'unified'

const options: Options = {
  allowDangerousHtml: true,
  handlers: {
    text(state, node) {
      return state.applyData(node, {type: 'text', value: String(node.value)})
    }
  }
}

unified().use(remarkParse).use(remarkRehype, options)
unified().use(remarkParse).use(remarkRehype, unified(), options)

const content = defaultFootnoteBackContent(0, 2)
const label = defaultFootnoteBackLabel(0, 2)
const hast: HastRoot = {type: 'root', children: [{type: 'raw', value: '<b>'}]}
const data: MdastData = {hName: 'b', hProperties: {className: ['example']}}

void content
void data
void defaultHandlers
void hast
void label
