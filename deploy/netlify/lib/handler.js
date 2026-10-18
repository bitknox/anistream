// The anistream plugin store as a Node.js function, kept entirely in Postgres.
//
// Every route and rule is the store's own Rust core, compiled to WebAssembly. This file lends it a
// Postgres database for its records and, since these platforms have no disk, for plugin files
// too, and translates each request and response. Vercel and Netlify run the same file. Its one
// source is deploy/vercel/lib/handler.js, which deploy/sync.sh copies into the Netlify template.

import { readFileSync } from "node:fs";
import pg from "pg";
import { initSync, Store } from "../store/store.js";

// pg hands back BIGINT, which COUNT(*) also answers with, as a string by default so no precision
// is lost. Every integer a store keeps fits a double exactly, and the core wants numbers.
pg.types.setTypeParser(pg.types.builtins.INT8, Number);

// One upload part and a little room: the most any request to the store carries.
const MAX_BODY_BYTES = 4 * 1024 * 1024 + 64 * 1024;

// Statuses a Response may not be given a body for, even an empty one.
const NO_BODY = new Set([101, 204, 205, 304]);

/**
 * A request handler for one platform.
 *
 * `wasm` is where the store's WebAssembly is. The entry point gives it, relative to itself, because
 * a platform's bundler may fold this file into the entry point, and then a path relative to this
 * file would point somewhere else. `databaseUrl` reads the connection string from wherever the
 * platform puts it. `connections` is how many connections one instance may hold,
 * `ANISTREAM_STORE_DB_CONNECTIONS` overriding it for a database that allows fewer. `onPool` sees
 * the pool once it exists. Answers `(request, client) => Response`, where `client` is the caller's
 * address as the platform reports it.
 */
export function storeHandler({ wasm, databaseUrl, connections, onPool }) {
  // Opening runs the schema statements, so it happens once per instance rather than per request.
  // A failed open is forgotten, and the next request tries again.
  let opening = null;

  function open() {
    if (!opening) {
      opening = start();
      opening.catch(() => {
        opening = null;
      });
    }
    return opening;
  }

  async function start() {
    const url = databaseUrl();
    if (!url) {
      throw new Error("no database: set DATABASE_URL to a Postgres connection string");
    }
    initSync({ module: readFileSync(wasm) });
    const pool = new pg.Pool({
      connectionString: url,
      max: Number(process.env.ANISTREAM_STORE_DB_CONNECTIONS) || connections,
      // A function can be frozen between requests, and a connection left open across that only
      // ends up cut by the database. Letting idle ones go soon costs a reconnect now and then.
      idleTimeoutMillis: 5_000,
      // Long enough for a hosted database that sleeps when unused to wake up.
      connectionTimeoutMillis: 15_000,
    });
    // Without a listener, a database closing an idle connection would take the process with it.
    pool.on("error", (error) => console.error("an idle database connection failed", error));
    onPool?.(pool);
    try {
      return await Store.open(postgres(pool), null, {
        name: process.env.ANISTREAM_STORE_NAME,
        url: process.env.ANISTREAM_STORE_URL,
        setupCode: process.env.ANISTREAM_STORE_SETUP_CODE,
      });
    } catch (error) {
      await pool.end().catch(() => {});
      throw error;
    }
  }

  return async function answer(request, client) {
    const url = new URL(request.url);
    try {
      const pending = open();
      const body = await readBody(request);
      if (body === null) {
        return failure(413, "too_large", "the body is over the 4 MiB limit");
      }
      const store = await pending;
      const out = await store.handle({
        method: request.method,
        path: url.pathname,
        headers: [...request.headers],
        body,
        client: client ?? "",
        base: process.env.ANISTREAM_STORE_URL || url.origin,
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
      console.error("the store failed to answer", request.method, url.pathname, error);
      return failure(500, "internal", "the store hit an error");
    }
  };
}

/** The database as the store asks for it: Postgres, through a pool of `pg` connections. */
function postgres(pool) {
  return {
    dialect: "postgres",
    async query(sql, params) {
      const result = await pool.query({ text: sql, values: values(params), rowMode: "array" });
      return result.rows;
    },
    // All or nothing, on one connection, because a transaction belongs to the connection it began on.
    async batch(statements) {
      const connection = await pool.connect();
      try {
        await connection.query("BEGIN");
        const counts = [];
        for (const { sql, params } of statements) {
          const result = await connection.query({ text: sql, values: values(params) });
          counts.push(result.rowCount ?? 0);
        }
        await connection.query("COMMIT");
        return counts;
      } catch (error) {
        await connection.query("ROLLBACK").catch(() => {});
        throw error;
      } finally {
        connection.release();
      }
    },
  };
}

// Without values, pg sends a statement with the simple query protocol: one round trip, nothing
// prepared. Bytes go over as Buffers, viewing the same memory rather than copying it.
function values(params) {
  if (!params?.length) return undefined;
  return params.map((value) =>
    value instanceof Uint8Array && !Buffer.isBuffer(value)
      ? Buffer.from(value.buffer, value.byteOffset, value.byteLength)
      : value,
  );
}

// The body, or null when it is over the limit. Read with a running count, because a chunked
// request has no length to check up front and buffering an unbounded one could exhaust memory.
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
