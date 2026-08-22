const DEFAULT_CHANNEL = "default";
const INITIALIZE = "$initialize";
const HOST_CHANNEL = "editorWorkerHost";

function getWorker(opts) {
  const env = globalThis.MonacoEnvironment;
  if (typeof env?.getWorker === "function") {
    return env.getWorker("", opts.label ?? "typescript");
  }
  if (typeof opts.createWorker === "function") {
    return opts.createWorker();
  }
  throw new Error("MonacoEnvironment.getWorker is required for the TypeScript worker");
}

function modelUrl(resource) {
  if (!resource) return "";
  if (typeof resource === "string") return resource;
  if (typeof resource.toString === "function") return String(resource.toString());
  return String(resource);
}

function linesOf(model) {
  if (typeof model.getLinesContent === "function") {
    const rows = model.getLinesContent();
    if (Array.isArray(rows) && rows.length) return rows.map((line) => String(line ?? ""));
  }
  return String(model.getValue?.() ?? "").split(/\r\n|\r|\n/);
}

class WorkerProtocol {
  constructor(post) {
    this._workerId = 1;
    this._lastReq = 0;
    this._pending = Object.create(null);
    this._channels = new Map();
    this._post = post;
  }

  setChannel(name, handler) {
    this._channels.set(name, handler);
  }

  send(channel, method, args) {
    const req = String(++this._lastReq);
    return new Promise((resolve, reject) => {
      this._pending[req] = { resolve, reject };
      this._post({
        vsWorker: this._workerId,
        req,
        channel,
        method,
        args,
        type: 0,
      });
    });
  }

  handle(message) {
    if (!message || !message.vsWorker) return;
    if (message.type === 1) {
      const pending = this._pending[message.seq];
      if (!pending) return;
      delete this._pending[message.seq];
      if (message.err) {
        pending.reject(message.err);
        return;
      }
      pending.resolve(message.res);
      return;
    }
    if (message.type === 0) {
      const handler = message.channel === DEFAULT_CHANNEL ? null : this._channels.get(message.channel);
      const fn = handler?.[message.method];
      Promise.resolve(typeof fn === "function" ? fn.apply(handler, message.args) : undefined)
        .then((res) => {
          this._post({
            vsWorker: this._workerId,
            seq: message.req,
            res,
            err: undefined,
            type: 1,
          });
        })
        .catch((err) => {
          this._post({
            vsWorker: this._workerId,
            seq: message.req,
            res: undefined,
            err: { $isError: true, name: err?.name, message: String(err?.message ?? err), stack: err?.stack },
            type: 1,
          });
        });
    }
  }

  remote() {
    const send = this.send.bind(this);
    return new Proxy(
      {},
      {
        get(_target, name) {
          if (name === "then") return undefined;
          if (typeof name !== "string") return undefined;
          if (name.charCodeAt(0) === 36) {
            return (...args) => send(DEFAULT_CHANNEL, name, args);
          }
          return undefined;
        },
      },
    );
  }
}

class MonacoWebWorker {
  constructor(workerPromise, host, keepIdleModels) {
    this._synced = new Map();
    this._keepIdleModels = Boolean(keepIdleModels);
    this._worker = null;
    this._protocol = null;
    this._proxy = null;
    this._ready = Promise.resolve(workerPromise).then((worker) => {
      this._worker = worker;
      this._protocol = new WorkerProtocol((msg, transfer) => worker.postMessage(msg, transfer ?? []));
      this._protocol.setChannel(HOST_CHANNEL, {
        $fhr: (method, args) => {
          if (!host || typeof host[method] !== "function") {
            return Promise.reject(new Error("Missing method " + method));
          }
          return host[method](...args);
        },
      });
      worker.addEventListener("message", (ev) => this._protocol.handle(ev.data));
      return this._protocol.send(DEFAULT_CHANNEL, INITIALIZE, [1]).then(() => this._protocol.remote());
    });
  }

  getProxy() {
    if (!this._proxy) {
      this._proxy = this._ready.then((remote) => {
        return new Proxy(
          {},
          {
            get(_target, name) {
              if (name === "then") return undefined;
              if (typeof name !== "string") return undefined;
              return (...args) => remote.$fmr(name, args);
            },
          },
        );
      });
    }
    return this._proxy;
  }

  async withSyncedResources(resources) {
    const proxy = await this._ready;
    const monaco = globalThis.monaco;
    for (const resource of resources ?? []) {
      const url = modelUrl(resource);
      const model = monaco?.editor?.getModel?.(resource) ?? monaco?.editor?.getModels?.()?.find((row) => modelUrl(row.uri) === url);
      if (!model) continue;
      if (!this._synced.has(url)) {
        await proxy.$acceptNewModel({
          url,
          lines: linesOf(model),
          EOL: model.getEOL?.() || "\n",
          versionId: model.getVersionId?.() ?? 1,
        });
        const unsub = model.onDidChangeContent?.(() => {
          Promise.resolve(proxy.$acceptRemovedModel(url)).then(() =>
            proxy.$acceptNewModel({
              url,
              lines: linesOf(model),
              EOL: model.getEOL?.() || "\n",
              versionId: model.getVersionId?.() ?? 1,
            }),
          );
        });
        this._synced.set(url, unsub);
      }
    }
    return this.getProxy();
  }

  dispose() {
    for (const unsub of this._synced.values()) {
      unsub?.dispose?.();
    }
    this._synced.clear();
    this._worker?.terminate?.();
  }
}

export function createWebWorker(opts) {
  const worker = Promise.resolve(getWorker(opts)).then((w) => {
    w.postMessage("ignore");
    w.postMessage(opts.createData ?? {});
    return w;
  });
  return new MonacoWebWorker(worker, opts.host, opts.keepIdleModels);
}
