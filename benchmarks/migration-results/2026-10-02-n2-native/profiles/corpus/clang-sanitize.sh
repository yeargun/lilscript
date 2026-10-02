#!/bin/sh
exec /home/azureuser/lilscript-work/toolchains/clang-18/root/usr/bin/clang-18 -Wall -Wextra -Werror -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer "$@"
