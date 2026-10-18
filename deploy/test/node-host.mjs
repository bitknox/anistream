// Serves the Vercel or Netlify template's real function over HTTP, the way each platform calls it,
// with an in-memory Postgres (PGlite) behind it. For journeys.sh; not part of any deployment.
//
//   ANISTREAM_STORE_SETUP_CODE=… node deploy/test/node-host.mjs vercel 7990

import http from "node:http";
import { Readable } from "node:stream";
import { PGlite } from "@electric-sql/pglite";
import { PGLiteSocketServer } from "@electric-sql/pglite-socket";

const [template, port] = process.argv.slice(2);
const deploy = new URL("..", import.meta.url);

// PGlite answers one connection at a time, and a second one would see the first's open
// transaction, so the template is held to a single connection.
const db = await PGlite.create();
const postgres = new PGLiteSocketServer({ db, port: Number(port) + 1000, host: "127.0.0.1" });
await postgres.start();
process.env.DATABASE_URL = `postgres://postgres@127.0.0.1:${Number(port) + 1000}/postgres?sslmode=disable`;
process.env.ANISTREAM_STORE_DB_CONNECTIONS = "1";

let call;
if (template === "vercel") {
  const { default: fn } = await import(new URL("vercel/api/index.js", deploy));
  // Vercel writes these itself, whatever the client sent.
  call = (request, ip) => {
    request.headers.set("x-real-ip", ip);
    request.headers.set("x-forwarded-for", ip);
    return fn.fetch(request);
  };
} else if (template === "netlify") {
  const { default: fn } = await import(new URL("netlify/netlify/functions/store.js", deploy));
  call = (request, ip) => fn(request, { ip, params: {}, site: {} });
} else {
  throw new Error("usage: node-host.mjs vercel|netlify <port>");
}

http
  .createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host}`);
    const headers = new Headers();
    for (let i = 0; i < req.rawHeaders.length; i += 2) {
      headers.append(req.rawHeaders[i], req.rawHeaders[i + 1]);
    }
    const body = req.method === "GET" || req.method === "HEAD" ? undefined : Readable.toWeb(req);
    try {
      const response = await call(
        new Request(url, { method: req.method, headers, body, duplex: "half" }),
        req.socket.remoteAddress ?? "",
      );
      res.statusCode = response.status;
      for (const [name, value] of response.headers) {
        if (name !== "set-cookie") res.setHeader(name, value);
      }
      const cookies = response.headers.getSetCookie();
      if (cookies.length) res.setHeader("set-cookie", cookies);
      res.end(Buffer.from(await response.arrayBuffer()));
    } catch (error) {
      // Not a status the store uses, so a failing journey says where the fault was.
      console.error(req.method, url.pathname, "the function threw", error);
      res.statusCode = 599;
      res.end();
    }
  })
  .listen(Number(port), "127.0.0.1", () => console.log(`${template} on http://127.0.0.1:${port}`));
