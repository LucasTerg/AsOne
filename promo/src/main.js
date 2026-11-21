// AsOne – film promocyjny w czasie rzeczywistym (WebGPU). Jeden plik, wszystko proceduralne.
// Parametry adresu: ?lang=en|pl  ?t=sekunda  ?free  ?perf (znaczniki czasu GPU)  ?record (sterowanie z zewnątrz)

// --- Prawdziwe liczby z przebiegu w AsOne 0.1.29 (zdjęcia z tego filmu, IMG_2041–2043) ----------
const REAL = {
  photo: [1600, 1200],
  crop: [354, 212, 1133, 1037],            // „Crop (auto)”: 779 × 825 px (od 354, 212)
  square: [0, 0, 23, 23],                  // „Fit to square 1:1”: 825 × 825
  frame: [5, 5, 28, 28],                   // + „Frame 5 px”: 835 × 835
  export: {
    dir: '/home/user/shop/web',
    summary: { en: 'Saved 3 of 3 files · 165 KB · 0.2 s', pl: 'Zapisano 3 z 3 plików · 165 KB · 0.2 s' },
    rows: [
      { name: 'Mug-ASONE-Classic-1.jpg', dims: '835×835', kb: 34, q: 90 },
      { name: 'Mug-ASONE-Classic-2-ver2.jpg', dims: '1600×1200', kb: 65, q: 90 },
      { name: 'Mug-ASONE-Classic-3.jpg', dims: '1600×1200', kb: 66, q: 90 },
    ],
  },
  files: ['IMG_2041.jpg', 'IMG_2042.jpg', 'IMG_2043.jpg'],
  name: 'Mug ASONE Classic',
};
const TOAST = { en: 'Crop to object: 779 × 825 px', pl: 'Kadr do obiektu: 779 × 825 px' };

const Q = new URLSearchParams(location.search);
let LANG = Q.get('lang') || ((navigator.language || 'en').toLowerCase().startsWith('pl') ? 'pl' : 'en');
const FILM_LEN = 85;
const msg = document.getElementById('msg');

// --- Matematyka bez alokacji -------------------------------------------------------------------
const V = () => new Float32Array(3);
function mul4(o, a, b) {
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
  }
}
function perspective(o, fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2);
  o.fill(0); o[0] = f / aspect; o[5] = f; o[10] = far / (near - far); o[11] = -1; o[14] = near * far / (near - far);
}
function lookAt(o, eye, at, right, up) {
  let fx = at[0] - eye[0], fy = at[1] - eye[1], fz = at[2] - eye[2];
  const fl = Math.hypot(fx, fy, fz) || 1; fx /= fl; fy /= fl; fz /= fl;
  let rx = fy * 0 - fz * 1, ry = fz * 0 - fx * 0, rz = fx * 1 - fy * 0;
  const rl = Math.hypot(rx, ry, rz) || 1; rx /= rl; ry /= rl; rz /= rl;
  const ux = ry * fz - rz * fy, uy = rz * fx - rx * fz, uz = rx * fy - ry * fx;
  right[0] = rx; right[1] = ry; right[2] = rz; up[0] = ux; up[1] = uy; up[2] = uz;
  o[0] = rx; o[1] = ux; o[2] = -fx; o[3] = 0;
  o[4] = ry; o[5] = uy; o[6] = -fy; o[7] = 0;
  o[8] = rz; o[9] = uz; o[10] = -fz; o[11] = 0;
  o[12] = -(rx * eye[0] + ry * eye[1] + rz * eye[2]);
  o[13] = -(ux * eye[0] + uy * eye[1] + uz * eye[2]);
  o[14] = fx * eye[0] + fy * eye[1] + fz * eye[2]; o[15] = 1;
}
function invert4(o, m) {
  const a00 = m[0], a01 = m[1], a02 = m[2], a03 = m[3], a10 = m[4], a11 = m[5], a12 = m[6], a13 = m[7];
  const a20 = m[8], a21 = m[9], a22 = m[10], a23 = m[11], a30 = m[12], a31 = m[13], a32 = m[14], a33 = m[15];
  const b00 = a00 * a11 - a01 * a10, b01 = a00 * a12 - a02 * a10, b02 = a00 * a13 - a03 * a10, b03 = a01 * a12 - a02 * a11;
  const b04 = a01 * a13 - a03 * a11, b05 = a02 * a13 - a03 * a12, b06 = a20 * a31 - a21 * a30, b07 = a20 * a32 - a22 * a30;
  const b08 = a20 * a33 - a23 * a30, b09 = a21 * a32 - a22 * a31, b10 = a21 * a33 - a23 * a31, b11 = a22 * a33 - a23 * a32;
  const det = 1 / (b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06);
  o[0] = (a11 * b11 - a12 * b10 + a13 * b09) * det; o[1] = (a02 * b10 - a01 * b11 - a03 * b09) * det;
  o[2] = (a31 * b05 - a32 * b04 + a33 * b03) * det; o[3] = (a22 * b04 - a21 * b05 - a23 * b03) * det;
  o[4] = (a12 * b08 - a10 * b11 - a13 * b07) * det; o[5] = (a00 * b11 - a02 * b08 + a03 * b07) * det;
  o[6] = (a32 * b02 - a30 * b05 - a33 * b01) * det; o[7] = (a20 * b05 - a22 * b02 + a23 * b01) * det;
  o[8] = (a10 * b10 - a11 * b08 + a13 * b06) * det; o[9] = (a01 * b08 - a00 * b10 - a03 * b06) * det;
  o[10] = (a30 * b04 - a31 * b02 + a33 * b00) * det; o[11] = (a21 * b02 - a20 * b04 - a23 * b00) * det;
  o[12] = (a11 * b07 - a10 * b09 - a12 * b06) * det; o[13] = (a00 * b09 - a01 * b07 + a02 * b06) * det;
  o[14] = (a31 * b01 - a30 * b03 - a32 * b00) * det; o[15] = (a20 * b03 - a21 * b01 + a22 * b00) * det;
}
const clamp = (x, a, b) => (x < a ? a : x > b ? b : x);
const sstep = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t); };
const s5 = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * t * (t * (t * 6 - 15) + 10); };
const lerp = (a, b, t) => a + (b - a) * t;

// --- Okno AsOne w świecie: 16 × 10 jednostek, piksel logiczny 1440×900 → świat --------------------
const WX = (lx) => lx / 90 - 8, WY = (ly) => 5 - ly / 90;

// --- Kamera: klatki kluczowe [czas, pozycja, cel, przysłona] (krzywa Hermite'a z prędkościami Catmull-Rom) --
const CAM = [
  [0, [0.6, 0.9, 31], [0, 0.2, 14], 0.02],
  [7.5, [-1.2, 0.3, 24.5], [0, 0, 12], 0.025],
  [10.5, [0, 0.6, 19.5], [0, 0, 0], 0.008],
  [12.8, [-2.6, 0.9, 13.5], [-3.2, 0.5, 0], 0.01],
  [15.5, [-3.6, 1.6, 10.5], [-4.2, 1.0, 0], 0.012],
  [18.5, [-4.7, 3.75, 4.3], [-5.3, 4.32, 0], 0.03],
  [23.5, [-4.25, 3.6, 3.9], [-4.9, 4.3, 0], 0.032],
  [26.6, [-1.4, 3.0, 6.2], [-1.2, 3.8, 0], 0.026],
  [28.6, [-2.2, 3.1, 5.6], [-2.4, 3.6, 0], 0.026],
  [30.8, [5.0, 1.5, 4.4], [5.65, 1.45, 0], 0.03],
  [32.6, [0.3, -0.4, 8.3], [0.14, -0.69, 0], 0.012],
  [35.8, [0.25, -0.5, 7.6], [0.14, -0.69, 0], 0.014],
  [37.4, [2.0, 0.2, 10.2], [1.7, -0.25, 0], 0.01],
  [40.2, [2.2, 0.1, 10.0], [2.0, -0.25, 0], 0.01],
  [43.5, [2.45, 0.05, 10.4], [2.55, -0.2, 0], 0.01],
  [48.4, [2.35, 0.05, 10.2], [2.5, -0.2, 0], 0.01],
  [50.8, [0.9, -0.15, 12.4], [0.5, -0.5, 0], 0.008],
  [53.2, [-4.7, -0.2, 6.6], [-4.6, -0.8, 0], 0.02],
  [55.6, [-4.5, -0.3, 6.2], [-4.4, -0.85, 0], 0.02],
  [57.6, [-5.3, 0.6, 4.6], [-5.4, 0.35, 0], 0.03],
  [59.4, [-5.2, 0.6, 4.4], [-5.35, 0.4, 0], 0.03],
  [60.15, [0.4, 2.6, 12.5], [1.6, 2.5, 0], 0.006],
  [61.0, [5.5, 3.6, 5.6], [6.3, 3.95, 0], 0.028],
  [64.0, [4.6, 2.2, 14.5], [6.8, 1.7, 0.5], 0.008],
  [67.0, [0.3, 1.3, 8.2], [0.05, 1.05, 0], 0.012],
  [70.5, [0.25, 1.2, 7.8], [0.05, 1.05, 0], 0.012],
  [73.5, [0.4, 0.8, 23], [0, 0, 0], 0.006],
  [79, [0.1, 0.4, 20.5], [0, 0.1, 0], 0.006],
  [85, [0, 0.3, 27], [0, 0.2, 0], 0.004],
];
function hermite(out, k, t, field) {
  let i = 0;
  while (i < k.length - 2 && t > k[i + 1][0]) i++;
  const a = k[i], b = k[i + 1];
  const u = s5(a[0], b[0], t) * 0.35 + clamp((t - a[0]) / (b[0] - a[0]), 0, 1) * 0.65;
  const u2 = u * u, u3 = u2 * u;
  const h00 = 2 * u3 - 3 * u2 + 1, h10 = u3 - 2 * u2 + u, h01 = -2 * u3 + 3 * u2, h11 = u3 - u2;
  const pa = k[Math.max(i - 1, 0)], pb = k[Math.min(i + 2, k.length - 1)];
  const dt = b[0] - a[0];
  for (let c = 0; c < 3; c++) {
    const m0 = (b[field][c] - pa[field][c]) / (b[0] - pa[0] || 1) * dt * 0.5;
    const m1 = (pb[field][c] - a[field][c]) / (pb[0] - a[0] || 1) * dt * 0.5;
    out[c] = h00 * a[field][c] + h10 * m0 + h01 * b[field][c] + h11 * m1;
  }
  return lerp(a[3], b[3], u);
}

// --- Stan filmu w chwili t (bez alokacji: pola obiektu nadpisywane) --------------------------------
const S = {
  sweep: 0, files: 0, nameLen: 0, active: 2, bg: false, removal: 0, inspect: 0, cropStep: 0, view: 0,
  dimsIdx: 0, menu: 0, menuHover: -1, verHover: -1, ver2: false, dialog: false, pressed: '', toast: false, caption: -1, captionA: 0,
  endA: 0, endDim: 0, splitA: 0, split: 0, flyOut: 0, caret: false, curX: 0, curY: 0, curA: 0, click: 0,
};
const NAME_TIMES = []; // chwile kolejnych liter „Mug ASONE Classic”
for (let i = 0; i < 17; i++) NAME_TIMES.push(19.4 + i * 0.24);
const CAPS = [[1, 7.4], [9.2, 17.4], [18.6, 27.4], [29.2, 36.3], [36.9, 40.6], [42.4, 51.2], [52.6, 59.6], [60.6, 70.8], [75.0, 79.4]];
// kursor: [czas, x, y] w pikselach okna; kliknięcia: [czas, x, y, naciśnięty przycisk]
const CUR = [
  [17.4, 470, 330], [18.75, 190, 50], [19.3, 236, 72], [27.4, 236, 72], [28.15, 440, 110], [29.0, 452, 118],
  [30.85, 1239, 313], [31.6, 1239, 313], [32.4, 1110, 700], [36.3, 1110, 700], [36.85, 760, 520], [41.4, 760, 520],
  [42.5, 1278, 480], [44.6, 1278, 480], [45.2, 1226, 678], [46.6, 1226, 678], [47.2, 1209, 744], [47.8, 1209, 744],
  [48.5, 760, 520], [51.6, 760, 520], [52.6, 258, 410], [53.2, 258, 410], [53.6, 322, 482], [54.2, 322, 482],
  [54.7, 440, 482], [55.1, 476, 482], [58.8, 476, 482], [60.6, 1343, 76], [62.0, 1343, 76], [63.0, 1050, 600],
  [70.4, 1050, 600], [71.3, 690, 445], [73.5, 690, 445],
];
const CLICKS = [[18.9, ''], [28.3, ''], [31.1, 'bg'], [42.7, 'crop'], [45.4, 'square'], [47.4, 'frame'], [52.8, ''], [55.3, ''], [60.9, 'save'], [71.6, 'close']];
function evalState(t) {
  S.files = t < 13.4 ? 0 : t < 14.0 ? 1 : t < 14.6 ? 2 : 3;   // plik pojawia się, gdy jego zdjęcie doleci
  let n = 0; while (n < NAME_TIMES.length && t >= NAME_TIMES[n]) n++;
  S.nameLen = n;
  S.caret = t > 18.9 && t < 24.6;
  S.active = t < 28.35 ? Math.max(0, Math.min(2, S.files - 1)) : 0;
  S.pressed = ''; S.click = 0;
  for (let i = 0; i < CLICKS.length; i++) {
    const c = CLICKS[i], dt = t - c[0];
    if (dt > -0.05 && dt < 0.25) S.pressed = c[1];
    if (dt > -0.05 && dt < 0.5) S.click = 1 - dt / 0.5;
  }
  // kursor wzdłuż ścieżki (wygładzony ruch między punktami)
  let i = 0; while (i < CUR.length - 2 && t > CUR[i + 1][0]) i++;
  const a = CUR[i], b = CUR[i + 1], u = s5(a[0], b[0], t);
  S.curX = lerp(a[1], b[1], u); S.curY = lerp(a[2], b[2], u);
  S.curA = sstep(17.4, 17.8, t) * (1 - sstep(72.6, 73.2, t));
  S.removal = sstep(31.7, 35.8, t);
  S.bg = t >= 35.8;
  S.inspect = sstep(37.0, 37.2, t) * (1 - sstep(39.6, 39.8, t));
  S.cropStep = t < 42.95 ? 0 : t < 45.6 ? 1 : t < 47.6 ? 2 : 3;
  // przejścia widoku zdjęcia: 0 pełne zdjęcie, 1 kadr, 2 kwadrat, 3 ramka; view – płynne
  S.view = t < 42.95 ? 0 : t < 45.6 ? s5(42.95, 44.1, t) : t < 47.6 ? 1 + s5(45.6, 46.5, t) : 2 + s5(47.6, 48.3, t);
  S.toast = t > 43.0 && t < 46.2;
  S.dimsIdx = dimsIndex(t);
  S.menu = t < 52.9 ? 0 : t < 54.15 ? 1 : t < 55.45 ? 2 : 0;
  S.menuHover = t < 53.5 ? -1 : 2;
  S.verHover = t < 54.6 ? -1 : 0;
  S.ver2 = t >= 55.45;
  S.flyOut = clamp((t - 61.2) / 3.4, 0, 1);
  S.dialog = t > 63.6 && t < 71.8;
  if (!S.dialog && S.pressed === 'close') S.pressed = '';
  S.sweep = s5(72.2, 74.2, t);                       // najpierw motyw jasny…
  S.splitA = sstep(74.4, 75.0, t) * (1 - sstep(79.6, 80.4, t));   // …potem przed / po
  S.split = s5(75.0, 78.8, t);
  S.endDim = sstep(79.8, 81.0, t);
  S.endA = sstep(80.2, 81.4, t);
  S.caption = -1; S.captionA = 0;
  for (let k = 0; k < CAPS.length; k++) {
    const c = CAPS[k];
    if (t > c[0] - 0.6 && t < c[1] + 0.6) {
      const a = sstep(c[0] - 0.6, c[0] + 0.2, t) * (1 - sstep(c[1] - 0.2, c[1] + 0.6, t));
      if (a > S.captionA) { S.caption = k; S.captionA = a; }
    }
  }
}
// liczniki wymiarów: krok na 1/15 przejścia
const DIMS_STEPS = 14;
function dimsIndex(t) {
  if (t < 42.95) return 0;
  if (t < 45.6) return 1 + Math.round(clamp((t - 42.95) / 1.1, 0, 1) * DIMS_STEPS);
  if (t < 47.6) return 2 + DIMS_STEPS + Math.round(clamp((t - 45.6) / 0.9, 0, 1) * DIMS_STEPS);
  return 3 + 2 * DIMS_STEPS + Math.round(clamp((t - 47.6) / 0.7, 0, 1) * DIMS_STEPS) + (t > 48.6 ? 1 : 0);
}

// --- Start ------------------------------------------------------------------------------------
async function main() {
  if (!navigator.gpu) { msg.textContent = STR[LANG].noGpu; return; }
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) { msg.textContent = STR[LANG].noGpu; return; }
  const wantTs = Q.has('perf') && adapter.features.has('timestamp-query');
  const device = await adapter.requestDevice({ requiredFeatures: wantTs ? ['timestamp-query'] : [] });
  device.addEventListener('uncapturederror', (e) => console.error('GPU', e.error.message));
  const canvas = document.getElementById('c');
  const ctx = canvas.getContext('webgpu');
  const outFmt = navigator.gpu.getPreferredCanvasFormat();
  ctx.configure({ device, format: outFmt, alphaMode: 'opaque' });
  msg.textContent = '';

  // --- zdjęcia produktów (raz, przy starcie) ---------------------------------------------------
  const prodMod = device.createShaderModule({ code: WGSL_PRODUCTS });
  const prodPipe = device.createRenderPipeline({ layout: 'auto', vertex: { module: prodMod, entryPoint: 'vs' },
    fragment: { module: prodMod, entryPoint: 'fs', targets: [{ format: 'rgba8unorm' }] } });
  const SHOTS = [
    [0, [1.25, 1.05, 2.6], [0.05, 0.42, 0], 0.5, [0.18, 0.05], [0.95, 0.95, 0.95], [0.58, 0.58, 0.6]],
    [0, [-0.5, 0.75, 2.9], [0.08, 0.4, 0], 0.48, [-0.2, 0.12], [0.93, 0.93, 0.92], [0.55, 0.55, 0.56]],
    [0, [0.7, 2.0, 2.4], [0.05, 0.35, 0], 0.58, [0.08, -0.04], [0.94, 0.94, 0.95], [0.6, 0.6, 0.62]],
    [1, [1.1, 1.1, 3.2], [0, 0.62, 0], 0.55, [0.1, 0.05], [0.94, 0.94, 0.94], [0.56, 0.56, 0.58]],
    [2, [0.9, 0.95, 2.1], [-0.02, 0.22, 0], 0.5, [-0.1, 0.08], [0.94, 0.94, 0.93], [0.57, 0.57, 0.57]],
    [3, [-0.9, 0.95, 2.6], [0, 0.6, 0], 0.52, [0.15, 0.0], [0.93, 0.94, 0.95], [0.55, 0.57, 0.6]],
  ];
  const [PW, PH] = REAL.photo;
  const PMIPS = Math.floor(Math.log2(PW)) + 1;
  const mkArray = (layers) => device.createTexture({ size: [PW, PH, layers], format: 'rgba8unorm', viewFormats: ['rgba8unorm-srgb'], mipLevelCount: PMIPS,
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_SRC });
  const photoTex = mkArray(6), maskTex = mkArray(3);
  const thumbTex = device.createTexture({ size: [240, 180], format: 'rgba8unorm', usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC });
  const thumbBuf = device.createBuffer({ size: 256 * 4 * 180, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const shotBuf = device.createBuffer({ size: 80, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const shotBG = device.createBindGroup({ layout: prodPipe.getBindGroupLayout(0), entries: [{ binding: 0, resource: { buffer: shotBuf } }] });
  const shotU = new Float32Array(20);
  function renderPhoto(i, mode, view, w, h) {
    const s = SHOTS[i];
    shotU.set(s[1], 0); shotU[3] = s[3]; shotU.set(s[2], 4); shotU[7] = s[0]; shotU.set(s[5], 8); shotU[11] = mode;
    shotU.set(s[6], 12); shotU[15] = i * 13; shotU[16] = s[4][0]; shotU[17] = s[4][1]; shotU[18] = w; shotU[19] = h;
    device.queue.writeBuffer(shotBuf, 0, shotU);
    const enc = device.createCommandEncoder();
    const pass = enc.beginRenderPass({ colorAttachments: [{ view, loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
    pass.setPipeline(prodPipe); pass.setBindGroup(0, shotBG); pass.draw(3); pass.end();
    device.queue.submit([enc.finish()]);
  }
  for (let i = 0; i < 6; i++) renderPhoto(i, 0, photoTex.createView({ dimension: '2d', baseArrayLayer: i, arrayLayerCount: 1, baseMipLevel: 0, mipLevelCount: 1 }), PW, PH);
  for (let i = 0; i < 3; i++) renderPhoto(i, 2, maskTex.createView({ dimension: '2d', baseArrayLayer: i, arrayLayerCount: 1, baseMipLevel: 0, mipLevelCount: 1 }), PW, PH);
  // miniatury do UI (Canvas2D potrzebuje obrazów)
  const thumbs = [];
  for (let i = 0; i < 3; i++) {
    renderPhoto(i, 0, thumbTex.createView(), 240, 180);
    const enc = device.createCommandEncoder();
    enc.copyTextureToBuffer({ texture: thumbTex }, { buffer: thumbBuf, bytesPerRow: 1024 }, [240, 180]);
    device.queue.submit([enc.finish()]);
    await thumbBuf.mapAsync(GPUMapMode.READ);
    const raw = new Uint8Array(thumbBuf.getMappedRange());
    const px = new Uint8ClampedArray(240 * 180 * 4);
    for (let y = 0; y < 180; y++) px.set(raw.subarray(y * 1024, y * 1024 + 960), y * 960);
    thumbBuf.unmap();
    thumbs.push(await createImageBitmap(new ImageData(px, 240, 180)));
  }
  window.__photos = { renderPhoto, photoTex, SHOTS };

  // --- tekstury UI (ciemny i jasny motyw) + generowanie mipmap -----------------------------------
  const UIW = UI_W * UI_S, UIH = UI_H * UI_S;
  const mips = Math.floor(Math.log2(Math.max(UIW, UIH))) + 1;
  const mkUiTex = () => device.createTexture({ size: [UIW, UIH], format: 'rgba8unorm-srgb', mipLevelCount: mips,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.COPY_SRC | GPUTextureUsage.RENDER_ATTACHMENT });
  // trzy tekstury: dwie ciemne (bieżąca i budowana z wyprzedzeniem) oraz jasna (stan końcowy filmu)
  // cztery tekstury: trzy ciemne (bieżąca, poprzednia – do przenikania, budowana z wyprzedzeniem) i jasna
  const uiTex = [mkUiTex(), mkUiTex(), mkUiTex(), mkUiTex()];
  const uis = [0, 1, 2].map(() => new AsOneUI('dark', LANG, thumbs, true));
  uis.push(new AsOneUI('light', LANG, thumbs, true));
  for (const u of uis) u.fileNames = REAL.files;
  const mipMod = device.createShaderModule({ code: `
    @group(0) @binding(0) var src: texture_2d<f32>; @group(0) @binding(1) var smp: sampler;
    struct O { @builtin(position) p: vec4f, @location(0) uv: vec2f };
    @vertex fn vs(@builtin(vertex_index) i: u32) -> O { var q = array<vec2f,3>(vec2f(-1,-1), vec2f(3,-1), vec2f(-1,3)); var o: O; o.p = vec4f(q[i],0,1); o.uv = q[i]*vec2f(.5,-.5)+.5; return o; }
    @fragment fn fs(i: O) -> @location(0) vec4f { return textureSampleLevel(src, smp, i.uv, 0.0); }` });
  const mipPipe = device.createRenderPipeline({ layout: 'auto', vertex: { module: mipMod, entryPoint: 'vs' },
    fragment: { module: mipMod, entryPoint: 'fs', targets: [{ format: 'rgba8unorm-srgb' }] } });
  const linear = device.createSampler({ magFilter: 'linear', minFilter: 'linear', mipmapFilter: 'linear', maxAnisotropy: 16,
    addressModeU: 'clamp-to-edge', addressModeV: 'clamp-to-edge' });
  const mipSampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear' });
  const mipChain = uiTex.map((tex) => {
    const levels = [];
    for (let l = 1; l < mips; l++) {
      levels.push({
        view: tex.createView({ baseMipLevel: l, mipLevelCount: 1 }),
        bg: device.createBindGroup({ layout: mipPipe.getBindGroupLayout(0), entries: [
          { binding: 0, resource: tex.createView({ baseMipLevel: l - 1, mipLevelCount: 1 }) }, { binding: 1, resource: mipSampler }] }),
      });
    }
    return levels;
  });
  // mipmapy zdjęć i masek (raz) – bez migotania, gdy zdjęcie maleje do miniatury
  for (const [tex, layers] of [[photoTex, 6], [maskTex, 3]]) {
    const enc = device.createCommandEncoder();
    for (let layer = 0; layer < layers; layer++) {
      for (let l = 1; l < PMIPS; l++) {
        const v = (lv) => tex.createView({ format: 'rgba8unorm-srgb', dimension: '2d', baseArrayLayer: layer, arrayLayerCount: 1, baseMipLevel: lv, mipLevelCount: 1 });
        const bg = device.createBindGroup({ layout: mipPipe.getBindGroupLayout(0), entries: [{ binding: 0, resource: v(l - 1) }, { binding: 1, resource: mipSampler }] });
        const p = enc.beginRenderPass({ colorAttachments: [{ view: v(l), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
        p.setPipeline(mipPipe); p.setBindGroup(0, bg); p.draw(3); p.end();
      }
    }
    device.queue.submit([enc.finish()]);
  }
  // Odświeżanie interfejsu: programowe płótno (willReadFrequently) → tylko zmienione prostokąty → writeTexture.
  function regenMips(k) {
    const enc = device.createCommandEncoder();
    for (const lv of mipChain[k]) {
      const p = enc.beginRenderPass({ colorAttachments: [{ view: lv.view, loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
      p.setPipeline(mipPipe); p.setBindGroup(0, lv.bg); p.draw(3); p.end();
    }
    device.queue.submit([enc.finish()]);
  }
  const upDst = { texture: null, origin: [0, 0] }, upSize = [0, 0], upLayout = { bytesPerRow: 0 };
  function uploadRect(k, x0, y0, x1, y1) {
    // ten sam prostokąt co przycięcie rysowania (wyrównany do pikseli tekstury) – bez starych pikseli z brzegu
    const px0 = Math.max(0, Math.floor(x0 * UI_S)), py0 = Math.max(0, Math.floor(y0 * UI_S));
    const px1 = Math.min(UIW, Math.ceil(x1 * UI_S)), py1 = Math.min(UIH, Math.ceil(y1 * UI_S));
    if (px1 <= px0 || py1 <= py0) return;
    // kopiowanie bez tworzenia buforów w JS (getImageData za każdym razem przydziela nową pamięć → przerwy na GC)
    // getImageData + writeTexture: działa wszędzie (kopiowanie z płótna potrafi zawieść w trybie na żywo)
    const d = uis[k].g.getImageData(px0, py0, px1 - px0, py1 - py0);
    upDst.texture = uiTex[k]; upDst.origin[0] = px0; upDst.origin[1] = py0; upSize[0] = px1 - px0; upSize[1] = py1 - py0;
    upLayout.bytesPerRow = (px1 - px0) * 4;
    device.queue.writeTexture(upDst, d.data, upLayout, upSize);
  }
  function redrawRects(k, rects, n, off = 0) {
    const u = uis[k], g = u.g;
    g.save(); g.setTransform(1, 0, 0, 1, 0, 0); g.beginPath();
    for (let i = off; i < off + n; i++) {
      const x0 = Math.floor(rects[i * 4] * UI_S), y0 = Math.floor(rects[i * 4 + 1] * UI_S);
      g.rect(x0, y0, Math.ceil(rects[i * 4 + 2] * UI_S) - x0, Math.ceil(rects[i * 4 + 3] * UI_S) - y0);
    }
    g.clip(); u.draw(); g.restore();
    for (let i = off; i < off + n; i++) uploadRect(k, rects[i * 4], rects[i * 4 + 1], rects[i * 4 + 2], rects[i * 4 + 3]);
  }
  function uploadUi(k) {
    uis[k].draw();
    uploadRect(k, 0, 0, UI_W, UI_H);
    regenMips(k);
  }

  // --- atlas: pliki JPG, folder „web/”, „przed / po”, napisy i plansza końcowa ------------------
  const ATW = 2048, ATH = 2048;
  const atlasCanvas = document.createElement('canvas'); atlasCanvas.width = ATW; atlasCanvas.height = ATH;
  const ATM = Math.floor(Math.log2(ATW)) + 1;
  const atlasTex = device.createTexture({ size: [ATW, ATH], format: 'rgba8unorm-srgb', mipLevelCount: ATM,
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST | GPUTextureUsage.RENDER_ATTACHMENT });
  const A = { file: [], folder: null, before: null, after: null, caps: [], logo: null, tag: null, end: null, url: null,
    cursor: [1300, 8, 1372, 116], ring: [1390, 8, 1518, 136], white: [1536 / 2048, 24 / 2048] };
  const finalThumb = document.createElement('canvas'); finalThumb.width = 200; finalThumb.height = 200;
  function drawAtlas() {
    const g = atlasCanvas.getContext('2d'); const T = STR[LANG];
    g.clearRect(0, 0, ATW, ATH);
    const F = (px, w = 300) => `${w} ${px}px ${FONT}`;
    // pliki: kafelek 400×300 – miniatura, nazwa, wymiary i rozmiar (z prawdziwego eksportu)
    for (let i = 0; i < 3; i++) {
      const x = 8 + i * 416, y = 8, r = REAL.export.rows[i];
      g.save(); g.shadowColor = 'rgba(0,0,0,.5)'; g.shadowBlur = 14;
      g.fillStyle = '#F4F5F7'; g.beginPath(); g.roundRect(x, y, 400, 300, 16); g.fill(); g.restore();
      g.fillStyle = '#2680EB'; g.beginPath(); g.roundRect(x + 16, y + 16, 64, 28, 6); g.fill();
      g.fillStyle = '#fff'; g.font = F(17, 600); g.textBaseline = 'middle'; g.textAlign = 'center'; g.fillText('JPG', x + 48, y + 31);
      if (i === 0) g.drawImage(finalThumb, x + 120, y + 18, 160, 160);
      else g.drawImage(thumbs[i], x + 80, y + 28, 240, 150);
      g.fillStyle = '#16181c'; g.font = F(22, 400); g.fillText(r.name, x + 200, y + 222);
      g.fillStyle = '#5b6370'; g.font = F(19); g.fillText(`${r.dims} · ${r.kb} KB · ${T.quality_} ${r.q}`, x + 200, y + 258);
      A.file[i] = [x, y, x + 400, y + 300];
    }
    // folder „web/”
    { const x = 8, y = 330;
      g.save(); g.shadowColor = 'rgba(40,120,235,.55)'; g.shadowBlur = 30;
      g.fillStyle = '#2B6FC9'; g.beginPath(); g.moveTo(x + 20, y + 60); g.lineTo(x + 150, y + 60); g.lineTo(x + 180, y + 90); g.lineTo(x + 480, y + 90);
      g.lineTo(x + 480, y + 370); g.lineTo(x + 20, y + 370); g.closePath(); g.fill(); g.restore();
      g.fillStyle = '#3F8BF0'; g.beginPath(); g.roundRect(x + 20, y + 120, 460, 250, 18); g.fill();
      g.fillStyle = '#fff'; g.font = F(76, 500); g.textAlign = 'center'; g.fillText('web/', x + 250, y + 248);
      A.folder = [x, y + 40, x + 500, y + 380]; }
    // przed / po
    g.font = F(54, 400); g.textAlign = 'center';
    for (const [key, label, x] of [['before', LANG === 'pl' ? 'Przed' : 'Before', 540], ['after', LANG === 'pl' ? 'Po' : 'After', 860]]) {
      g.fillStyle = 'rgba(10,12,16,.78)'; g.beginPath(); g.roundRect(x, 350, 300, 96, 48); g.fill();
      g.fillStyle = '#fff'; g.fillText(label, x + 150, 400); A[key] = [x, 350, x + 300, 446];
    }
    // napisy (po jednym pasku 2000×120)
    for (let i = 0; i < T.caps.length; i++) {
      const y = 740 + i * 112;
      g.font = F(66, 300); const w = Math.min(g.measureText(T.caps[i]).width + 120, 2000);
      g.fillStyle = 'rgba(6,8,12,.0)'; g.fillRect(24, y, w, 112);
      g.shadowColor = 'rgba(0,0,0,.85)'; g.shadowBlur = 18; g.fillStyle = '#F2F5FA'; g.textAlign = 'left';
      g.fillText(T.caps[i], 84, y + 58); g.shadowBlur = 0;
      g.fillStyle = '#2680EB'; g.fillRect(40, y + 22, 8, 72);
      A.caps[i] = [24, y, 24 + w, y + 108];
    }
    // plansza końcowa: znak A1 + AsOne, hasło, systemy, adres
    { const x = 1350, y = 330;
      g.fillStyle = '#2680EB'; g.beginPath(); g.roundRect(x, y, 140, 140, 30); g.fill();
      g.fillStyle = '#fff'; g.font = F(76, 600); g.textAlign = 'center'; g.fillText('A1', x + 70, y + 74);
      g.font = F(110, 300); g.textAlign = 'left'; g.fillStyle = '#F2F5FA'; g.fillText('AsOne', x + 170, y + 72);
      A.logo = [x - 10, y - 10, x + 640, y + 150]; }
    const line = (key, text, y, px, color, w = 300) => {
      g.font = F(px, w); const tw = g.measureText(text).width; g.fillStyle = color; g.textAlign = 'left'; g.fillText(text, 30, y + px * 0.6);
      A[key] = [20, y - 10, 40 + tw, y + px * 1.25];
    };
    line('tag', T.tagline, 1760, 62, '#E6EBF2', 400);
    line('end', T.endLine, 1852, 50, '#C9D3E2');
    line('url', 'asone.lmk.one', 1950, 70, '#6AA8FF', 500);
    // kursor (strzałka 18×27 px w skali 4×), fala kliknięcia, biały piksel
    g.save(); g.translate(1302, 10); g.scale(4, 4);
    g.beginPath(); g.moveTo(0, 0); g.lineTo(0, 19.5); g.lineTo(4.6, 15.2); g.lineTo(7.8, 22.6); g.lineTo(10.8, 21.4); g.lineTo(7.6, 14.2); g.lineTo(13.6, 14.2); g.closePath();
    g.shadowColor = 'rgba(0,0,0,.45)'; g.shadowBlur = 2; g.shadowOffsetY = 0.6; g.fillStyle = '#fff'; g.fill();
    g.shadowColor = 'transparent'; g.lineWidth = 1.1; g.lineJoin = 'round'; g.strokeStyle = '#111'; g.stroke(); g.restore();
    g.save(); g.strokeStyle = '#5BA6FF'; g.shadowColor = '#2680EB'; g.shadowBlur = 10; g.lineWidth = 7;
    g.beginPath(); g.arc(1454, 72, 50, 0, Math.PI * 2); g.stroke(); g.restore();
    g.fillStyle = '#fff'; g.fillRect(1528, 16, 16, 16);
    device.queue.copyExternalImageToTexture({ source: atlasCanvas, premultipliedAlpha: true }, { texture: atlasTex }, [ATW, ATH]);
    const enc = device.createCommandEncoder();
    for (let l = 1; l < ATM; l++) {
      const bg = device.createBindGroup({ layout: mipPipe.getBindGroupLayout(0), entries: [{ binding: 0, resource: atlasTex.createView({ baseMipLevel: l - 1, mipLevelCount: 1 }) }, { binding: 1, resource: mipSampler }] });
      const p = enc.beginRenderPass({ colorAttachments: [{ view: atlasTex.createView({ baseMipLevel: l, mipLevelCount: 1 }), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 0] }] });
      p.setPipeline(mipPipe); p.setBindGroup(0, bg); p.draw(3); p.end();
    }
    device.queue.submit([enc.finish()]);
  }

  // --- zasoby renderowania -------------------------------------------------------------------
  const sceneMod = device.createShaderModule({ code: WGSL_SCENE });
  const postMod = device.createShaderModule({ code: WGSL_POST });
  for (const m of [sceneMod, postMod]) {
    const info = await m.getCompilationInfo();
    for (const e of info.messages) if (e.type === 'error') console.error('WGSL', e.lineNum, e.message);
  }
  const VF = GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
  const tex2d = (b, vis = GPUShaderStage.FRAGMENT) => ({ binding: b, visibility: vis, texture: { sampleType: 'float' } });
  const tex2da = (b, vis = GPUShaderStage.FRAGMENT) => ({ binding: b, visibility: vis, texture: { sampleType: 'float', viewDimension: '2d-array' } });
  const ubo = (b, vis = VF) => ({ binding: b, visibility: vis, buffer: { type: 'uniform' } });
  const L0 = device.createBindGroupLayout({ entries: [ubo(0), { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: {} }] });
  const LBg = device.createBindGroupLayout({ entries: [ubo(0)] });
  const LPanel = device.createBindGroupLayout({ entries: [{ binding: 1, visibility: VF, buffer: { type: 'read-only-storage' } }, tex2d(2), tex2d(3), tex2da(4), tex2d(5), tex2da(6), ubo(7), tex2d(8)] });
  const LParts = device.createBindGroupLayout({ entries: [ubo(7), ubo(10), tex2da(11, VF), tex2da(12, VF)] });
  const HDR = 'rgba16float', MS = 4;
  const premul = { color: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' }, alpha: { srcFactor: 'one', dstFactor: 'one-minus-src-alpha' } };
  const targets = [{ format: HDR, blend: premul }, { format: HDR, blend: premul }];
  const mkScene = (vs, fs, layout, depthWrite = true, depthTest = 'less') => device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: [L0, layout] }),
    vertex: { module: sceneMod, entryPoint: vs }, fragment: { module: sceneMod, entryPoint: fs, targets },
    primitive: { topology: 'triangle-list' }, multisample: { count: MS },
    depthStencil: { format: 'depth24plus', depthWriteEnabled: depthWrite, depthCompare: depthTest } });
  const pipeBg = mkScene('vs_bg', 'fs_bg', LBg, false, 'always');
  const pipePanel = mkScene('vs_panel', 'fs_panel', LPanel, true, 'less');
  const pipePanelT = mkScene('vs_panel', 'fs_panel', LPanel, false, 'less');
  const pipeParts = mkScene('vs_parts', 'fs_parts', LParts, false, 'less');

  const frameBuf = device.createBuffer({ size: 112, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const frameU = new Float32Array(28);
  const bgBuf = device.createBuffer({ size: 80, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const bgU = new Float32Array(20);
  const MAXP = 16;
  const panelBuf = device.createBuffer({ size: MAXP * 80, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  const panelU = new Float32Array(MAXP * 20);
  const canvasBuf = device.createBuffer({ size: 96, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const canvasU = new Float32Array(24);
  const partsBuf = device.createBuffer({ size: 64, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const partsU = new Float32Array(16);
  const photoView = photoTex.createView({ dimension: '2d-array', format: 'rgba8unorm-srgb' });
  const maskView = maskTex.createView({ dimension: '2d-array' });
  const bg0 = device.createBindGroup({ layout: L0, entries: [{ binding: 0, resource: { buffer: frameBuf } }, { binding: 1, resource: linear }] });
  const bgBgG = device.createBindGroup({ layout: LBg, entries: [{ binding: 0, resource: { buffer: bgBuf } }] });
  // grupy wiązań dla każdej pary (bieżąca, poprzednia) tekstura interfejsu
  const bgPanels = [];
  for (let k = 0; k < 9; k++) {
    const cur = Math.floor(k / 3), prev = k % 3;
    bgPanels.push(device.createBindGroup({ layout: LPanel, entries: [{ binding: 1, resource: { buffer: panelBuf } },
      { binding: 2, resource: uiTex[cur].createView() }, { binding: 3, resource: uiTex[3].createView() }, { binding: 4, resource: photoView },
      { binding: 5, resource: atlasTex.createView() }, { binding: 6, resource: maskView }, { binding: 7, resource: { buffer: canvasBuf } },
      { binding: 8, resource: uiTex[prev].createView() }] }));
  }
  const bgParts = device.createBindGroup({ layout: LParts, entries: [{ binding: 7, resource: { buffer: canvasBuf } }, { binding: 10, resource: { buffer: partsBuf } }, { binding: 11, resource: photoView }, { binding: 12, resource: maskView }] });

  // postproces
  const LPost = device.createBindGroupLayout({ entries: [ubo(0, GPUShaderStage.FRAGMENT), { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: {} },
    tex2d(2), tex2d(3), tex2d(4), tex2d(5)] });
  const LBlur = device.createBindGroupLayout({ entries: [ubo(0, GPUShaderStage.FRAGMENT)] });
  const LOver = device.createBindGroupLayout({ entries: [tex2d(1), ubo(2, GPUShaderStage.FRAGMENT)] });
  const overBuf = device.createBuffer({ size: 176, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const overU = new Float32Array(44);
  const bgOver = device.createBindGroup({ layout: LOver, entries: [{ binding: 1, resource: atlasTex.createView() }, { binding: 2, resource: { buffer: overBuf } }] });
  const mkPost = (fs, fmt, extra) => device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: extra ? [LPost, extra] : [LPost] }),
    vertex: { module: postMod, entryPoint: 'vs' }, fragment: { module: postMod, entryPoint: fs, targets: [{ format: fmt }] } });
  const pipeBright = mkPost('fs_bright', HDR), pipeBlur = mkPost('fs_blur', HDR, LBlur), pipeDof = mkPost('fs_dof', HDR), pipeFinal = mkPost('fs_final', outFmt, LOver);
  const postBuf = device.createBuffer({ size: 192, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const postU = new Float32Array(48);
  const blurBufs = [0, 1].map(() => device.createBuffer({ size: 16, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST }));
  const blurU = new Float32Array(4);
  const blurBGs = blurBufs.map((b) => device.createBindGroup({ layout: LBlur, entries: [{ binding: 0, resource: { buffer: b } }] }));
  const pointSampler = linear;

  // napisy w przestrzeni ekranu: małe płótno nakładki (rysowane, gdy zmienia się napis lub plansza)
  let RT = null; // cele renderowania zależne od rozmiaru
  function makeTargets(w, h) {
    const usage = GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING;
    const t = {
      w, h,
      msColor: device.createTexture({ size: [w, h], format: HDR, sampleCount: MS, usage: GPUTextureUsage.RENDER_ATTACHMENT }),
      msDepthC: device.createTexture({ size: [w, h], format: HDR, sampleCount: MS, usage: GPUTextureUsage.RENDER_ATTACHMENT }),
      msZ: device.createTexture({ size: [w, h], format: 'depth24plus', sampleCount: MS, usage: GPUTextureUsage.RENDER_ATTACHMENT }),
      color: device.createTexture({ size: [w, h], format: HDR, usage }),
      depth: device.createTexture({ size: [w, h], format: HDR, usage }),
      dof: device.createTexture({ size: [w, h], format: HDR, usage }),
      b1: device.createTexture({ size: [Math.ceil(w / 4), Math.ceil(h / 4)], format: HDR, usage }),
      b2: device.createTexture({ size: [Math.ceil(w / 4), Math.ceil(h / 4)], format: HDR, usage }),
      out: device.createTexture({ size: [w, h], format: outFmt, usage: usage | GPUTextureUsage.COPY_SRC }),
    };
    const v = (x) => x.createView();
    t.views = { msColor: v(t.msColor), msDepthC: v(t.msDepthC), msZ: v(t.msZ), color: v(t.color), depth: v(t.depth), dof: v(t.dof), b1: v(t.b1), b2: v(t.b2), out: v(t.out) };
    const pg = (aux, aux2) => device.createBindGroup({ layout: LPost, entries: [{ binding: 0, resource: { buffer: postBuf } }, { binding: 1, resource: pointSampler },
      { binding: 2, resource: t.views.color }, { binding: 3, resource: t.views.depth }, { binding: 4, resource: aux }, { binding: 5, resource: aux2 }] });
    t.bgBright = pg(t.views.b2, t.views.b2);
    t.bgBlurH = pg(t.views.b1, t.views.b1);
    t.bgBlurV = pg(t.views.b2, t.views.b2);
    t.bgDof = pg(t.views.b1, t.views.b1);
    t.bgFinal = pg(t.views.dof, t.views.b1);
    t.colorAtt = [{ view: t.views.msColor, resolveTarget: t.views.color, loadOp: 'clear', storeOp: 'discard', clearValue: [0, 0, 0, 1] },
      { view: t.views.msDepthC, resolveTarget: t.views.depth, loadOp: 'clear', storeOp: 'discard', clearValue: [1000, 0, 0, 1] }];
    t.sceneDesc = { colorAttachments: t.colorAtt, depthStencilAttachment: { view: t.views.msZ, depthClearValue: 1, depthLoadOp: 'clear', depthStoreOp: 'discard' } };
    const pd = (view) => ({ colorAttachments: [{ view, loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
    t.dBright = pd(t.views.b1); t.dBlurH = pd(t.views.b2); t.dBlurV = pd(t.views.b1); t.dDof = pd(t.views.dof);
    t.finalAtt = [{ view: null, loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }];
    t.dFinal = { colorAttachments: t.finalAtt };
    t.overKey = -2;
    return t;
  }
  // układ napisu i planszy końcowej na ekranie (uv) – liczony przy zmianie napisu albo rozmiaru
  function layoutOverlay(t, key) {
    overU.fill(0);
    const W = t.w, H = t.h, cap = key % 100, end = key >= 100, vertical = H > W;
    const sc = vertical ? W / 1500 : Math.min(W / 2560, H / 1440);
    const put = (slot, r, x, y, s) => {
      const w = (r[2] - r[0]) * s, h = (r[3] - r[1]) * s, o = slot * 8;
      overU[o] = x / W; overU[o + 1] = y / H; overU[o + 2] = (x + w) / W; overU[o + 3] = (y + h) / H;
      overU[o + 4] = r[0] / ATW; overU[o + 5] = r[1] / ATH; overU[o + 6] = r[2] / ATW; overU[o + 7] = r[3] / ATH;
      return h;
    };
    overU[42] = 1;
    if (cap >= 0 && cap < 99) {
      const r = A.caps[cap], s = Math.min(sc, W * 0.92 / (r[2] - r[0])), cw = (r[2] - r[0]) * s, ch = (r[3] - r[1]) * s;
      const x = vertical ? (W - cw) / 2 : W * 0.06, y = vertical ? H * 0.8 : cap === 2 ? H * 0.07 : H * 0.84;
      put(0, r, x, y, s); overU[42] = (y - ch * 1.6) / H;
    }
    if (end) {
      const cx = W / 2; let y = H * (vertical ? 0.36 : 0.3);
      const c = (slot, r, s) => put(slot, r, cx - (r[2] - r[0]) * s / 2, y, s);
      y += c(1, A.logo, sc * (vertical ? 1.15 : 1.25)) + 30 * sc;
      y += c(2, A.tag, sc * (vertical ? 0.95 : 1.1)) + 24 * sc;
      y += c(3, A.end, sc * (vertical ? 0.62 : 0.85)) + 40 * sc;
      c(4, A.url, sc * (vertical ? 1.0 : 1.05));
    }
    t.overKey = key;
  }
  // --- pomiar czasu GPU ------------------------------------------------------------------------
  let ts = null;
  if (wantTs) {
    ts = { set: device.createQuerySet({ type: 'timestamp', count: 2 }), res: device.createBuffer({ size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC }),
      read: device.createBuffer({ size: 16, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ }), busy: false, log: [] };
    ts.begin = { querySet: ts.set, beginningOfPassWriteIndex: 0 }; ts.end = { querySet: ts.set, endOfPassWriteIndex: 1 };
    ts.done = () => { const d = new BigInt64Array(ts.read.getMappedRange()); ts.log.push(ts.t, Number(d[1] - d[0]) / 1e6); ts.read.unmap(); ts.busy = false; };
  }

  // --- stan kamery i UI --------------------------------------------------------------------------
  const eye = V(), at = V(), right = V(), up = V();
  const view = new Float32Array(16), proj = new Float32Array(16), vp = new Float32Array(16), ivp = new Float32Array(16), pvp = new Float32Array(16);
  let havePrev = false;
  const free = { on: Q.has('free'), pos: new Float32Array([0, 0.5, 18]), yaw: Math.PI, pitch: 0, keys: new Set(), speed: 4 };
  let filmT = Number(Q.get('t') || 0), uiT = filmT, playing = !free.on, lastNow = 0;
  const dbgUi = [null, null];
  const perfDbg = Q.has('perf') ? [] : null;
  const nameSlugs = [];
  for (let i = 0; i <= REAL.name.length; i++) {
    const s = REAL.name.slice(0, i);
    nameSlugs.push([s, s.trim().replace(/\s+/g, '-')]);
  }
  // wymiary w kolejnych krokach (liczniki) – teksty przygotowane z góry
  const DIMS = { en: [], pl: [] };
  function buildDims() {
    for (const lang of ['en', 'pl']) {
      const T = STR[lang], out = DIMS[lang]; out.length = 0;
      const mk = (w, h, crop, m, zoom) => ({ source: '1600 × 1200 px', result: `${w} × ${h} px`, saved: `${w} × ${h} px`,
        cropLine: T.cropLine(crop[2] - crop[0], crop[3] - crop[1], crop[0], crop[1]), margins: m.map((v) => `${v} px`), zoom, status: T.status(zoom, 1600, 1200, w, h) });
      const full = [0, 0, 1600, 1200], c = REAL.crop;
      out.push(mk(1600, 1200, full, [0, 0, 0, 0], 46));
      for (let i = 0; i <= DIMS_STEPS; i++) { const k = i / DIMS_STEPS; out.push(mk(Math.round(lerp(1600, 779, k)), Math.round(lerp(1200, 825, k)), c, [0, 0, 0, 0], 46)); }
      for (let i = 0; i <= DIMS_STEPS; i++) { const k = i / DIMS_STEPS, m = Math.round(23 * k); out.push(mk(779 + 2 * m, 825, c, [0, 0, m, m], 46)); }
      for (let i = 0; i <= DIMS_STEPS; i++) { const k = i / DIMS_STEPS, m = Math.round(5 * k); out.push(mk(825 + 2 * m, 825 + 2 * m, c, [m, m, 23 + m, 23 + m], 46)); }
      const last = out[out.length - 1];
      out.push({ ...last, zoom: 83, status: T.status(83, 1600, 1200, 835, 835) });
    }
  }
  buildDims();
  function applyUi(ui, t) { fillState(ui.st); }
  function fillState(s) {
    const p = nameSlugs[S.nameLen];
    s.files = S.files; s.name = p[0]; s.slug = p[1]; s.active = S.active; s.bg = S.bg; s.crop = S.cropStep > 0;
    s.menu = S.menu; s.menuHover = S.menuHover; s.verHover = S.verHover; s.ver2 = S.ver2; s.inspect = S.inspect > 0.5;
    s.dialog = S.dialog; s.pressed = S.pressed; s.caret = S.caret; s.toast = S.toast ? TOAST[LANG] : null;
    s.dims = DIMS[LANG][Math.min(S.dimsIdx, DIMS[LANG].length - 1)];
    s.titleFile = REAL.files[S.active];
  }
  // prostokąty do odświeżenia po zmianie stanu (piksele okna); -1 – zmiana układu (całość)
  const dirty = new Float32Array(4 * 64);
  let nDirty = 0;
  function addRect(x0, y0, x1, y1) { if (nDirty >= 64) return; const o = nDirty * 4; dirty[o] = x0; dirty[o + 1] = y0; dirty[o + 2] = x1; dirty[o + 3] = y1; nDirty++; }
  function pressedRect(p, top) {
    if (p === 'save') addRect(1000, 58, 1440, top + 1);
    else if (p === 'close') addRect(440, 236, 1000, 482);
    else if (p) addRect(1160, top, 1440, 900);
  }
  function diffRects(a, b) {
    nDirty = 0;
    if ((a.files === 0) !== (b.files === 0) || (a.name.length > 0) !== (b.name.length > 0)) return -1;
    const top = b.name.length > 0 ? 97 : 72;
    if (a.files !== b.files) { addRect(0, 0, 1440, top + 1); addRect(47, top, 358, 900); addRect(358, top, 1160, top + 30); addRect(1160, top + 44, 1440, top + 162); addRect(358, 866, 1160, 900); }
    if (a.name !== b.name) { addRect(0, 32, 1440, top + 1); addRect(358, top, 1160, top + 30); addRect(1160, top + 60, 1440, top + 92); }
    if (a.caret !== b.caret) addRect(80, 36, 330, 64);
    if (a.active !== b.active) { addRect(0, 0, 1440, 32); addRect(358, top, 1160, top + 30); addRect(1160, top + 44, 1440, top + 162); }
    if (a.bg !== b.bg || a.crop !== b.crop) { addRect(1160, top, 1440, 900); addRect(0, top, 47, 900); }
    if (a.inspect !== b.inspect) { addRect(1160, top + 250, 1440, top + 320); addRect(358, 846, 1160, 900); }
    if (a.dims !== b.dims) { addRect(1160, top + 88, 1440, top + 162); addRect(1160, top + 320, 1440, top + 545); addRect(358, 866, 1160, 900); }
    if (a.toast !== b.toast) addRect(358, 846, 1160, 900);
    if (a.menu !== b.menu) addRect(250, top + 300, 640, top + 700);
    else {
      if (a.menuHover !== b.menuHover) addRect(258, top + 313, 412, top + 680);
      if (a.verHover !== b.verHover) addRect(412, top + 360, 600, top + 530);
    }
    if (a.ver2 !== b.ver2) { addRect(188, top + 230, 330, top + 402); addRect(358, top, 1160, top + 30); addRect(250, top + 300, 640, top + 700); }
    if (a.dialog !== b.dialog) addRect(440, 236, 1000, 482);
    if (a.pressed !== b.pressed) { pressedRect(a.pressed, top); pressedRect(b.pressed, top); }
    return nDirty;
  }
  function copyState(d, s) { for (const k in s) d[k] = s[k]; }
  // Wszystkie zmiany interfejsu są znane z osi czasu: następny stan budujemy z wyprzedzeniem w drugiej
  // teksturze (kopia GPU bieżącej + przerysowane paski, ograniczony koszt na klatkę), a w chwili zdarzenia
  // tylko zamieniamy tekstury. Skok w czasie (tryb swobodny, powtórka kroku) – odświeżenie od razu.
  const EV = [];                          // { t, st }
  let uiCur = 0, uiPrev = 1, curEv = -1, fadeT0 = 0, fadeDur = 0, uiFade = 0;
  const STRIP = 0.22e6, BUDGET = 0.45e6;    // pikseli tekstury na pasek / na klatkę
  const build = { ev: -1, k: 2, rects: new Float32Array(4 * 256), n: 0, i: 0, ready: false };
  function scanEvents() {
    EV.length = 0;
    let last = -1;
    for (let f = 0; f <= FILM_LEN * 240; f++) {
      const t = f / 240;
      evalState(t);
      const sig = uiSignature();
      if (sig !== last) { const st = {}; fillState(st); EV.push({ t, st, fade: 0 }); last = sig; }
    }
    // przenikanie: zmiana układu 0,3 s, okno dialogowe 0,25 s, menu 0,1 s; reszta natychmiast (jak w programie)
    for (let i = 1; i < EV.length; i++) {
      const a = EV[i - 1].st, b = EV[i].st;
      EV[i].fade = diffRects(a, b) < 0 ? 0.3 : a.dialog !== b.dialog ? 0.25 : a.menu !== b.menu ? 0.1 : 0;
    }
  }
  function eventAt(t) {
    let i = curEv < 0 ? 0 : curEv;
    if (i >= EV.length || EV[i].t > t) i = 0;
    while (i + 1 < EV.length && EV[i + 1].t <= t) i++;
    return i;
  }
  function pushStrips(x0, y0, x1, y1) {
    const area = (x1 - x0) * (y1 - y0) * UI_S * UI_S, k = Math.max(1, Math.ceil(area / STRIP)), h = (y1 - y0) / k;
    for (let j = 0; j < k && build.n < 256; j++) {
      const o = build.n * 4; build.rects[o] = x0; build.rects[o + 1] = y0 + j * h; build.rects[o + 2] = x1; build.rects[o + 3] = y0 + (j + 1) * h; build.n++;
    }
  }
  function startBuild(e) {
    const k = 3 - uiCur - uiPrev;   // wolna tekstura (ani bieżąca, ani przenikana)
    build.k = k;
    const enc = device.createCommandEncoder();
    enc.copyTextureToTexture({ texture: uiTex[uiCur] }, { texture: uiTex[k] }, [UIW, UIH]);
    device.queue.submit([enc.finish()]);
    copyState(uis[k].st, EV[e].st);
    build.ev = e; build.n = 0; build.i = 0; build.ready = false;
    const n = diffRects(EV[curEv].st, EV[e].st);
    if (n < 0) pushStrips(0, 0, UI_W, UI_H);
    else for (let i = 0; i < n; i++) pushStrips(dirty[i * 4], dirty[i * 4 + 1], dirty[i * 4 + 2], dirty[i * 4 + 3]);
  }
  function stepBuild(budget) {
    if (build.ev < 0 || build.ready) return;
    const k = build.k;
    let spent = 0, j = build.i;
    while (j < build.n && (spent === 0 || spent < budget)) {
      const o = j * 4; spent += (build.rects[o + 2] - build.rects[o]) * (build.rects[o + 3] - build.rects[o + 1]) * UI_S * UI_S; j++;
    }
    if (j > build.i) redrawRects(k, build.rects, j - build.i, build.i);
    build.i = j;
    if (build.i >= build.n) { regenMips(k); build.ready = true; }
  }
  function syncTo(e) {
    // bezpośrednie odświeżenie bieżącej tekstury (skoki w czasie)
    const cur = uis[uiCur];
    const n = curEv < 0 ? -1 : diffRects(cur.st, EV[e].st);
    copyState(cur.st, EV[e].st);
    if (n < 0) uploadUi(uiCur); else if (n > 0) { redrawRects(uiCur, dirty, n); regenMips(uiCur); }
    curEv = e;
  }
  function updateUi(t) {
    const target = eventAt(t);
    if (target !== curEv) {
      if (build.ev > curEv && build.ev <= target) {
        stepBuild(Infinity); uiPrev = uiCur; uiCur = build.k; curEv = build.ev;
        fadeT0 = EV[curEv].t; fadeDur = EV[curEv].fade;
      }
      build.ev = -1;
      if (curEv !== target) { syncTo(target); fadeDur = 0; }
    }
    uiFade = fadeDur > 0 ? 1 - clamp((t - fadeT0) / fadeDur, 0, 1) : 0;
    if (build.ev < 0 && curEv + 1 < EV.length) startBuild(curEv + 1);
  }
  function buildAllUi() {
    scanEvents();
    evalState(73); fillState(uis[3].st); uploadUi(3);            // jasny motyw: stan po eksporcie
    curEv = -1; build.ev = -1; uiCur = 0; uiPrev = 1; fadeDur = 0;
    syncTo(eventAt(filmT));
  }
  function uiSignature() {
    return (S.files) + S.nameLen * 4 + S.active * 80 + (S.bg ? 400 : 0) + S.dimsIdx * 800 + S.menu * 64000 + (S.menuHover + 1) * 192000
      + (S.verHover + 1) * 384000 + (S.ver2 ? 768000 : 0) + (S.inspect > 0.5 ? 1536000 : 0) + (S.dialog ? 3072000 : 0)
      + (S.caret ? 6144000 : 0) + (S.toast ? 12288000 : 0) + (S.pressed ? (S.pressed.charCodeAt(0) + S.pressed.length) * 24576000 : 0);
  }
  // obraz „po” dla kafelka pliku #1 i panelu przed/po: kadr + marginesy na białym (z tekstury, raz)
  async function buildFinalThumb() {
    const c = REAL.crop, m = REAL.frame;
    const g = finalThumb.getContext('2d'); g.fillStyle = '#fff'; g.fillRect(0, 0, 200, 200);
    const w = 835, h = 835, k = 200 / w;
    // sylwetka z maski × zdjęcie: odczyt fragmentu (tylko przy starcie)
    const buf = device.createBuffer({ size: 1600 * 4 * 1200, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const enc = device.createCommandEncoder();
    enc.copyTextureToBuffer({ texture: photoTex, origin: [0, 0, 0] }, { buffer: buf, bytesPerRow: 6400 }, [1600, 1200]);
    device.queue.submit([enc.finish()]);
    const buf2 = device.createBuffer({ size: 1600 * 4 * 1200, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const enc2 = device.createCommandEncoder();
    enc2.copyTextureToBuffer({ texture: maskTex, origin: [0, 0, 0] }, { buffer: buf2, bytesPerRow: 6400 }, [1600, 1200]);
    device.queue.submit([enc2.finish()]);
    await buf.mapAsync(GPUMapMode.READ); await buf2.mapAsync(GPUMapMode.READ);
    const px = new Uint8Array(buf.getMappedRange()), mk = new Uint8Array(buf2.getMappedRange());
    const cw = c[2] - c[0], ch = c[3] - c[1];
    const img = new ImageData(cw, ch);
    for (let y = 0; y < ch; y++) for (let x = 0; x < cw; x++) {
      const si = ((y + c[1]) * 1600 + x + c[0]) * 4, di = (y * cw + x) * 4, a = mk[si] / 255;
      for (let j = 0; j < 3; j++) img.data[di + j] = Math.round(px[si + j] * a + 255 * (1 - a));
      img.data[di + 3] = 255;
    }
    buf.unmap(); buf2.unmap();
    const bmp = await createImageBitmap(img);
    g.drawImage(bmp, m[2] * k, m[0] * k, cw * k, ch * k);
  }
  await buildFinalThumb();
  drawAtlas();
  buildAllUi();

  // --- panele sceny ----------------------------------------------------------------------------
  // pływające zdjęcia (ujęcie 1): 6 paneli; kubki lecą potem do listy plików
  const FLOAT = [
    [-4.6, 1.6, 13.2, 0.18], [0.3, 2.2, 11.6, -0.06], [4.8, 1.2, 13.6, -0.2],
    [-2.6, -1.9, 14.8, 0.1], [2.9, -2.1, 15.2, -0.12], [7.6, -0.6, 12.4, -0.26],
  ];
  const ORDER = [3, 0, 4, 1, 5, 2]; // kolejność na scenie: lampa, kubek, but, kubek, butelka, kubek
  // kawałki okna składanego „ze światła”: prostokąt w pikselach okna, skąd leci (dx, dy, dz), obrót, opóźnienie
  const PIECES = [
    [0, 0, 1440, 72, 0, 4.5, 3, 0, 0],
    [0, 72, 47, 900, -6, 0, 2.5, 0.6, 0.3],
    [1160, 72, 1440, 900, 7, 0.5, 2.5, -0.6, 0.45],
    [47, 72, 1160, 280, 0, 5.5, 4.5, 0, 0.65],
    [47, 280, 1160, 900, 0, -6.5, 3.5, 0, 0.85],
  ];
  const ASSEMBLE = 8.4, FLIGHT = 1.9, SETTLED = ASSEMBLE + 0.85 + FLIGHT + 0.8;
  const thumbPos = new Float32Array(2);
  function thumbWorld(i) {
    // środek miniatury #i na liście plików (po wczytaniu, jeden wiersz paska narzędzi)
    thumbPos[0] = WX(52 + (i % 2) * 140 + 66); thumbPos[1] = WY(280 + 33 + Math.floor(i / 2) * 174 + 65);
  }
  function setPanel(i, cx, cy, cz, ax, ay, az, hw, bx, by, bz, hh, kind, layer, alpha, u0, v0, u1, v1, glow, light, sweep, emit) {
    const o = i * 20;
    panelU[o] = cx; panelU[o + 1] = cy; panelU[o + 2] = cz; panelU[o + 3] = kind;
    panelU[o + 4] = ax * hw; panelU[o + 5] = ay * hw; panelU[o + 6] = az * hw; panelU[o + 7] = layer;
    panelU[o + 8] = bx * hh; panelU[o + 9] = by * hh; panelU[o + 10] = bz * hh; panelU[o + 11] = alpha;
    panelU[o + 12] = u0; panelU[o + 13] = v0; panelU[o + 14] = u1; panelU[o + 15] = v1;
    panelU[o + 16] = glow; panelU[o + 17] = light; panelU[o + 18] = sweep; panelU[o + 19] = emit;
  }
  function sprite(n, r, cx, cy, cz, hw, hh, alpha, emit) {
    setPanel(n, cx, cy, cz, 1, 0, 0, hw, 0, 1, 0, hh, 2, 0, alpha, r[0] / ATW, r[1] / ATH, r[2] / ATW, r[3] / ATH, 0, 0, 0, emit);
  }
  let nPanels = 0, nOpaque = 0;
  function buildScene(t) {
    let n = 0;
    // okno AsOne: w trakcie składania 5 kawałków, potem jedna płyta (bez szwów)
    if (t < SETTLED && !flat.on) {
      for (let k = 0; k < PIECES.length; k++) {
        const q = PIECES[k], t0 = ASSEMBLE + q[8];
        if (t < t0) continue;
        const u = s5(t0, t0 + FLIGHT, t), w = 1 - u;
        const rot = q[7] * w, c = Math.cos(rot), sn = Math.sin(rot);
        const light = 1 - s5(t0 + FLIGHT - 0.5, t0 + FLIGHT + 0.8, t);
        setPanel(n++, WX((q[0] + q[2]) / 2) + q[4] * w * w, WY((q[1] + q[3]) / 2) + q[5] * w * w, q[6] * w * w,
          c, 0, -sn, (q[2] - q[0]) / 180, 0, 1, 0, (q[3] - q[1]) / 180, 0, 0, sstep(t0, t0 + 0.5, t),
          q[0] / UI_W, q[1] / UI_H, q[2] / UI_W, q[3] / UI_H, 0, light, 0, 1);
      }
    } else {
      setPanel(n++, 0, 0, 0, 1, 0, 0, 8, 0, 1, 0, 5, 0, uiFade, 1, 0, 0, 1, 1, 0, 0, S.sweep, 1 - 0.72 * S.splitA);
    }
    nOpaque = n;
    // pływające zdjęcia i lot kubków do listy plików
    for (let k = 0; k < 6 && !flat.on; k++) {
      const idx = ORDER[k], f = FLOAT[k];
      const bob = Math.sin(t * 0.6 + k * 1.7) * 0.12;
      const back = s5(8.0, 12.0, t);
      let x = (f[0] + Math.sin(t * 0.21 + k) * 0.25) * (1 + back * 0.45), y = f[1] + bob + back * 1.2, z = f[2] - back * 6.5, rot = f[3] + Math.sin(t * 0.3 + k) * 0.04;
      let hw = 1.6, hh = 1.2, alpha = sstep(0.0, 1.6, t - k * 0.25), glow = 0.9;
      if (idx < 3) {
        // kubki: lot do miniatury, zmniejszanie do 120×90 px; plik pojawia się na liście, gdy dotrze
        const start = 11.9 + idx * 0.6, end = start + 1.5;
        const u = s5(start, end, t);
        thumbWorld(idx);
        const arc = Math.sin(u * Math.PI) * 1.4;
        x = lerp(x, thumbPos[0], u); y = lerp(y, thumbPos[1], u) + arc; z = lerp(z, 0.05, u);
        hw = lerp(1.6, 60 / 90, u); hh = lerp(1.2, 45 / 90, u); rot = lerp(rot, 0, u);
        alpha *= 1 - sstep(end - 0.05, end + 0.25, t); glow = lerp(0.9, 2.5, Math.sin(u * Math.PI));
      } else {
        // lampa, but, butelka: odpływają w ciemność, gdy pojawia się okno
        const u = s5(9.0, 13.0, t);
        z += u * -6; x += (x < 0 ? -1 : 1) * u * 6; alpha *= 1 - u;
      }
      if (alpha < 0.004) continue;
      const c = Math.cos(rot), sn = Math.sin(rot);
      setPanel(n++, x, y, z, c, 0, -sn, hw, 0, 1, 0, hh, 1, idx, alpha, 0, 0, 1, 1, glow * (1 - sstep(10, 14, t) * 0.6), 0, 0, 1);
    }
    // kursor myszy i fala kliknięcia (nad oknem)
    if (S.curA > 0.003) {
      const cx = WX(S.curX), cy = WY(S.curY);
      if (S.click > 0) { const r = 0.16 + (1 - S.click) * 0.34; sprite(n++, A.ring, cx, cy, 0.03, r, r, S.click * S.curA, 1.6); }
      sprite(n++, A.cursor, cx + 0.1, cy - 0.15, 0.04, 0.1, 0.15, S.curA, 1.0);
    }
    // pliki JPG wylatujące do „web/” (ujęcie 7)
    const fo = S.flyOut;
    const fold = FOLDER;
    if (fo > 0 && t < 70.5) {
      for (let i = 0; i < 3; i++) {
        const u = s5(i * 0.12, 0.62 + i * 0.12, fo);
        if (u <= 0 || u >= 1) continue;
        const sx = WX(1343), sy = WY(76), arc = Math.sin(u * Math.PI) * 1.6;
        const x = lerp(sx, fold[0] - 0.4 + i * 0.4, u), y = lerp(sy, fold[1] + 0.3, u) + arc, z = lerp(0.3, fold[2] + 0.4, u);
        const sc = lerp(0.35, 1.0, Math.sin(u * Math.PI)) * 0.9;
        sprite(n++, A.file[i], x, y, z, 2.0 * sc, 1.5 * sc, sstep(0, 0.1, u) * (1 - sstep(0.85, 1, u)), 1.15);
      }
    }
    const folderA = sstep(60.9, 61.8, t) * (1 - sstep(70.0, 71.5, t));
    if (folderA > 0) {
      const r = A.folder, pulse = 1 + Math.exp(-Math.abs(t - 64.2) * 3) * 0.08;
      setPanel(n++, fold[0], fold[1], fold[2], 0.96, 0, -0.28, 2.5 * pulse, 0, 1, 0, 1.7 * pulse, 2, 0, folderA, r[0] / ATW, r[1] / ATH, r[2] / ATW, r[3] / ATH, 0, 0, 0, 1.25);
    }
    // przed / po (ujęcie 8): panel z podziałem; surowe po lewej, gotowe po prawej
    if (S.splitA > 0.003) {
      const cx = 0, cy = 0.1, cz = 4.2, hw = 3.6, hh = 3.6, sp = lerp(-1, 1, 0.15 + S.split * 0.7);
      const xl = cx - hw, xs = cx + hw * sp;
      const ul = (sp + 1) / 2;
      // obie połówki pokazują ten sam obszar zdjęcia: wynik 835×835 (kadr + marginesy)
      const c = REAL.crop, m = REAL.frame, ox = c[0] - m[2], oy = c[1] - m[0];
      setPanel(n++, (xl + xs) / 2, cy, cz, 1, 0, 0, (xs - xl) / 2, 0, 1, 0, hh, 1, 0, S.splitA, ox / 1600, oy / 1200, (ox + 835 * ul) / 1600, (oy + 835) / 1200, 0.6, 0, 0, 1);
      const u0 = (ox + 835 * ul) / 1600, u1 = (ox + 835) / 1600, v0 = oy / 1200, v1 = (oy + 835) / 1200;
      setPanel(n++, (xs + cx + hw) / 2, cy, cz, 1, 0, 0, (cx + hw - xs) / 2, 0, 1, 0, hh, 3, 0, S.splitA, u0, v0, u1, v1, 0.6, 0, 0, 1);
      setPanel(n++, xs, cy, cz + 0.01, 1, 0, 0, 0.018, 0, 1, 0, hh, 2, 0, S.splitA, A.white[0], A.white[1], A.white[0], A.white[1], 0, 0, 0, 6);
      sprite(n++, A.before, xl + 1.2, cy + hh - 0.55, cz + 0.02, 0.95, 0.3, S.splitA, 1);
      sprite(n++, A.after, cx + hw - 1.2, cy + hh - 0.55, cz + 0.02, 0.95, 0.3, S.splitA, 1);
    }
    nPanels = n;
    // zdjęcie na płótnie programu (liczone w shaderze okna)
    const top = S.nameLen > 0 ? 97 : 72;
    const k = S.view, i0 = Math.floor(k), f = k - i0;
    const a = viewStates[Math.min(i0, 3)], b = viewStates[Math.min(i0 + 1, 3)];
    for (let j = 0; j < 4; j++) { canvasU[4 + j] = lerp(a.view[j], b.view[j], f); canvasU[8 + j] = lerp(a.crop[j], b.crop[j], f); canvasU[12 + j] = lerp(a.m[j], b.m[j], f); }
    const rw = canvasU[6] - canvasU[4], rh = canvasU[7] - canvasU[5];
    // skala jak w programie: 46 % (dopasowanie pierwszego zdjęcia) aż do Ctrl+0, potem dopasowanie do płótna
    const aw = 790, ah = UI_H - 29 - top - 33;
    const fit = Math.min(aw * 0.935 / rw, ah * 0.935 / rh), z46 = aw * 0.935 / 1600;
    const zoom = lerp(z46, fit, s5(48.6, 49.4, t));
    const fitK = s5(48.6, 49.4, t), pw = rw * zoom, ph = rh * zoom;
    const cxl0 = 364 + aw / 2, cyl0 = top + 33 + ah / 2;
    // do Ctrl+0 wynik stoi tam, gdzie kadr był w pełnym zdjęciu (jak w programie), potem jest wyśrodkowany
    const ax0 = cxl0 - 800 * z46 + (canvasU[8] - canvasU[14]) * z46, ay0 = cyl0 - 600 * z46 + (canvasU[9] - canvasU[12]) * z46;
    const x0 = lerp(ax0, cxl0 - pw / 2, fitK), y0 = lerp(ay0, cyl0 - ph / 2, fitK);
    const cxl = x0 + pw / 2, cyl = y0 + ph / 2;
    canvasU[0] = x0; canvasU[1] = y0; canvasU[2] = x0 + pw; canvasU[3] = y0 + ph;
    canvasU[16] = S.active; canvasU[17] = S.removal; canvasU[18] = S.inspect; canvasU[19] = S.files > 0 ? 1 - S.splitA : 0;
    canvasU[20] = Math.exp(-Math.abs(t - 43.6) * 4) + Math.exp(-Math.abs(t - 46.0) * 4) + Math.exp(-Math.abs(t - 47.9) * 4);
    canvasU[21] = 743; canvasU[22] = 624; canvasU[23] = 0;
    // cząsteczki: ten sam prostokąt co zdjęcie
    partsU[0] = WX(cxl); partsU[1] = WY(cyl); partsU[2] = 0.03; partsU[3] = S.removal;
    partsU[4] = pw / 180; partsU[5] = 0; partsU[6] = 0; partsU[7] = PARTICLES;
    partsU[8] = 0; partsU[9] = ph / 180; partsU[10] = 0; partsU[11] = (S.removal > 0 && S.removal < 1) ? 1 : 0;
    partsU[12] = 0; partsU[13] = 0; partsU[14] = 1600; partsU[15] = 1200;
  }
  const FOLDER = [10.6, 2.0, 1.6];
  const PARTICLES = 90000;
  const c0 = REAL.crop;
  const viewStates = [
    { view: [0, 0, 1600, 1200], crop: [0, 0, 1600, 1200], m: [0, 0, 0, 0] },
    { view: [0, 0, 779, 825], crop: c0, m: [0, 0, 0, 0] },
    { view: [0, 0, 825, 825], crop: c0, m: [0, 0, 23, 23] },
    { view: [0, 0, 835, 835], crop: c0, m: [5, 5, 28, 28] },
  ];

  // --- pętla ------------------------------------------------------------------------------------
  const perf = { frames: 0, cpu: [], gpu: [], on: Q.has('perf') };
  function resize(w, h) {
    canvas.width = w; canvas.height = h;
    RT = makeTargets(w, h);
  }
  let dpr = Math.min(window.devicePixelRatio || 1, 2);
  if (!Q.has('record')) resize(Math.round(innerWidth * dpr), Math.round(innerHeight * dpr));
  window.addEventListener('resize', () => { if (!Q.has('record')) { dpr = Math.min(window.devicePixelRatio || 1, 2); resize(Math.round(innerWidth * dpr), Math.round(innerHeight * dpr)); } });

  let aperture = 0.01;
  // płaskie ujęcie do porównań z prawdziwym programem: okno na wprost, bez efektów, 1440×900
  const flat = { on: false, light: false };
  function updateCamera(t, dt) {
    if (flat.on) { eye[0] = 0; eye[1] = 0; eye[2] = 20; at[0] = 0; at[1] = 0; at[2] = 0; aperture = 0; return; }
    if (!free.on) {
      aperture = hermite(eye, CAM, t, 1);
      hermite(at, CAM, t, 2);
      // kadr pionowy: kamera cofa się wzdłuż osi widzenia, żeby temat zmieścił się w wąskim kadrze
      if (RT && RT.h > RT.w) for (let c = 0; c < 3; c++) eye[c] = at[c] + (eye[c] - at[c]) * 1.65;
    } else {
      const sp = (free.keys.has('ShiftLeft') || free.keys.has('ShiftRight') ? 4 : 1) * free.speed * dt;
      const fx = Math.sin(free.yaw) * Math.cos(free.pitch), fy = Math.sin(free.pitch), fz = Math.cos(free.yaw) * Math.cos(free.pitch);
      const rx = -Math.cos(free.yaw), rz = Math.sin(free.yaw);
      const k = free.keys;
      const mv = (a, b) => (k.has(a) ? 1 : 0) - (k.has(b) ? 1 : 0);
      const f = mv('KeyW', 'KeyS'), r = mv('KeyD', 'KeyA'), u = mv('KeyE', 'KeyQ');
      free.pos[0] += (fx * f + rx * r) * sp; free.pos[1] += (fy * f) * sp + u * sp; free.pos[2] += (fz * f + rz * r) * sp;
      // świat ograniczony (krawędzi nie widać – mgła i ciemność)
      const d = Math.hypot(free.pos[0], free.pos[1], free.pos[2]);
      if (d > 42) { const s = 42 / d; free.pos[0] *= s; free.pos[1] *= s; free.pos[2] *= s; }
      eye.set(free.pos); at[0] = eye[0] + fx; at[1] = eye[1] + fy; at[2] = eye[2] + fz;
      aperture = 0.008;
    }
  }
  function frameUniforms(t, w, h) {
    const aspect = w / h;
    // stały kąt poziomy (pion 9:16 pokazuje to samo w szerz, więcej w górę i w dół)
    // poziomo: stały kąt jak w 16:9; pionowo (9:16): szerszy kąt pionowy, temat w środku kadru
    const vfov = flat.on ? 2 * Math.atan(5 / 20) : aspect >= 1 ? 2 * Math.atan(Math.tan(0.31) * (16 / 9) / aspect) : 2 * Math.atan(Math.tan(0.31) * 1.6);
    perspective(proj, vfov, aspect, 0.05, 400);
    if (!flat.on) proj[9] = -0.14 * (1 - S.endDim);
    lookAt(view, eye, at, right, up);
    if (havePrev) pvp.set(vp);
    mul4(vp, proj, view);
    if (!havePrev) { pvp.set(vp); havePrev = true; }
    invert4(ivp, vp);
    frameU.set(vp, 0); frameU[16] = eye[0]; frameU[17] = eye[1]; frameU[18] = eye[2]; frameU[19] = t;
    frameU[20] = right[0]; frameU[21] = right[1]; frameU[22] = right[2]; frameU[23] = aspect;
    frameU[24] = up[0]; frameU[25] = up[1]; frameU[26] = up[2]; frameU[27] = 1;
    device.queue.writeBuffer(frameBuf, 0, frameU);
    bgU.set(ivp, 0); bgU[16] = 1 - S.endDim * 0.6; bgU[17] = 1; device.queue.writeBuffer(bgBuf, 0, bgU);
    // plansza końcowa: ostrość przeskakuje przed okno, okno rozmywa się w tło
    const focus = lerp(Math.hypot(at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]), 3, S.endDim);
    postU.set(ivp, 0); postU.set(pvp, 16);
    postU[32] = eye[0]; postU[33] = eye[1]; postU[34] = eye[2]; postU[35] = focus;
    postU[36] = w; postU[37] = h; postU[38] = lerp(aperture, 0.05, S.endDim) * 0.6; postU[39] = 22 * (h / 1440);
    postU[40] = 0.7; postU[41] = 1.0; postU[42] = 0.008; postU[43] = t;
    if (flat.on) { postU[40] = 0; postU[42] = 0; }
    postU[44] = flat.on ? 0 : 0.55; postU[45] = sstep(0, 0.8, t) * (1 - sstep(84.2, 85, t) * (free.on ? 0 : 1)) * (1 - S.endDim * 0.93); postU[46] = flat.on ? 0 : 0.25; postU[47] = 1;
    device.queue.writeBuffer(postBuf, 0, postU);
  }
  function pass(enc, desc, pipe, bg, extra) {
    const q = enc.beginRenderPass(desc); q.setPipeline(pipe); q.setBindGroup(0, bg); if (extra) q.setBindGroup(1, extra); q.draw(3); q.end();
  }
  function renderFrame(t, target) {
    evalState(t);
    // UI: przerysowanie tylko przy zmianie stanu
    const q0 = performance.now();
    updateUi(t);
    const q1 = performance.now();
    const ok = (S.caption >= 0 ? S.caption : 99) + (S.endA > 0 ? 100 : 0);
    if (ok !== RT.overKey) layoutOverlay(RT, ok);
    overU[40] = flat.on ? 0 : S.captionA; overU[41] = flat.on ? 0 : S.endA; device.queue.writeBuffer(overBuf, 0, overU);
    if (flat.on) { S.curA = 0; S.splitA = 0; S.sweep = flat.light ? 1 : 0; S.flyOut = 0; }
    buildScene(t);
    device.queue.writeBuffer(panelBuf, 0, panelU, 0, nPanels * 20);
    device.queue.writeBuffer(canvasBuf, 0, canvasU);
    device.queue.writeBuffer(partsBuf, 0, partsU);
    frameUniforms(t, RT.w, RT.h);
    postU[47] = Math.max(S.captionA, S.endA); device.queue.writeBuffer(postBuf, 188, postU, 47, 1);
    const enc = device.createCommandEncoder();
    const sd = RT.sceneDesc;
    if (ts) sd.timestampWrites = ts.begin;
    const p = enc.beginRenderPass(sd);
    p.setBindGroup(0, bg0);
    p.setPipeline(pipeBg); p.setBindGroup(1, bgBgG); p.draw(3);
    p.setPipeline(pipePanel); p.setBindGroup(1, bgPanels[uiCur * 3 + uiPrev]); p.draw(6, nOpaque, 0, 0);
    p.setPipeline(pipePanelT); p.setBindGroup(1, bgPanels[uiCur * 3 + uiPrev]); if (nPanels > nOpaque) p.draw(6, nPanels - nOpaque, 0, nOpaque);
    if (partsU[11] > 0) { p.setPipeline(pipeParts); p.setBindGroup(1, bgParts); p.draw(6, PARTICLES); }
    p.end();
    pass(enc, RT.dBright, pipeBright, RT.bgBright);
    pass(enc, RT.dBlurH, pipeBlur, RT.bgBlurH, blurBGs[0]);
    pass(enc, RT.dBlurV, pipeBlur, RT.bgBlurV, blurBGs[1]);
    pass(enc, RT.dDof, pipeDof, RT.bgDof);
    RT.finalAtt[0].view = target;
    const fd = RT.dFinal;
    if (ts) fd.timestampWrites = ts.end;
    pass(enc, fd, pipeFinal, RT.bgFinal, bgOver);
    if (ts && !ts.busy) { enc.resolveQuerySet(ts.set, 0, 2, ts.res, 0); enc.copyBufferToBuffer(ts.res, 0, ts.read, 0, 16); }
    device.queue.submit([enc.finish()]);
    const q2 = performance.now();
    stepBuild(BUDGET);
    if (perfDbg) { const q3 = performance.now(); if (q3 - q0 > 6) perfDbg.push([t.toFixed(2), (q1 - q0).toFixed(1), (q2 - q1).toFixed(1), (q3 - q2).toFixed(1), curEv, build.ev, build.n]); }
    if (ts && !ts.busy) {
      ts.busy = true; ts.t = t;
      ts.read.mapAsync(GPUMapMode.READ).then(ts.done);
    }
  }
  function setBlur(w, h) {
    blurU[0] = 4 / w; blurU[1] = 0; device.queue.writeBuffer(blurBufs[0], 0, blurU);
    blurU[0] = 0; blurU[1] = 4 / h; device.queue.writeBuffer(blurBufs[1], 0, blurU);
  }
  if (RT) setBlur(RT.w, RT.h);

  // --- HUD i sterowanie -------------------------------------------------------------------------
  const hud = document.getElementById('hud');
  function hudRender() {
    const T = STR[LANG];
    hud.innerHTML = '';
    const b = (label, fn) => { const e = document.createElement('button'); e.textContent = label; e.onclick = fn; hud.append(e); };
    b(LANG === 'pl' ? 'EN' : 'PL', () => setLang(LANG === 'pl' ? 'en' : 'pl'));
    b(T.hudPlay, () => { free.on = false; playing = true; filmT = 0; uiT = 0; havePrev = false; });
    b(T.hudFree, () => toggleFree());
    for (let i = 0; i < 8; i++) b(`${i + 1}`, () => replayStep(i));
    if (free.on) { const s = document.createElement('span'); s.textContent = ' ' + T.freeHelp; hud.append(s); }
  }
  const STEP_T = [0, 8, 18.4, 28.2, 42.4, 52.4, 60.4, 70];
  function replayStep(i) { uiT = STEP_T[i]; if (!free.on) { filmT = STEP_T[i]; havePrev = false; } }
  function setLang(l) {
    LANG = l; for (const u of uis) u.lang = l; drawAtlas(); buildAllUi(); if (RT) RT.overKey = -2; hudRender();
  }
  function toggleFree() {
    free.on = !free.on;
    if (free.on) { free.pos.set(eye); const dx = at[0] - eye[0], dy = at[1] - eye[1], dz = at[2] - eye[2]; free.yaw = Math.atan2(dx, dz); free.pitch = Math.atan2(dy, Math.hypot(dx, dz)); uiT = filmT; }
    else { filmT = uiT = 0; playing = true; havePrev = false; }
    hudRender();
  }
  window.addEventListener('keydown', (e) => { if (e.code === 'KeyF') toggleFree(); else free.keys.add(e.code); });
  window.addEventListener('keyup', (e) => free.keys.delete(e.code));
  canvas.addEventListener('mousemove', (e) => {
    if (!free.on || document.pointerLockElement !== canvas) return;
    free.yaw -= e.movementX * 0.0022; free.pitch = clamp(free.pitch - e.movementY * 0.0022, -1.4, 1.4);
  });
  // kliknięcie w trybie swobodnym: promień w płaszczyznę okna → przycisk AsOne → powtórka kroku
  const BUTTONS = [[1172, 300, 1305, 326, 3], [1241, 467, 1325, 493, 4], [1172, 665, 1281, 691, 4], [1172, 731, 1252, 757, 4],
    [52, 331, 324, 669, 5], [1259, 64, 1428, 89, 6], [1158, 5, 1183, 27, 7], [88, 40, 297, 60, 2]];
  canvas.addEventListener('mousedown', () => {
    if (!free.on) return;
    if (document.pointerLockElement !== canvas) { canvas.requestPointerLock(); return; }
    const dx = at[0] - eye[0], dy = at[1] - eye[1], dz = at[2] - eye[2];
    if (Math.abs(dz) < 1e-4) return;
    const k = -eye[2] / dz; if (k <= 0) return;
    const x = eye[0] + dx * k, y = eye[1] + dy * k;
    const lx = (x + 8) * 90, ly = (5 - y) * 90;
    for (const b of BUTTONS) if (lx >= b[0] && lx <= b[2] && ly >= b[1] && ly <= b[3]) { replayStep(b[4]); return; }
  });
  hudRender();

  // --- API do nagrywania i pomiarów (sterowane z zewnątrz) --------------------------------------
  window.__promo = {
    resize: (w, h) => { resize(w, h); setBlur(w, h); havePrev = false; },
    renderAt: (t, prevT) => { updateCamera(prevT ?? t - 1 / 60, 1 / 60); evalState(prevT ?? t - 1 / 60); frameUniforms(prevT ?? t - 1 / 60, RT.w, RT.h); updateCamera(t, 1 / 60); renderFrame(t, RT.views.out); },
    readPixels: async (swap = true) => {
      const bpr = Math.ceil(RT.w * 4 / 256) * 256;
      const buf = device.createBuffer({ size: bpr * RT.h, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
      const enc = device.createCommandEncoder();
      enc.copyTextureToBuffer({ texture: RT.out }, { buffer: buf, bytesPerRow: bpr }, [RT.w, RT.h]);
      device.queue.submit([enc.finish()]);
      await buf.mapAsync(GPUMapMode.READ);
      const src = new Uint8Array(buf.getMappedRange()), out = new Uint8Array(RT.w * RT.h * 4);
      for (let y = 0; y < RT.h; y++) out.set(src.subarray(y * bpr, y * bpr + RT.w * 4), y * RT.w * 4);
      buf.unmap(); buf.destroy();
      if (swap && outFmt === 'bgra8unorm') for (let i = 0; i < out.length; i += 4) { const r = out[i]; out[i] = out[i + 2]; out[i + 2] = r; }
      return out;
    },
    gpuLog: () => (ts ? ts.log : []),
    camPos: () => Array.from(eye), state: () => ({ free: free.on, playing, filmT, uiT, lang: LANG, hud: hud.textContent }),
    perfDbg: () => perfDbg, events: () => EV.map((e) => +e.t.toFixed(3)),
    flat: (t, light) => { flat.on = true; flat.light = !!light; havePrev = false; window.__promo.renderAt(t, t); flat.on = false; },
    ui: (t, k) => {
      if (!dbgUi[k]) { dbgUi[k] = new AsOneUI(k ? 'light' : 'dark', LANG, thumbs); dbgUi[k].fileNames = REAL.files; }
      const u = dbgUi[k]; u.lang = LANG; evalState(t); fillState(u.st); u.draw(); return u.canvas;
    },
    setLang, device, S,
  };

  function tick(now) {
    const dt = lastNow ? Math.min((now - lastNow) / 1000, 0.1) : 1 / 60;
    lastNow = now;
    if (playing && !free.on) { filmT += dt; uiT = filmT; if (filmT > FILM_LEN) { filmT = FILM_LEN; playing = false; toggleFree(); } }
    else if (free.on) uiT = Math.min(uiT + dt, uiT < 70 ? uiT + dt : FILM_LEN);
    updateCamera(free.on ? uiT : filmT, dt);
    const tt = free.on ? uiT : filmT;
    // błąd jednej klatki nie zatrzymuje pętli
    try { renderFrame(tt, ctx.getCurrentTexture().createView()); } catch (e) { console.error(e); }
    requestAnimationFrame(tick);
  }
  window.done = true;
  if (!Q.has('record')) requestAnimationFrame(tick);
}
main().catch((e) => { console.error(e); msg.textContent = String(e); });
