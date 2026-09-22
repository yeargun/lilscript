#!/home/azureuser/.nvm/versions/node/v24.11.1/bin/node
import { runCompilerReceipt } from "file:///home/azureuser/lilscript/finer/tools/compiler-receipt.mjs";
try { process.exitCode = runCompilerReceipt("/tmp/lilscript-remark-rehypelil-baseline-x5Kwul/portgate/wrappers/remark-rehypelil.json", process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
