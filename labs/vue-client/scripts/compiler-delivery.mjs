// Package adapters install exact compiler artifacts. All behavior, flags and
// export selection live in source entries and the profile's TOML configuration.
import {spawn, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {existsSync} from 'node:fs';
import {mkdir, readFile, readdir, rename, rm, writeFile} from 'node:fs/promises';
import {dirname, isAbsolute, relative, resolve, sep} from 'node:path';
import {compilerPath, projectRoot} from '../tooling/compiler-path.mjs';

const sha256 = value => createHash('sha256').update(value).digest('hex');
const local = path => relative(projectRoot, path).split(sep).join('/');
const inside = (root, file) => {
  const path = resolve(root, file), name = relative(root, path);
  if (!name || name === '..' || name.startsWith(`..${sep}`) || isAbsolute(name)) throw Error(`Invalid artifact path: ${file}`);
  return path;
};
async function files(directory) {
  const result = [];
  for (const item of await readdir(directory, {withFileTypes:true})) {
    const path = resolve(directory, item.name);
    if (item.isDirectory()) result.push(...await files(path));
    else if (item.isFile()) result.push(path);
    else throw Error(`Unexpected source link: ${path}`);
  }
  return result.sort();
}
async function readJson(path) { return JSON.parse(await readFile(path, 'utf8')); }
async function sourceInputs() {
  const inputs = [...await files(resolve(projectRoot, 'src')), ...await files(resolve(projectRoot, 'config')),
    resolve(projectRoot, 'package-lock.json'), resolve(projectRoot, 'scripts/compiler-delivery.mjs')];
  return Promise.all(inputs.map(async path => [local(path), sha256(await readFile(path))]));
}
async function run(compiler, args) {
  await new Promise((accept, reject) => {
    const child = spawn(compiler, args, {cwd:projectRoot, stdio:'inherit'});
    child.on('error', reject);
    child.on('close', code => code === 0 ? accept() : reject(Error(`Compiler exited ${code}`)));
  });
}
export async function buildProfiles(names = ['development', 'test']) {
  const profiles = await readJson(resolve(projectRoot, 'config/compiler-profiles.json'));
  const compiler = compilerPath(), compilerSha256 = sha256(await readFile(compiler));
  // The adapter's cache covers all source owners, host adapters, configuration,
  // the dependency lock and every installed artifact. Compiler caches remain
  // independently keyed by their checked graph and resolved policy.
  const sourceReceipt = await sourceInputs();
  const reports = [];
  for (const name of [...new Set(names)]) {
    const profile = profiles[name];
    if (!profile) throw Error(`Unknown Vue delivery profile: ${name}`);
    const mode = process.env.LILSCRIPT_BUILD_MODE ?? profile.mode;
    if (!['development','production'].includes(mode)) throw Error(`Invalid build mode: ${mode}`);
    const config = resolve(projectRoot, profile.config);
    const fingerprint = sha256(JSON.stringify({compilerSha256, sourceReceipt, name, mode, profile,
      work:process.env.LILSCRIPT_LOGICAL_WORK ?? '40000000000'}));
    const receiptPath = resolve(projectRoot, '.tmp/compiler-receipts', `${name}.json`);
    const previous = existsSync(receiptPath) ? await readJson(receiptPath) : null;
    if (previous?.fingerprint === fingerprint && !process.env.VUELIL_FORCE_BUILD) {
      let valid = true;
      for (const [file, hash] of Object.entries(previous.artifacts)) {
        const path = inside(projectRoot, file);
        if (!existsSync(path) || sha256(await readFile(path)) !== hash) { valid = false; break; }
      }
      if (valid) { reports.push(previous); continue; }
    }
    const stage = resolve(projectRoot, '.tmp', `vue-${name}-${process.pid}`);
    await rm(stage, {recursive:true, force:true});
    await mkdir(stage, {recursive:true});
    const args = ['--config', config, '--target', 'js-module', '--mode', mode,
      '--logical-work', process.env.LILSCRIPT_LOGICAL_WORK ?? '40000000000', '--out-dir', stage];
    const policy = spawnSync(compiler, [...args, '--print-policy'], {cwd:projectRoot, encoding:'utf8', maxBuffer:16*1024*1024});
    if (policy.status !== 0) throw Error(policy.stderr || 'Could not resolve compiler policy');
    const started = performance.now();
    try {
      await run(compiler, args);
      const manifestBytes = await readFile(resolve(stage, 'lilscript.manifest.json'));
      const manifest = JSON.parse(manifestBytes);
      if (![3,4,5].includes(manifest.version) || !manifest.source_sha256) throw Error('Compiler delivery receipt is missing');
      const artifacts = {};
      for (const group of manifest.outputs) for (const file of group.files) {
        if (!/^(packages\/vuelil|tests|artifacts)\//.test(file.file) || artifacts[file.file]) throw Error(`Invalid or duplicate Vue artifact: ${file.file}`);
        const bytes = await readFile(inside(stage, file.file));
        if (sha256(bytes) !== file.sha256 || bytes.length !== file.bytes) throw Error(`Compiler artifact hash mismatch: ${file.file}`);
        artifacts[file.file] = file.sha256;
      }
      // Validate the entire staged graph before installing any file. Source and
      // unrelated lab evidence never live under a replaceable output directory.
      if (JSON.stringify(await sourceInputs()) !== JSON.stringify(sourceReceipt)) {
        throw Error('Vue sources changed during compilation; rerun the build');
      }
      for (const file of Object.keys(artifacts)) {
        const path = inside(projectRoot, file);
        await mkdir(dirname(path), {recursive:true});
        await rename(inside(stage, file), path);
      }
      for (const file of Object.keys(previous?.artifacts ?? {})) {
        if (!artifacts[file] && /\/chunks\//.test(file)) await rm(inside(projectRoot, file), {force:true});
      }
      const report = {schema:1, profile:name, fingerprint, compiler:local(compiler), compilerSha256,
        config:profile.config, configSha256:sha256(await readFile(config)), sourceReceipt,
        mode, arguments:args, resolvedPolicy:JSON.parse(policy.stdout),
        sourceSha256:manifest.source_sha256, manifestSha256:sha256(manifestBytes),
        compileWallMs:Math.round(performance.now()-started), artifacts};
      await mkdir(dirname(receiptPath), {recursive:true});
      await writeFile(receiptPath.replace(/\.json$/, '.manifest.json'), manifestBytes);
      await writeFile(receiptPath, `${JSON.stringify(report,null,2)}\n`);
      reports.push(report);
    } finally { await rm(stage, {recursive:true,force:true}); }
  }
  return reports;
}

export async function buildFoundation() {
  const requested = process.env.VUELIL_PROFILES?.split(',').filter(Boolean) ?? ['development','test'];
  const reports = await buildProfiles(requested);
  console.log(JSON.stringify({profiles:reports.map(row=>({name:row.profile, files:Object.keys(row.artifacts).length, fingerprint:row.fingerprint}))}));
}
