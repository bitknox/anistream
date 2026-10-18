// The anistream plugin store as a Vercel function. vercel.json sends every path here, and the
// request keeps the path it was sent to, so the store routes it as usual.

import { attachDatabasePool, ipAddress } from "@vercel/functions";
import { storeHandler } from "../lib/handler.js";

const answer = storeHandler({
  wasm: new URL("../store/store_bg.wasm", import.meta.url),
  // Neon, added from the Vercel Marketplace, sets both. DATABASE_URL is its pooled connection.
  databaseUrl: () => process.env.DATABASE_URL || process.env.POSTGRES_URL,
  // One instance answers several requests at once, so it keeps a few connections.
  connections: 3,
  // Keeps the instance awake until idle connections are closed, so none is left open while it
  // is suspended.
  onPool: attachDatabasePool,
});

export default {
  fetch(request) {
    // Vercel sets x-real-ip itself, so a client cannot choose what it says.
    return answer(request, ipAddress(request));
  },
};
