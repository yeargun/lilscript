#!/bin/bash
# brotli.sh <js file>: raw gzip brotli sizes via the repository codec
SP=/tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad
$SP/bin/lilscript-codec --json "$1" | python3 -c "import json,sys;a=json.load(sys.stdin)['artifacts'][0];print(a['raw'],a.get('gzip9',a.get('gzip','?')),a['brotli11'])"
