const inputs=["# Hello\n\nA **small** [link](https://example.com).\n","- one\n- two\n","plain *text*\n"];
const expected=["<h1>Hello</h1>\n<p>A <strong>small</strong> <a href=\"https://example.com\">link</a>.</p>\n",
  "<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n","<p>plain <em>text</em></p>\n"];
export function run(api,iterations) {
  let characters=0;
  for(let index=0;index<iterations;index++) for(let item=0;item<inputs.length;item++) {
    const output=api.parse(inputs[item]);
    if(output!==expected[item]) throw new Error(`Markdown oracle differs at ${item}`);
    characters+=output.length;
  }
  return {oracle:{characters,last:api.parse(inputs[0])},counters:{parse_calls:iterations*3+1,input_documents:inputs.length}};
}
