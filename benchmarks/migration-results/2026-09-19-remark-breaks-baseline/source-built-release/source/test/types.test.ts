import type {Root} from "mdast"
import remarkBreaks from "@itslil/remark-breaks"
import {unified} from "unified"

const tree: Root = {type: "root", children: []}
const transform = remarkBreaks()
const result: undefined = transform(tree)
unified().use(remarkBreaks)

// @ts-expect-error: the upstream plugin takes no options.
remarkBreaks({})

// @ts-expect-error: the upstream package has no named runtime export.
import {remarkBreaks as namedRemarkBreaks} from "@itslil/remark-breaks"

void result
void namedRemarkBreaks
