// Cloudflare Worker for the static (serverless) deployment of SIMULACRE.
//
// The UI and the engine (WebAssembly) are plain assets in web/; the vault lives
// in each visitor's browser. The Worker only does what a browser cannot:
// fetch a portrait from thispersondoesnotexist.com, which sends no CORS
// headers. It relays that single fixed URL, nothing else, and stores nothing.

const FACE_SOURCE = "https://thispersondoesnotexist.com/random-person.jpeg";
const MAX_BYTES = 8 * 1024 * 1024;

const json = (status, body) =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", "cache-control": "no-store" } });

async function face() {
  let res;
  try {
    res = await fetch(FACE_SOURCE, {
      headers: { "user-agent": "simulacre (+OSINT persona workbench)", accept: "image/jpeg,image/*" },
      cf: { cacheTtl: 0, cacheEverything: false },
    });
  } catch (e) {
    return json(502, { error: `face source unreachable: ${e.message}` });
  }
  const type = res.headers.get("content-type") || "";
  if (!res.ok || !type.startsWith("image/")) return json(502, { error: `face source answered ${res.status}` });
  const body = await res.arrayBuffer();
  if (body.byteLength > MAX_BYTES) return json(502, { error: "face too large" });
  return new Response(body, { headers: { "content-type": type, "cache-control": "no-store", "x-content-type-options": "nosniff" } });
}

export default {
  async fetch(request, env) {
    const { pathname } = new URL(request.url);
    // Tells the UI there is no engine server here: use the in-browser one.
    if (pathname === "/api/health") return json(200, { ok: true, backend: "browser" });
    if (pathname === "/api/face") {
      return request.method === "GET" ? face() : json(405, { error: "method not allowed" });
    }
    // Every other /api route belongs to the Rust server, which does not run
    // here: answer 404 so the UI switches to its in-browser engine.
    if (pathname.startsWith("/api/")) return json(404, { error: "not found" });
    return env.ASSETS.fetch(request);
  },
};
