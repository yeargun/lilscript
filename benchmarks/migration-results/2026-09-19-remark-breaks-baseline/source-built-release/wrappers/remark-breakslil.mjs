#!/home/azureuser/.nvm/versions/node/v24.11.1/bin/node
import { runCompilerReceipt } from "file:///home/azureuser/lilscript/finer/tools/compiler-receipt.mjs";
try { process.exitCode = runCompilerReceipt("/tmp/lilscript-remark-breaks-20260919-U8OjGp/portgate/wrappers/remark-breakslil.json", process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
