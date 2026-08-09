# Integrated labs

`labs/solid-client` is the formerly sibling `lilscript-solid-lab` repository,
now pinned here as a Git submodule. It retains its independent history and its
own upstream Solid submodule while making the compiler integration, application
sources, compatibility cases, scripts, and checked evidence discoverable from
the main LilScript tree.

Clone everything with:

```sh
git clone --recurse-submodules https://github.com/yeargun/lilscript.git
```

For an existing clone:

```sh
git submodule update --init labs/solid-client
```

The main benchmark site does not require the submodule checkout. A current
portable size snapshot lives at
`benchmarks/popular/apps/solid/size-report.json`; the popular-library runner
uses that first, then the integrated lab report, and only then a legacy sibling
checkout. This prevents local absolute paths from becoming a hidden build
requirement.

Generated dependencies, `dist`, the lab's nested upstream Solid checkout, and
temporary artifacts remain owned and ignored by the lab rather than being
duplicated into the main repository.
