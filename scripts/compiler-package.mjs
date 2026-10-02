// Port-owned package delivery. Copy this module into a port's scripts directory;
// the installed package never depends on a compiler checkout or post-minifier.
import {spawn, spawnSync} from 'node:child_process'
import {createHash} from 'node:crypto'
import {constants, existsSync} from 'node:fs'
import {access, cp, mkdir, readFile, readdir, rename, rm, stat, writeFile} from 'node:fs/promises'
import {delimiter, dirname, isAbsolute, join, relative, resolve, sep} from 'node:path'

const digest = bytes => createHash('sha256').update(bytes).digest('hex')
const inside = (root, file) => {
  const path = resolve(root, file), local = relative(root, path)
  if (!local || local === '..' || local.startsWith(`..${sep}`) || isAbsolute(local)) throw Error(`Invalid output path: ${file}`)
  return path
}

export async function compilerPath(root, explicit) {
  const pinned = explicit ?? process.env.LILSCRIPT_COMPILER
  const candidates = pinned ? [pinned] : [resolve(process.env.LILSCRIPT_ROOT ?? resolve(root, '../lilscript'), 'target/release/lilscript'), 'lilscript']
  for (const candidate of candidates) {
    const paths = candidate.includes(sep) ? [resolve(candidate)] : (process.env.PATH ?? '').split(delimiter).map(dir => resolve(dir, candidate))
    for (const path of paths) {
      try { await access(path, constants.X_OK) } catch { continue }
      if (spawnSync(path, ['--version'], {stdio:'ignore'}).status === 0) return path
    }
  }
  throw Error('LilScript compiler not found. Set LILSCRIPT_COMPILER to the pinned release executable.')
}

export async function buildPackage({root, profiles, assets = [], providerAssets = [], aliases = {}, compiler: explicit, sideEffects = true}) {
  const compiler = await compilerPath(root, explicit)
  const scratch = join(root, '.tmp'), stage = join(scratch, `package-${process.pid}`)
  const dist = join(root, 'dist'), previous = join(scratch, `package-previous-${process.pid}`)
  await mkdir(scratch, {recursive:true})
  await rm(stage, {recursive:true, force:true})
  await mkdir(stage, {recursive:true})
  const report = {schema:3, compiler, compilerSha256:digest(await readFile(compiler)), profiles:[], artifacts:{}, providers:[], writtenBy:{}}
  if (existsSync(join(root, 'package-lock.json'))) report.dependencyLockSha256 = digest(await readFile(join(root, 'package-lock.json')))
  const outputs = [], effects = new Set()
  let installed = false
  try {
    for (const profile of profiles) {
      if (!/^[a-z0-9-]+$/.test(profile.name)) throw Error(`Invalid profile: ${profile.name}`)
      const mode = process.env.LILSCRIPT_BUILD_MODE ?? profile.mode ?? 'production'
      if (!['development','production'].includes(mode)) throw Error(`Invalid build mode: ${mode}`)
      const target = profile.target ?? 'js-module'
      if (!['js-module','js'].includes(target)) throw Error(`Invalid package target: ${target}`)
      const config = resolve(root, profile.config), output = join(stage, '.lilscript', profile.name)
      const args = [...(profile.entry ? [resolve(root, profile.entry)] : []), '--config', config, '--target', target, '--mode', mode, '--out-dir', output]
      const policy = spawnSync(compiler, [...args, '--print-policy'], {cwd:root, encoding:'utf8', maxBuffer:16 * 1024 * 1024})
      if (policy.status !== 0) throw Error(policy.stderr || `Could not resolve ${profile.name} policy`)
      const started = performance.now()
      await new Promise((accept, reject) => {
        const child = spawn(compiler, args, {cwd:root, stdio:'inherit'})
        child.on('error', reject)
        child.on('close', status => status === 0 ? accept() : reject(Error(`LilScript ${profile.name} exited ${status}`)))
      })
      const compileWallMs = Math.round(performance.now() - started)
      const manifestBytes = await readFile(join(output, 'lilscript.manifest.json'))
      const manifest = JSON.parse(manifestBytes)
      if (![3,4,5].includes(manifest.version) || !manifest.source_sha256) throw Error(`${profile.name} requires a compiler delivery manifest with source receipts`)
      for (const group of manifest.outputs) {
        const groupName = `${profile.name}/${group.output}`
        for (const file of group.files) {
          if (report.artifacts[file.file]) throw Error(`Output collision: ${file.file}`)
          const from = inside(output, file.file), to = inside(stage, file.file)
          if (relative(stage, to).split(sep)[0] === '.lilscript') throw Error('Compiler output uses reserved .lilscript directory')
          const bytes = await readFile(from)
          if (digest(bytes) !== file.sha256 || bytes.length !== file.bytes) throw Error(`Compiler manifest mismatch: ${file.file}`)
          await mkdir(dirname(to), {recursive:true})
          await rename(from, to)
          report.artifacts[file.file] = {sha256:file.sha256, raw:file.bytes, codec:group.codec, codecBytes:file.codec_bytes, output:groupName, policyFingerprint:group.policy_fingerprint}
          report.writtenBy[`dist/${file.file}`] = 'compiler'
        }
        for (const file of group.side_effects) effects.add(`./dist/${file}`)
        outputs.push({...group, output:groupName})
      }
      report.profiles.push({name:profile.name, config:relative(root, config), configSha256:digest(await readFile(config)), mode, target, sourceSha256:manifest.source_sha256, manifestSha256:digest(manifestBytes), resolvedPolicy:JSON.parse(policy.stdout), compileWallMs})
      await rm(output, {recursive:true, force:true})
      await writeFile(join(stage, '.lilscript', `${profile.name}.json`), manifestBytes)
    }
    // Aliases are exact copies of a compiler artifact, never edited source.
    // Keep aliases beside their source so relative module requests retain meaning.
    for (const [destination, source] of Object.entries(aliases)) {
      if (dirname(destination) !== dirname(source) || report.artifacts[destination] || !report.artifacts[source]) throw Error(`Invalid compiler alias: ${destination} -> ${source}`)
      await cp(inside(stage, source), inside(stage, destination))
      report.artifacts[destination] = {...report.artifacts[source], aliasOf:source}
      report.writtenBy[`dist/${destination}`] = 'compiler'
      if (effects.has(`./dist/${source}`)) effects.add(`./dist/${destination}`)
    }
    for (const {source, destination} of assets) {
      const target = inside(stage, destination)
      const checkAsset = async path => {
        if ((await stat(path)).isDirectory()) {
          for (const name of await readdir(path)) await checkAsset(join(path, name))
        } else if (/\.(?:m?js|cjs)$/.test(path)) throw Error(`Runtime assets must be compiler outputs or explicit dependency providers: ${path}`)
      }
      if (/\.(?:m?js|cjs)$/.test(destination)) throw Error(`Runtime assets must be compiler outputs or explicit dependency providers: ${destination}`)
      await checkAsset(resolve(root, source))
      await mkdir(dirname(target), {recursive:true})
      await cp(resolve(root, source), target, {recursive:true, errorOnExist:true, force:false})
      if (destination.endsWith('.css')) effects.add(`./dist/${destination}`)
    }
    // Existing third-party runtimes (for example Microsoft's TypeScript worker)
    // remain dependency artifacts. Copy exact installed package bytes and record
    // their owner and digest; never attribute them to the LilScript compiler.
    for (const provider of providerAssets) {
      if (!/^(@[a-z0-9_.-]+\/)?[a-z0-9_.-]+$/.test(provider.package)) throw Error('Invalid dependency provider package')
      const base = resolve(root, 'node_modules', provider.package)
      const pkg = JSON.parse(await readFile(join(base, 'package.json'), 'utf8'))
      if (pkg.version !== provider.version) throw Error(`Dependency provider version changed: ${provider.package}`)
      const source = inside(base, provider.source), target = inside(stage, provider.destination)
      if (existsSync(target) || relative(stage, target).split(sep)[0] === '.lilscript') throw Error(`Dependency artifact collision: ${provider.destination}`)
      const bytes = await readFile(source), sha256 = digest(bytes)
      await mkdir(dirname(target), {recursive:true})
      await writeFile(target, bytes, {flag:'wx'})
      report.providers.push({...provider, sha256, raw:bytes.length})
      report.writtenBy[`dist/${provider.destination}`] = `${provider.package}@${provider.version}`
    }
    const receipt = {version:5, outputs, aliases, providers:report.providers, source_sha256:digest(JSON.stringify(report.profiles.map(profile => [profile.name, profile.sourceSha256])))}
    await writeFile(join(stage, 'lilscript.manifest.json'), `${JSON.stringify(receipt, null, 2)}\n`)
    report.manifestSha256 = digest(await readFile(join(stage, 'lilscript.manifest.json')))
    if (existsSync(dist)) await rename(dist, previous)
    try { await rename(stage, dist); installed = true }
    catch (error) { if (existsSync(previous)) await rename(previous, dist); throw error }
    if (sideEffects) {
      const packagePath = join(root, 'package.json'), pkg = JSON.parse(await readFile(packagePath, 'utf8'))
      pkg.sideEffects = [...effects].sort()
      await writeFile(packagePath, `${JSON.stringify(pkg, null, 2)}\n`)
    }
    await writeFile(join(scratch, 'build-report.json'), `${JSON.stringify(report, null, 2)}\n`)
    await rm(previous, {recursive:true, force:true})
    console.log(`Built ${Object.keys(report.artifacts).length} compiler-written artifacts (${profiles.map(profile => profile.name).join(', ')})`)
    return report
  } finally {
    if (!installed) await rm(stage, {recursive:true, force:true})
  }
}
