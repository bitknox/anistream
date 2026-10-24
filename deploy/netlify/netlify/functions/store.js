// The anistream plugin store as a Netlify function, answering every path of the site.

import { storeHandler } from "../../lib/handler.js";

const answer = storeHandler({
  host: "netlify",
  // Relative to this file, which keeps its place when Netlify bundles the function.
  wasm: new URL("../../store/store_bg.wasm", import.meta.url),
  // DATABASE_URL first, so a database of your own wins. NETLIFY_DB_URL is the Netlify Database
  // that a deploy creates because package.json lists @netlify/database, and NETLIFY_DATABASE_URL
  // is the older Neon extension's.
  databaseUrl: () =>
    process.env.DATABASE_URL ||
    process.env.NETLIFY_DB_URL ||
    process.env.NETLIFY_DATABASE_URL ||
    process.env.POSTGRES_URL,
  // An instance answers one request at a time, so one connection is all it can use.
  connections: 1,
});

export default (request, context) => answer(request, context.ip);

// Every path, "/" included. A function given a path runs ahead of any static file there.
export const config = { path: "/*" };
