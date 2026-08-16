# `[package]`, `[dependencies]`, and lockfiles

Parent: [config](README.md). Delivery contract:
[packages and lockfiles](../../modules-and-delivery.md#packages-and-lockfiles).
Source anchors: metadata/validation in `src/config.rs`, lock and resolver logic in
`src/package.rs`, and module resolution in `src/module.rs`.

`[package]` describes the current package:

| Key | Default | Meaning |
|---|---|---|
| `name` | required when packaging | ASCII letters/digits/`-`/`_` |
| `version` | `0.1.0` | package semantic version |
| `abi` | current compiler ABI | compatibility gate |
| `entry` | `src/lib.lil` | exported package root |

Each `[dependencies.<name>]` entry has `path`, a version requirement, and expected
ABI. Paths are the current transport; there is no registry resolution hidden behind
the config.

`--write-lock` builds `lilscript.lock` from the complete declared graph and package
contents. Normal compilation loads rather than rewrites it and recomputes the expected
graph. A missing/stale lock, changed source hash, version/ABI mismatch, symlink/path
escape, conflicting package identity, or undeclared transitive access is an error.

Dependency visibility belongs to the importer. Root code sees only root dependencies;
a package sees only dependencies declared by that package. Bare subpaths resolve
inside the locked package root and cannot escape it.

Writing a lock also emits package effect summaries where possible. Those summaries
are optimization inputs, not permission to trust arbitrary package code: a changed
package invalidates the lock/effect evidence.

Package configuration changes the closed world and therefore invalidates semantic,
size, and reproducibility reports even when the entry source did not change.
