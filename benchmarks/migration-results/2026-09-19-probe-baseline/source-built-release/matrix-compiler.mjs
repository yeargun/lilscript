#!/home/azureuser/.nvm/versions/node/v24.11.1/bin/node
import {runCompilerReceipt} from "file:///home/azureuser/lilscript/finer/tools/compiler-receipt.mjs";
try { process.exitCode=runCompilerReceipt("/tmp/lilscript-probe-baseline-20260919-kKoGmp/portgate/matrix-wrapper.json",process.argv.slice(2)); } catch(error) { console.error(error.message); process.exitCode=1; }
