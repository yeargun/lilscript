import { spawnSync } from "node:child_process";

export function run(
  command,
  args,
  { cwd, env = process.env, capture = false } = {},
) {
  const result = spawnSync(command, args, {
    cwd,
    env,
    encoding: "utf8",
    maxBuffer: 50 * 1024 * 1024,
    stdio: capture ? ["inherit", "pipe", "pipe"] : "inherit",
  });
  if (capture && result.stderr) process.stderr.write(result.stderr);
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} exited with ${result.status}`);
  }
  return capture ? result.stdout.trim() : "";
}
