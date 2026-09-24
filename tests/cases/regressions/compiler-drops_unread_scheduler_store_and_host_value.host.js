// compiler.rs drops_unread_scheduler_store_and_host_value ran no host: the old route removed
// every reference to `host`. The program still declares the extern, so a correct build may
// read it; this prelude supplies a never-called binding so that read is not a ReferenceError.
{
  globalThis.host = function host(callback) {};
}
