// Offline stand-in for thispersondoesnotexist.com: serves a different small
// PNG on every request. Uses only node:zlib to build valid PNGs.
import http from "node:http";
import zlib from "node:zlib";
import { pathToFileURL } from "node:url";

const CRC = new Int32Array(256).map((_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c;
});
const crc32 = (buf) => {
  let c = -1;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
};

export function png(size, seed) {
  const raw = Buffer.alloc((size * 3 + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (size * 3 + 1)] = 0;
    for (let x = 0; x < size; x++) {
      const o = y * (size * 3 + 1) + 1 + x * 3;
      const d = Math.hypot(x - size / 2, y - size / 2.4) < size / 4.5;
      raw[o] = d ? 210 : (seed * 53) % 255;
      raw[o + 1] = d ? 170 : (x + seed * 31) % 255;
      raw[o + 2] = d ? 150 : (y + seed * 17) % 255;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

// Run as a script: serve faces. Imported (static-server.mjs): only png().
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const port = Number(process.argv[2] || 7989);
  let n = 0;
  http.createServer((req, res) => {
    if (req.url === "/health") { res.end("ok"); return; }
    res.writeHead(200, { "Content-Type": "image/png", "Cache-Control": "no-store" });
    res.end(png(160, ++n));
  }).listen(port, "127.0.0.1");
}
