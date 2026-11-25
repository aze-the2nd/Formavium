// Erzeugt das Quellbild für die App-Symbole (1024×1024 PNG) ohne Abhängigkeiten.
// Aufruf: node scripts/make-icon.mjs && npx tauri icon app-icon.png
// Vorläufiges Zeichen: abgerundete Fläche in Markenblau mit einem stilisierten „f“.
import { writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const N = 1024;
const px = Buffer.alloc(N * N * 4);
const brand = [31, 58, 95];
const accent = [108, 156, 255];

function inRoundRect(x, y, r) {
  const m = 64;
  const lo = m + r, hi = N - m - r;
  if (x < m || y < m || x >= N - m || y >= N - m) return false;
  const cx = Math.min(Math.max(x, lo), hi);
  const cy = Math.min(Math.max(y, lo), hi);
  return (x - cx) ** 2 + (y - cy) ** 2 <= r * r;
}

for (let y = 0; y < N; y++) {
  for (let x = 0; x < N; x++) {
    const i = (y * N + x) * 4;
    if (!inRoundRect(x, y, 180)) continue;
    let c = brand;
    // Stamm des „f“
    if (x >= 430 && x < 530 && y >= 300 && y < 800) c = [255, 255, 255];
    // Bogen oben
    const dx = x - 600, dy = y - 330;
    const d = Math.sqrt(dx * dx + dy * dy);
    if (d >= 70 && d < 170 && y < 330 && x >= 430) c = [255, 255, 255];
    // Querbalken als Akzent (Modulbaustein)
    if (x >= 340 && x < 660 && y >= 470 && y < 550) c = accent;
    px.set([...c, 255], i);
  }
}

const raw = Buffer.alloc(N * (N * 4 + 1));
for (let y = 0; y < N; y++) px.copy(raw, y * (N * 4 + 1) + 1, y * N * 4, (y + 1) * N * 4);

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
ihdr.writeUInt32BE(N, 0);
ihdr.writeUInt32BE(N, 4);
ihdr.set([8, 6, 0, 0, 0], 8);
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw, { level: 9 })),
  chunk('IEND', Buffer.alloc(0)),
]);
writeFileSync(new URL('../app-icon.png', import.meta.url), png);
console.log('app-icon.png geschrieben');
