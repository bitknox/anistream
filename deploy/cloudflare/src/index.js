// The anistream plugin store as a Cloudflare Worker.
//
// Every route and rule is the store's own Rust core, compiled to WebAssembly. This file only
// lends it D1 for its records and R2 for plugin files, and translates each request and response.

import { initSync, Store } from "../store/store.js";
import module from "../store/store_bg.wasm";

initSync({ module });

// One upload part and a little room: the most any request to the store carries.
const MAX_BODY_BYTES = 4 * 1024 * 1024 + 64 * 1024;

// Opening runs the schema statements, so it happens once per isolate rather than per request. A
// failed open is forgotten, and the next request tries again.
let opening = null;

function open(env) {
  opening ??= Store.open(d1(env.DB), r2(env.FILES), {
    name: env.ANISTREAM_STORE_NAME,
    url: env.ANISTREAM_STORE_URL,
    setupCode: env.ANISTREAM_STORE_SETUP_CODE,
    // The free plan allows a request 10 ms of CPU, and hashing a plugin of several megabytes
    // takes more than that, so the store records the SHA-256 the owner's app sent with the
    // upload. It no longer catches a wrong one itself, but members check every download against
    // it, so a wrong digest makes downloads fail rather than letting other bytes through.
    trustDigests: true,
  }).catch((error) => {
    opening = null;
    throw error;
  });
  return opening;
}

function d1(database) {
  const prepare = ({ sql, params }) => database.prepare(sql).bind(...params);
  return {
    dialect: "sqlite",
    async query(sql, params) {
      const rows = await prepare({ sql, params }).raw();
      return rows.map((row) => row.map(fromD1));
    },
    async batch(statements) {
      // D1 runs a batch as one transaction, which is the all-or-nothing the store relies on.
      const results = await database.batch(statements.map(prepare));
      return results.map((result) => result.meta?.changes ?? 0);
    },
  };
}

// D1 takes a Uint8Array as a BLOB but hands blobs back as plain arrays of numbers.
function fromD1(value) {
  if (Array.isArray(value)) return Uint8Array.from(value);
  if (value instanceof ArrayBuffer) return new Uint8Array(value);
  return value;
}

function r2(bucket) {
  return {
    async put(key, bytes) {
      await bucket.put(key, bytes);
    },
    async get(key) {
      const object = await bucket.get(key);
      return object ? new Uint8Array(await object.arrayBuffer()) : null;
    },
    // Downloads come in ranges, and R2 reads just the range: copying a whole plugin into the
    // store for each one would spend the CPU budget on bytes nobody asked for.
    async getRange(key, offset, length) {
      const object = await bucket.get(key, { range: { offset, length } });
      return object ? new Uint8Array(await object.arrayBuffer()) : null;
    },
    async delete(key) {
      await bucket.delete(key);
    },
  };
}

// The body, or null when it is over the limit. Read with a running count, because a chunked
// request has no length to check up front and buffering an unbounded one could exhaust the
// isolate's memory.
async function readBody(request) {
  if (!request.body) return new Uint8Array(0);
  if (Number(request.headers.get("content-length")) > MAX_BODY_BYTES) return null;
  const chunks = [];
  let size = 0;
  for await (const chunk of request.body) {
    size += chunk.byteLength;
    if (size > MAX_BODY_BYTES) return null;
    chunks.push(chunk);
  }
  const body = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    body.set(chunk, at);
    at += chunk.byteLength;
  }
  return body;
}

// The same shape and headers as the store's own errors, so the app reads these like any other.
function failure(status, code, message) {
  return new Response(JSON.stringify({ error: code, message }), {
    status,
    headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
}

// Statuses a Response may not be given a body for, even an empty one.
const NO_BODY = new Set([101, 204, 205, 304]);

export default {
  async fetch(request, env, ctx) {
    try {
      const pending = open(env);
      // Kept alive past this request, so a client that hangs up mid-open cannot leave the
      // shared promise unsettled for every request after it.
      ctx.waitUntil(pending.catch(() => {}));
      const body = await readBody(request);
      if (body === null) {
        return failure(413, "too_large", "the body is over the 4 MiB limit");
      }
      const url = new URL(request.url);
      const store = await pending;
      const out = await store.handle({
        method: request.method,
        path: url.pathname,
        headers: [...request.headers],
        body,
        client: request.headers.get("cf-connecting-ip") ?? "",
        base: env.ANISTREAM_STORE_URL || url.origin,
        now: Math.floor(Date.now() / 1000),
      });
      // Appended one by one: a response can carry more than one set-cookie.
      const headers = new Headers();
      for (const [name, value] of out.headers) headers.append(name, value);
      return new Response(NO_BODY.has(out.status) ? null : out.body, {
        status: out.status,
        headers,
      });
    } catch (error) {
      console.error("the store failed to answer", request.method, new URL(request.url).pathname, error);
      return failure(500, "internal", "the store hit an error");
    }
  },
};
