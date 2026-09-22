#!/home/azureuser/.nvm/versions/node/v24.11.1/bin/node
import { runCompilerReceipt } from "file:///home/azureuser/lilscript/finer/tools/compiler-receipt.mjs";
try { process.exitCode = runCompilerReceipt("/tmp/lilscript-to-hast-20260919-jmsKL8/portgate/wrappers/mdast-util-to-hastlil.json", process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
