// Erzeugt das Beispielbild des Übungsdokuments (PNG, ohne Abhängigkeiten):
// ein Montagewinkel mit zwei Bohrungen und einer Maßlinie.
// Aufruf: node scripts/make-sample-image.mjs
import { writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const W = 640, H = 420, S = 3; // S: Überabtastung für glatte Kanten
const bg = [238, 242, 247], steel = [31, 58, 95], edge = [108, 156, 255], line = [107, 118, 134];

const inRect = (x, y, x0, y0, x1, y1) => x >= x0 && x < x1 && y >= y0 && y < y1;
const inCircle = (x, y, cx, cy, r) => (x - cx) ** 2 + (y - cy) ** 2 <= r * r;

function color(x, y) {
  // Bohrungen
  if (inCircle(x, y, 470, 150, 16) || inCircle(x, y, 185, 300, 16)) return bg;
  // Winkel: waagerechter und senkrechter Schenkel, heller Rand oben
  if (inRect(x, y, 140, 110, 540, 118)) return edge;
  if (inRect(x, y, 140, 110, 540, 190) || inRect(x, y, 140, 110, 230, 350)) return steel;
  // Maßlinie mit Endstrichen
  if (inRect(x, y, 140, 72, 540, 75)) return line;
  if (inRect(x, y, 140, 60, 143, 87) || inRect(x, y, 537, 60, 540, 87)) return line;
  return bg;
}

const raw = Buffer.alloc(H * (W * 3 + 1));
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const sum = [0, 0, 0];
    for (let sy = 0; sy < S; sy++) {
      for (let sx = 0; sx < S; sx++) {
        const c = color(x + (sx + 0.5) / S, y + (sy + 0.5) / S);
        sum[0] += c[0]; sum[1] += c[1]; sum[2] += c[2];
      }
    }
    const i = y * (W * 3 + 1) + 1 + x * 3;
    raw[i] = sum[0] / (S * S); raw[i + 1] = sum[1] / (S * S); raw[i + 2] = sum[2] / (S * S);
  }
}

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type), data]);
  const c = Buffer.alloc(4);
  c.writeUInt32BE(crc(td));
  return Buffer.concat([len, td, c]);
};
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8; // Bittiefe
ihdr[9] = 2; // RGB
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);
const out = new URL('../../crates/formavium-cli/assets/beispiel.png', import.meta.url);
writeFileSync(out, png);
console.log(`${out.pathname}: ${png.length} Bytes, ${W}×${H}`);
