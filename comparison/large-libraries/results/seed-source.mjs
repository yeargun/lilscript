const recordedAt = "2026-08-24T00:00:00.000Z";

const pins = {
  solidlil: {
    revision: "81a1f08bb0eaa24c81700c105d4055127460cdb6",
    tree: "506d6e01b11caafa231b109c4466bd538bfa1898",
    packageLockSha256: "107ad056b4b0f4de7806f8b1628a773cf72ac5fe0824307963546ae9c8aef0eb",
    entrySha256: "a836c4e2150fb699c51e16e98b6522f97037f3c28af5df66a67f9a0f169cc7b7",
    configSha256: "dd32efa99d7316e0471bfcd98dad69138f30c122522bbc3c4e3165d5ccd96ddf",
  },
  markedlil: {
    revision: "3a540ac8d6961aaf8eb060b5eae5368939b8981c",
    tree: "ea0041325559c67e5ebcd0115ec729d302670304",
    packageLockSha256: "89432f687ae5dd6c4c29af04da368bc149fa4ddf2c16975fb9f80443c1b399f3",
    entrySha256: "b1f71341712881346d2271eeb8e9ad915529b72699a8c9a798edffec6d073ca5",
    configSha256: "21f9665c6dc35c5a8c6bd663ceaf00b5a6ae2f5a15740f03df3b68cfd778537a",
  },
  mobxlil: {
    revision: "ef1d18487d2b153e1fa9fab7a90dd1518a0cda0b",
    tree: "cb4217662c9b262092b8ffc0fbdf41ecfb5f348f",
    packageLockSha256: "d327ae7b0093237874f64263ee3fe922ad4695cf00e27a1761a951a55af85667",
    entrySha256: "daffae3ebb3585f3f6751c8d38b715b9f311062f76e63fbc7530a27789453e2a",
    configSha256: "15c1dec745765dc87eb9fca0cbd3423f917091dd21d2ef0caf305dc332b51cbe",
  },
  jquerylil: {
    revision: "b860fe8c9d9799e5e6fbfc2204d7bdd00948d5a1",
    tree: "791d4eb6da3899c65cd289c2d31f578df661b1e4",
    packageLockSha256: "694b0e61cee1560d7ebccb649e807bfd7965af8436fae8902b0d5d9c2fa13856",
    entrySha256: "f986f027261729561669b6b0b49368f6d99ef1e8f1cf978efe97ef28c6e5e2bd",
    configSha256: "9f4a0a5a043b1ecf20f80b8e1a6656ace8efb9cbe5cc06aee15b1deba638395d",
  },
};

const compilers = {
  baseline: {
    role: "baseline",
    revision: "5245f1790a9ee3d29e54fe72282da700dcc045d2",
    tree: "7ee0b9d7e32b147fb48d59a77f13aedd8ff10376",
    binarySha256: "86b2445bc91030fe0a80a9b2be6033f9e7635106560317b2ff50612d3e5815bc",
    primarySourceSha256: "fa0f20176494dbd3807b218ae204108321170649f7b2b1ef8e2c6ec22086b565",
    sourceIdentity: "exact git archive 5245f1790a9ee3d29e54fe72282da700dcc045d2",
  },
  checkpoint: {
    role: "checkpoint",
    revision: "979dc90d5c10fddb1328ea3f707cd17d3869a3fe",
    tree: "20a45a48a99955f705199d2fddf7dadf0edd20e2",
    binarySha256: "d5e2abee2d3c3ca82a69e262c8dd819c440933f570e62eae4744db2eb021284c",
    primarySourceSha256: "607ac880caa60b57011ac2ee0639f4b01c50cde543fffa9d47e710e7284e5684",
    sourceIdentity: "exact git archive 979dc90d5c10fddb1328ea3f707cd17d3869a3fe",
  },
  published: {
    role: "published-unknown",
    revision: null,
    tree: null,
    binarySha256: null,
    primarySourceSha256: null,
    sourceIdentity: null,
  },
};

function source(library, override = {}) {
  return {
    ...pins[library],
    configDerivation: null,
    ...override,
  };
}

function semantic(status, evidenceClass, summary, command = null) {
  return { status, evidenceClass, command, summary };
}

const notRun = (summary = "no artifact was available for semantic testing") =>
  semantic("not-run", "none", summary);

function timing({
  scope = "compiler command",
  wallMs = null,
  userCpuMs = null,
  systemCpuMs = null,
  contention = "unknown",
  unavailableReason = null,
} = {}) {
  return {
    scope,
    wallMs,
    userCpuMs,
    systemCpuMs,
    contention,
    diagnosticOnly: true,
    unavailableReason,
  };
}

function artifact({
  id,
  objective = null,
  gateMetrics = ["raw", "gzip9", "brotli11"],
  relativePath,
  configSha256,
  sha256,
  sizes,
  derivation = { kind: "identity", from: null },
  artifactSemantic,
  role = "gate",
}) {
  return {
    id,
    role,
    objective,
    gateMetrics,
    relativePath,
    configSha256,
    sha256,
    sizes,
    derivation,
    semantic: artifactSemantic,
  };
}

function observation({
  id,
  library,
  purpose,
  evidenceClass,
  compiler,
  status,
  artifacts = [],
  aggregateSemantic = notRun(),
  observationTiming = timing(),
  failure = null,
  notes = [],
  sourceOverride = {},
}) {
  return {
    id,
    library,
    purpose,
    evidenceClass,
    recordedAt,
    compiler: { ...compiler },
    source: source(library, sourceOverride),
    status,
    artifacts,
    semantic: aggregateSemantic,
    timing: observationTiming,
    failure,
    notes,
  };
}

function failed({ id, library, compiler, status, timing: failureTiming, failure, notes = [], purpose = "comparison", sourceOverride = {} }) {
  return observation({
    id,
    library,
    purpose,
    evidenceClass: "fresh",
    compiler,
    status,
    observationTiming: failureTiming,
    failure,
    notes,
    sourceOverride,
  });
}

const identity = { kind: "identity", from: null };
const markedBanner = {
  kind: "prepend-first-line",
  from: "dist/marked.esm.js",
};
const freshPass = (summary, command = null) =>
  semantic("passed", "fresh", summary, command);

export function seedSource(matrixSha256) {
  const observations = [
    observation({
      id: "published.solidlil",
      library: "solidlil",
      purpose: "published",
      evidenceClass: "published",
      compiler: compilers.published,
      status: "passed",
      artifacts: [
        artifact({
          id: "package",
          objective: "brotli11",
          gateMetrics: ["brotli11"],
          relativePath: "dist/core.js",
          configSha256: pins.solidlil.configSha256,
          sha256: "43d16ab858adf61bbd7cfb2043ab533430636c02a083fe27bea42558b2322371",
          sizes: { raw: 8179, gzip9: 3250, brotli11: 2909 },
          derivation: identity,
          artifactSemantic: semantic(
            "not-run",
            "none",
            "the published artifact was size-audited but not used as fresh semantic evidence",
          ),
        }),
      ],
      aggregateSemantic: notRun("published artifact semantics were not freshly gated"),
      observationTiming: timing({
        scope: "compile",
        unavailableReason: "published artifact has no reproducible compiler timing",
      }),
      notes: ["Canonical sizes were remeasured with the exact checkpoint codec."],
    }),
    observation({
      id: "published.markedlil",
      library: "markedlil",
      purpose: "published",
      evidenceClass: "published",
      compiler: compilers.published,
      status: "passed",
      artifacts: [
        artifact({
          id: "raw-objective",
          objective: "raw",
          gateMetrics: ["raw"],
          relativePath: "dist/marked.bytes.js (banner from dist/marked.esm.js)",
          configSha256: "2a3b9c59ab21829bb6aeac4974f171629e75336292abcaa2e7e09e37d67bb412",
          sha256: "df19a949039bb94f42dfdc7ee758d6f2eadb0f9cfb6856a9e4261ddf9adc24cc",
          sizes: { raw: 33632, gzip9: 10605, brotli11: 9504 },
          derivation: markedBanner,
          artifactSemantic: semantic(
            "failed",
            "fresh",
            "independent lane audit found 19 checks mismatching, including GFM 630-632 and CommonMark 604-605",
            "node comparison/large-libraries/semantic/marked-lane.mjs --root ../markedlil --artifact ../markedlil/dist/marked.bytes.js",
          ),
        }),
        artifact({
          id: "gzip-objective",
          objective: "gzip9",
          gateMetrics: ["gzip9"],
          relativePath: "dist/marked.gzip.js (banner from dist/marked.esm.js)",
          configSha256: "7edbbb56eb269296e0e907857141b4f59afe86f49634fc041383e4a23613bb74",
          sha256: "0a99a3fb937e8f5ff582b09ed13d4c039a34b2a875b8dacb0d204cc6183ae029",
          sizes: { raw: 36304, gzip9: 10727, brotli11: 9603 },
          derivation: markedBanner,
          artifactSemantic: freshPass(
            "660 corpus cases passed across 2,640 parse and 660 parseInline checks",
            "node comparison/large-libraries/semantic/marked-lane.mjs --root ../markedlil --artifact ../markedlil/dist/marked.gzip.js",
          ),
        }),
        artifact({
          id: "brotli-objective-shipped-esm",
          objective: "brotli11",
          gateMetrics: ["brotli11"],
          relativePath: "dist/marked.esm.js",
          configSha256: pins.markedlil.configSha256,
          sha256: "ccc36b473a78f08ff4af8bac05b8bd044291a9aa8d6c446c73409e817fb37109",
          sizes: { raw: 35985, gzip9: 10766, brotli11: 9589 },
          derivation: identity,
          artifactSemantic: freshPass(
            "29 Node test blocks passed, including all 660 loaded spec cases",
            "node --test test/compat.test.mjs test/options.test.mjs test/official-parse.test.mjs test/api.test.mjs test/closed.test.mjs",
          ),
        }),
      ],
      aggregateSemantic: semantic(
        "partial",
        "fresh",
        "the three objective artifacts have independent and different semantic evidence",
      ),
      observationTiming: timing({
        scope: "compile",
        unavailableReason: "published artifacts have no reproducible compiler timing",
      }),
      notes: [
        "Raw, gzip, and Brotli sizes come from three different compiler artifacts.",
        "Canonical sizes were remeasured with the exact checkpoint codec.",
      ],
    }),
    observation({
      id: "published.mobxlil",
      library: "mobxlil",
      purpose: "published",
      evidenceClass: "published",
      compiler: compilers.published,
      status: "passed",
      artifacts: [
        artifact({
          id: "production-esm",
          objective: "brotli11",
          gateMetrics: ["brotli11"],
          relativePath: "dist/mobx.esm.js",
          configSha256: pins.mobxlil.configSha256,
          sha256: "d953a70c359ff3c13ab6439916155d82eb89680c8c6ba74cb04992473d1c3c96",
          sizes: { raw: 65664, gzip9: 18690, brotli11: 16736 },
          derivation: identity,
          artifactSemantic: semantic(
            "reported-passed",
            "published",
            "project report: 78 exports; 769 passed, 0 failed, and 11 skipped",
          ),
        }),
      ],
      aggregateSemantic: semantic(
        "reported-passed",
        "published",
        "upstream project report only; not eligible as a fresh comparison gate",
      ),
      observationTiming: timing({
        scope: "compile",
        unavailableReason: "published artifact has no reproducible compiler timing",
      }),
      notes: ["Canonical sizes were remeasured with the exact checkpoint codec."],
    }),
    observation({
      id: "published.jquerylil",
      library: "jquerylil",
      purpose: "published",
      evidenceClass: "published",
      compiler: compilers.published,
      status: "passed",
      artifacts: [
        artifact({
          id: "shipped-esm",
          objective: "brotli11",
          gateMetrics: ["brotli11"],
          relativePath: "dist/jquery.esm.js",
          configSha256: pins.jquerylil.configSha256,
          sha256: "865b0cbf9a52a692390bc5fa1bd4cee153d51256b911f6458199e71ef15c4d21",
          sizes: { raw: 92765, gzip9: 34544, brotli11: 30973 },
          derivation: identity,
          artifactSemantic: semantic(
            "reported-passed",
            "published",
            "project report: 6 of 6 compatibility tests passed",
          ),
        }),
      ],
      aggregateSemantic: semantic(
        "reported-passed",
        "published",
        "project semantic report only; not eligible as a fresh comparison gate",
      ),
      observationTiming: timing({
        scope: "compile",
        unavailableReason: "published artifact has no reproducible compiler timing",
      }),
      notes: ["Canonical sizes were remeasured with the exact checkpoint codec."],
    }),
    observation({
      id: "fresh.solidlil.baseline",
      library: "solidlil",
      purpose: "comparison",
      evidenceClass: "fresh",
      compiler: compilers.baseline,
      status: "passed",
      artifacts: [
        artifact({
          id: "package",
          objective: "brotli11",
          gateMetrics: ["brotli11"],
          relativePath: "dist/core.js",
          configSha256: pins.solidlil.configSha256,
          sha256: "6a4f146db92fd6552ed94d42d87d9203bcaec11bfa1d2cd772d130d49f7ca823",
          sizes: { raw: 8131, gzip9: 3263, brotli11: 2922 },
          derivation: identity,
          artifactSemantic: freshPass("57 exports and queue/flush behavior passed"),
        }),
        artifact({
          id: "compiler-direct",
          role: "diagnostic",
          gateMetrics: [],
          relativePath: "src/.__compiled-core.mjs",
          configSha256: pins.solidlil.configSha256,
          sha256: "2c4543b67306e8621ebadee47f28b076370b95d4f66859a734c1c6767849852f",
          sizes: { raw: 9087, gzip9: 3249, brotli11: 2870 },
          derivation: identity,
          artifactSemantic: freshPass("57 exports, queue/flush, and selector behavior passed"),
        }),
      ],
      aggregateSemantic: freshPass("packaged and direct compiler artifacts passed their fresh gates"),
      observationTiming: timing({
        scope: "compiler command",
        contention: "contended",
        unavailableReason: "only an approximate pause-inflated wall time of about 16 minutes was retained",
      }),
      notes: [
        "Package artifact used the pinned build.mjs esbuild plus Terser path.",
        "Timing is deliberately null rather than turning an approximate contended duration into an exact number.",
      ],
    }),
    observation({
      id: "fresh.solidlil.checkpoint",
      library: "solidlil",
      purpose: "comparison",
      evidenceClass: "fresh",
      compiler: compilers.checkpoint,
      status: "passed",
      artifacts: [
        artifact({
          id: "package",
          objective: "brotli11",
          gateMetrics: ["brotli11"],
          relativePath: "dist/core.js",
          configSha256: pins.solidlil.configSha256,
          sha256: "395656a56981cc88c4b79a436fff8631b2d8a81acf2d540377597f08f7ba4d52",
          sizes: { raw: 8225, gzip9: 3263, brotli11: 2931 },
          derivation: identity,
          artifactSemantic: freshPass("57 exports and queue/flush behavior passed"),
        }),
        artifact({
          id: "compiler-direct",
          role: "diagnostic",
          gateMetrics: [],
          relativePath: "core.compiler.mjs",
          configSha256: pins.solidlil.configSha256,
          sha256: "67cecf6a79bae1f8b100e4d1016a844938d549c0968f28de5301e5d59697ef6e",
          sizes: { raw: 9326, gzip9: 3277, brotli11: 2901 },
          derivation: identity,
          artifactSemantic: freshPass("57 exports, queue/flush, and selector behavior passed"),
        }),
      ],
      aggregateSemantic: freshPass("packaged and direct compiler artifacts passed their fresh gates"),
      observationTiming: timing({
        scope: "direct compiler command",
        wallMs: 96650,
        userCpuMs: 356980,
        systemCpuMs: 3040,
      }),
      notes: [
        "The d5e2abee binary was independently rebuilt byte-identically from exact commit 979dc90.",
        "The package artifact came from the same pinned source and byte-identical compiler executable.",
        "Compiler-reported optimization time was 96.638250 seconds with 69 candidates, precise=false, and peephole=0.",
      ],
    }),
    failed({
      id: "fresh.markedlil.baseline",
      library: "markedlil",
      compiler: compilers.baseline,
      status: "aborted",
      timing: timing({
        wallMs: 696270,
        userCpuMs: 695530,
        systemCpuMs: 6460,
        contention: "contended",
      }),
      failure: {
        phase: "compile",
        kind: "aborted",
        diagnostic: "operator stopped the production compile after the audit timebox; no output was written",
        artifactEmitted: false,
      },
    }),
    failed({
      id: "fresh.markedlil.checkpoint",
      library: "markedlil",
      compiler: compilers.checkpoint,
      status: "compile-error",
      timing: timing({ wallMs: 2000, userCpuMs: 3520, systemCpuMs: 40 }),
      failure: {
        phase: "compile",
        kind: "compile-error",
        diagnostic: "function 48 ($m5$backpedalUrl, function) has no emitted name at src/str.lil:397; no output was written",
        artifactEmitted: false,
      },
      notes: ["The exact checkpoint binary is byte-identical to the captured compiler that produced this failure."],
    }),
    failed({
      id: "fresh.mobxlil.baseline",
      library: "mobxlil",
      compiler: compilers.baseline,
      status: "timeout",
      timing: timing({
        wallMs: 1250610,
        contention: "contended",
        unavailableReason: "CPU split was not retained",
      }),
      failure: {
        phase: "compile",
        kind: "timeout",
        diagnostic: "production ESM compile hit the 20-minute audit cap and wrote no output",
        artifactEmitted: false,
      },
      notes: ["DEV=false was pinned for the production build."],
    }),
    failed({
      id: "fresh.mobxlil.checkpoint",
      library: "mobxlil",
      compiler: compilers.checkpoint,
      status: "timeout",
      timing: timing({
        wallMs: 1253780,
        contention: "contended",
        unavailableReason: "CPU split was not retained",
      }),
      failure: {
        phase: "compile",
        kind: "timeout",
        diagnostic: "production ESM compile hit the 20-minute audit cap and wrote no output",
        artifactEmitted: false,
      },
      notes: ["DEV=false was pinned for the production build."],
    }),
    failed({
      id: "fresh.jquerylil.baseline",
      library: "jquerylil",
      compiler: compilers.baseline,
      status: "timeout",
      timing: timing({
        wallMs: 1817930,
        userCpuMs: 5667890,
        systemCpuMs: 178070,
        contention: "contended",
      }),
      failure: {
        phase: "compile",
        kind: "timeout",
        diagnostic: "shipped ESM compile exceeded the 30-minute audit cap and wrote no replacement output",
        artifactEmitted: false,
      },
    }),
    failed({
      id: "fresh.jquerylil.checkpoint",
      library: "jquerylil",
      compiler: compilers.checkpoint,
      status: "timeout",
      timing: timing({
        wallMs: 1810480,
        userCpuMs: 6185870,
        systemCpuMs: 179330,
        contention: "contended",
      }),
      failure: {
        phase: "compile",
        kind: "timeout",
        diagnostic: "shipped ESM compile exceeded the 30-minute audit cap and wrote no replacement output",
        artifactEmitted: false,
      },
    }),
    failed({
      id: "diagnostic.markedlil.baseline-level0",
      library: "markedlil",
      compiler: compilers.baseline,
      purpose: "diagnostic",
      status: "timeout",
      timing: timing({
        wallMs: 314000,
        unavailableReason: "CPU split was not retained; process stayed near one full core",
      }),
      failure: {
        phase: "compile",
        kind: "timeout",
        diagnostic: "manual 5m14s cap at optimization_level=0; no output was written",
        artifactEmitted: false,
      },
      sourceOverride: {
        configSha256: "a39d0332da890849c93d89b45e93cbc81b04dad9ea528a1cfdf2161dc6182c4c",
        configDerivation: "lilscript.toml with only optimization_level = 15 changed to 0",
      },
      notes: ["This diagnostic is not eligible for the production comparison."],
    }),
    failed({
      id: "diagnostic.markedlil.checkpoint-level0",
      library: "markedlil",
      compiler: compilers.checkpoint,
      purpose: "diagnostic",
      status: "compile-error",
      timing: timing({
        wallMs: 110,
        unavailableReason: "CPU split was not retained",
      }),
      failure: {
        phase: "compile",
        kind: "crash",
        diagnostic: "optimization_level=0 reached the same base-emitter failure in 0.11 seconds; no output was written",
        artifactEmitted: false,
      },
      sourceOverride: {
        configSha256: "a39d0332da890849c93d89b45e93cbc81b04dad9ea528a1cfdf2161dc6182c4c",
        configDerivation: "lilscript.toml with only optimization_level = 15 changed to 0",
      },
      notes: ["This diagnostic is not eligible for the production comparison."],
    }),
  ];

  return {
    schemaVersion: 1,
    format: "lilscript-large-library-observations",
    matrixSha256,
    regressionPolicy: {
      semanticStatusRequired: "passed",
      maxRegressionBytes: { raw: 0, gzip9: 0, brotli11: 0 },
    },
    codec: {
      binarySha256: "b3d93da88e4516f3b22e5e67b288bb2504868f1c438010e0ac9492ec33de2063",
      sourceSha256: "3b56e56e4118f24f801c196418c83df526ebf771407502c57550d60910805729",
      builtFromRevision: "979dc90d5c10fddb1328ea3f707cd17d3869a3fe",
      schemaVersion: 1,
      gzip9: {
        encoder: "upstream-stock-zlib-c",
        libraryVersion: "1.3.1",
        cargoPackage: "libz-sys",
        cargoPackageVersion: "1.1.24",
        level: 9,
        mtime: 0,
      },
      brotli11: {
        encoder: "official-google-brotli-c",
        libraryVersion: "1.1.0",
        cargoPackage: "compu-brotli-sys",
        cargoPackageVersion: "1.1.0",
        quality: 11,
        lgwin: 22,
        mode: "generic",
      },
    },
    observations,
    comparisons: [],
    evidenceFingerprint: "0".repeat(64),
  };
}
