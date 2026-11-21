// Składa promo w jeden plik: src/*.js i src/*.wgsl → index.html (czytelny) i index.min.html (zminifikowany).
// node build.mjs  (minifikacja: esbuild, jeśli jest – inaczej tylko czytelna wersja)
import fs from 'node:fs';
const read = (f) => fs.readFileSync(new URL('src/' + f, import.meta.url), 'utf8');
// WGSL w wersji zminifikowanej: bez komentarzy i zbędnych odstępów
const wgslMin = (src) => src.replace(/\/\/[^\n]*/g, '').replace(/\s+/g, ' ').replace(/ ?([{}()\[\],;:=<>+\-*\/&|!?.@]) ?/g, '$1').trim();
const shaders = (min) => ['products', 'scene', 'post'].map((n) => `const WGSL_${n.toUpperCase()} = ${JSON.stringify(min ? wgslMin(read(n + '.wgsl')) : read(n + '.wgsl'))};`).join('\n');
const body = [read('i18n.js'), read('ui.js'), read('main.js')].join('\n');
const js = [shaders(false), body].join('\n');
const html = (code) => `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>AsOne</title>
<style>html,body{margin:0;height:100%;background:#000;overflow:hidden}canvas{display:block;width:100%;height:100%}#hud{position:fixed;left:16px;bottom:14px;font:13px system-ui,sans-serif;color:#9aa;opacity:.85;user-select:none}#hud button{font:inherit;background:#1d1f24;color:#cfd3da;border:1px solid #333840;border-radius:6px;padding:4px 10px;margin-right:6px;cursor:pointer}#msg{position:fixed;inset:0;display:flex;align-items:center;justify-content:center;font:15px system-ui,sans-serif;color:#cfd3da;text-align:center;padding:24px}</style></head>
<body><canvas id="c"></canvas><div id="hud"></div><div id="msg"></div><script type="module">${code}</script></body></html>`;
fs.writeFileSync(new URL('index.html', import.meta.url), html(js));
let min = null;
try {
  const esbuild = await import(process.env.ESBUILD || 'esbuild');
  const r = await esbuild.transform([shaders(true), body].join('\n'), { minify: true, format: 'esm', target: 'es2022', legalComments: 'none' });
  min = html(r.code);
  fs.writeFileSync(new URL('index.min.html', import.meta.url), min);
} catch (e) { console.log('minifikacja pominięta:', e.message); }
console.log('index.html', fs.statSync(new URL('index.html', import.meta.url)).size, 'B', min ? `· index.min.html ${Buffer.byteLength(min)} B` : '');
