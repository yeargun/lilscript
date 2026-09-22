interface Root {
  type: "root"
  children: Array<{type: string}>
}

export default function remarkBreaks(): (tree: Root) => undefined
