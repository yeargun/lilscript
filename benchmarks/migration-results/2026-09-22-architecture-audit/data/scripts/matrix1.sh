#!/bin/bash
for P in probelil markedlil zodlil katexlil; do
  /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/time1.sh /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/bin/577d472d/lilscript $P semantic prod
  CFG=l0.toml /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/time1.sh /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/bin/577d472d/lilscript $P semantic l0
  /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/time1.sh /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/bin/577d472d/lilscript $P semantic dev --mode development
  CFG=l0.toml /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/time1.sh /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/bin/577d472d/lilscript $P legacy l0
  /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/time1.sh /tmp/claude-1000/-home-azureuser-lilscript/c2205f70-99b9-45bf-a2c2-793d7378d92b/scratchpad/bin/577d472d/lilscript $P legacy dev --mode development
done
echo DONE
