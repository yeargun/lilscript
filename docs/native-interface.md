# Native C interface

Select `[target.native] artifact="shared-library"` or `artifact="object"` to
retain the checked public interface. `--target c` writes C and its header;
`--target native` invokes the configured compiler and publishes the artifact
and header. An object has no `main`; link it with your C client and custom
providers. The default executable artifact retains its existing process flow.

```toml
[target.native]
artifact = "shared-library"
symbol_prefix = "mathlib"
objective = "balanced"
```

The generated header is the contract for that build. Public source names are
stable across optimizations: `mathlib_e_ENTRY__NAME`, with each non-alphanumeric
UTF-8 byte encoded `_hh` (including underscores). The double underscore
separates entry and export unambiguously. Names always include the configured
entry name, so separate roots may export the same name. Runtime names and
physical types become `mathlib_ls_...`; value tags become `mathlib_LS_...`.
Choose distinct prefixes for separate libraries. Header-local mappings are
removed after declarations so their headers can be included together.

Internal physical type numbers/layouts and callable signature numbers are not
a frozen binary ABI. Recompile C clients against the generated header after
changing source or compiler. Use source export aliases for client storage:
function `_argN` and `_result` aliases, `_binding_result` for a getter's result,
and exported nominal `_type` aliases. Every non-void alias has `_retain` and
`_release` helpers; callable aliases also have `_call`.

## Lifecycle and ownership

Call `mathlib_initialize(argc, argv)` once before source exports. It evaluates
the shared module initialization order exactly once. A successful repeated
call is a no-op. A failed initialization remains failed and replays its saved
exception; it never reruns side effects. `mathlib_drain()` runs the owned FIFO
task queue and reports an unhandled rejection through exception status.
Exported calls do not implicitly drain. A provider may synchronously reenter
an export during initialization; ordinary initialization guards still apply.

`mathlib_shutdown()` discards queued work, releases module owners and collects
unreachable cycles. It does not execute abandoned generator finalizers. Shutdown
is idempotent, terminal, and refused while a source call, initialization or
queue drain is active. Release host-owned values/callbacks before shutdown;
shutdown cannot reclaim a live owner still held by the host. Calls, retained
owners, queue operations and shutdown all stay on the originating thread.
There is no foreign unwinding, concurrent invocation or implicit event loop.

Arguments borrow; function results and live binding getters transfer one owner.
Retaining acquires another owner; releasing gives it up. This includes strings,
products, arrays, objects, callable environments and tagged values. A string
view owns its backing UTF-16 allocation; literal storage has a null owner.
Copy temporary host text with the generated `..._ls_string_from_utf16` API.
Pointers must be live handles from the same library. Type checks distinguish
runtime allocation kinds; they cannot validate arbitrary fabricated addresses.

After a call, check `..._ls_native_exception_pending()` before using its result.
`..._ls_native_exception_take()` clears status and transfers an owned tagged
exception. Providers can raise through `..._ls_native_exception_raise(value)`;
the caller retains ownership of the argument. No `longjmp` crosses source
cleanup. Clear pending status before another ordinary call.

## Source surface

A value export supplies `_get()` and preserves the live source binding.
Callable exports additionally supply a direct typed function with the source
name. A getter of a reassigned function returns the current function; an
earlier retained callback keeps its original identity/environment. There is no
setter for the module binding itself.

Optional arguments use callback ABI 3's `ls_native_arguments`: a supplied count
and an optional borrowed absence bitmap. Pass a placeholder for each physical
parameter, including omitted ones; its value is ignored when absent. A null
bitmap means all supplied arguments are present. Metadata is borrowed only for
the call. Synchronous `ref` arguments follow the shared source contract; the
language continues to reject public opaque reference interfaces and captured
or suspended reference parameters.

An exported constructor supplies `_new`, omitting the internal receiver.
Its count/bitmap describe only explicit parameters. A published class type
supplies `_field_NAME_get/set` and `_method_NAME`; method parameters start with
the receiver and their optional metadata includes it. A method wrapper selects
the actual class's prototype override. Objects remain managed opaque handles.
Struct fields have owned getters and copying setters on the C value layout.
Exported enum variants have `_variant_NAME()` functions. Shapes use managed
record handles and the record API, preserving their declared source schema.

An exported struct's `_box` and `_unbox` bridge its value layout and an owned
tagged product for generic calls. A C callback can use a managed record as its
environment; acquire a fresh identity with `..._ls_native_new_identity()` after
initialization, and preserve it when copying that callback. A null environment
is valid for a stateless callback. Do not put unmanaged host pointers in the
environment field: generated calls retain it as an ordinary managed owner.

The header exposes owned record operations, and array/task/generator operations
when those recipes are needed. Generic tagged arrays can pass through typed
array views with the same checked conversions used by source operations.
A host task can be resolved/rejected once; draining delivers continuations.
Reading a settled task's result marks rejection handled and returns its owned
payload. A generator's explicit close executes pending `finally` blocks;
merely releasing an unreachable generator does not run user code.

## Explicit standard providers

Bundled providers are selected only through `[host.native]`. No source name is
automatically granted access. The `host_lil_` namespace is reserved for the
following exact signatures; unknown names and mismatches are rejected before
C formation. Custom providers use other `host_` names and `native_sources`.

| C provider | Source declaration shape | Behavior |
|---|---|---|
| `host_lil_arg_count` | `extern int count();` | Number of copied arguments, including argv[0] when supplied. |
| `host_lil_arg` | `extern string? arg(int index);` | Owned argument or absence outside the supplied range. |
| `host_lil_env` | `extern string? env(string name);` | Reads the process environment at each call; absence for a missing key. |
| `host_lil_stdout` | `extern void output(string text);` | Writes and flushes stdout without adding a newline. |
| `host_lil_stderr` | `extern void errors(string text);` | Writes and flushes stderr without adding a newline. |
| `host_lil_stdin` | `extern string input();` | Reads UTF-8 stdin through EOF without closing it. Whole-stream buffering uses memory proportional to input. |
| `host_lil_read_text` | `extern string read(string path);` | Reads a whole UTF-8 file; accepts non-seekable input. |
| `host_lil_write_text` | `extern void write(string path, string text);` | Validates text, then truncates/writes/closes the destination. |
| `host_lil_wall_time` | `extern float wall();` | C11 UTC wall-clock seconds, including fractions; not monotonic. |
| `host_lil_cpu_time` | `extern float cpu();` | Process CPU seconds from `clock`; availability/resolution is platform dependent. |
| `host_lil_exit` | `extern void quit(int status);` | Flushes streams and terminates the process with `_Exit`; does not execute source `finally` or library shutdown. |

Host text boundaries use strict UTF-8. Malformed byte input or unpaired UTF-16
surrogates raise `EncodingError`; paths/environment keys containing NUL raise
`TypeError`. File/stream/clock failures raise `IOError`. File text may contain
NUL. These explicit provider contracts are separate from JavaScript runtime
facilities; cross-target programs supply corresponding JavaScript wrappers.
Whole-file reads consume memory proportional to input, and writes are ordinary
file operations, not transactional replacements. Only selected provider recipes
are emitted; argv storage is copied only when an argument provider is selected.
