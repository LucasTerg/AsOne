// Okno AsOne rysowane w Canvas2D (tekstura dla WebGPU). Współrzędne logiczne jak w prawdziwym programie
// przy oknie 1440×900 (zrzuty referencyjne), skala `UI_S` daje ostrość z bliska.
const UI_W = 1440, UI_H = 900, UI_S = 2.5;
const PAL = {
  dark: { chrome: '#262626', canvas: '#1E1E1E', dock: '#1E1E1E', card: '#262626', cardBorder: '#1A1A1A', field: '#383838',
    fieldBorder: '#464646', hover: '#424242', pressed: '#4A4A4A', text: '#D6D6D6', dim: '#969696', faint: '#707070', icon: '#C8C8C8',
    accent: '#2680EB', onAccent: '#FFFFFF', sep: '#1A1A1A', tabStrip: '#2B2B2B', rowSel: '#3A3A3A', danger: '#F06A5A', warning: '#E8B046',
    ok: '#6CC67A', r: 3, btnBorder: null },
  light: { chrome: '#F0F0F0', canvas: '#D6D6D6', dock: '#E2E2E2', card: '#F0F0F0', cardBorder: '#D6D6D6', field: '#FFFFFF',
    fieldBorder: '#BABABA', hover: '#E2E2E2', pressed: '#CECECE', text: '#141414', dim: '#404040', faint: '#787878', icon: '#2E2E2E',
    accent: '#378EF0', onAccent: '#FFFFFF', sep: '#D6D6D6', tabStrip: '#E2E2E2', rowSel: '#CCCCCC', danger: '#EC5B62', warning: '#E8B046',
    ok: '#2E8B3E', r: 4, btnBorder: '#BABABA' },
};
const FONT = '"Ubuntu Light", Ubuntu, "Segoe UI Semilight", "Segoe UI", system-ui, sans-serif';

// Wymiary i liczby z prawdziwego przebiegu (uzupełniane z referencji w main.js – REAL).
class AsOneUI {
  constructor(theme, lang, photos, soft = false) {
    this.canvas = document.createElement('canvas');
    this.canvas.width = UI_W * UI_S;
    this.canvas.height = UI_H * UI_S;
    this.g = this.canvas.getContext('2d', { alpha: true, willReadFrequently: soft });
    this.theme = theme;
    this.lang = lang;
    this.photos = photos; // ImageBitmap surowych zdjęć (miniatury)
    this.st = {
      files: 0, name: '', slug: '', active: 0, bg: false, crop: false, square: false, frame: false, ver2: false,
      menu: 0, menuHover: -1, verHover: -1, inspect: false, dialog: false, pressed: '', thumbHover: -1, dims: null,
    };
    this.fontCache = new Map();
    // kalibracja czcionki systemowej: zdanie z ekranu startowego ma w programie 423 px szerokości
    this.g.font = `300 13px ${FONT}`;
    this.fk = 423 / this.g.measureText('Product photos for the web: crop, margins, white background, file names.').width;
  }
  font(px, weight = 300) {
    const key = px * 1000 + weight;
    let f = this.fontCache.get(key);
    if (!f) { f = `${weight} ${(px * this.fk).toFixed(2)}px ${FONT}`; this.fontCache.set(key, f); }
    return f;
  }
  get T() { return STR[this.lang]; }
  get P() { return PAL[this.theme]; }

  // --- prymitywy ----------------------------------------------------------------------------------
  rect(x, y, w, h, c) { this.g.fillStyle = c; this.g.fillRect(x, y, w, h); }
  rr(x, y, w, h, r, fill, stroke) {
    const g = this.g;
    g.beginPath(); g.roundRect(x, y, w, h, r);
    if (fill) { g.fillStyle = fill; g.fill(); }
    if (stroke) { g.strokeStyle = stroke; g.lineWidth = 1; g.stroke(); }
  }
  text(s, x, y, c, px = 13, align = 'left', weight = 300) {
    const g = this.g;
    g.font = this.font(px, weight); g.fillStyle = c; g.textAlign = align; g.textBaseline = 'middle';
    g.fillText(s, x, y);
    // egui w jasnym motywie rysuje ciemny tekst wyraźnie grubiej niż cienki Ubuntu Light w przeglądarce
    if (this.theme === 'light') { g.strokeStyle = c; g.lineWidth = 0.32; g.strokeText(s, x, y); }
    return g.measureText(s).width;
  }
  wrap(s, maxW, px = 13) {
    const out = []; let line = '';
    for (const w of s.split(' ')) { const t = line ? line + ' ' + w : w; if (line && this.tw(t, px) > maxW) { out.push(line); line = w; } else line = t; }
    if (line) out.push(line);
    return out;
  }
  tw(s, px = 13, weight = 300) { this.g.font = this.font(px, weight); return this.g.measureText(s).width; }
  // przycisk egui: wypełnienie pola, w jasnym motywie ramka; zwraca szerokość
  button(label, x, y, { w = 0, h = 22, accent = false, disabled = false, selected = false, px = 13, pressed = false, flat = false } = {}) {
    const P = this.P;
    const width = w || Math.round(this.tw(label, px) + 16);
    let fill = P.field, fg = P.text, stroke = P.btnBorder;
    if (accent || selected) { fill = P.accent; fg = P.onAccent; stroke = null; }
    if (pressed) fill = accent ? '#4A9AF5' : P.pressed;
    if (disabled) { fg = P.faint; if (accent) { fill = P.accent + '80'; fg = '#FFFFFFAA'; } }
    if (flat) { fill = null; stroke = null; }
    this.rr(x + 0.5, y + 0.5, width - 1, h - 1, P.r, fill, stroke);
    this.text(label, x + width / 2, y + h / 2 + 0.5, fg, px, 'center');
    return width;
  }
  checkbox(label, x, y, checked, px = 13) {
    const P = this.P;
    this.rr(x + 0.5, y + 3.5, 13, 13, 2, P.field, P.btnBorder);
    if (checked) {
      const g = this.g; g.strokeStyle = P.text; g.lineWidth = 1.6; g.beginPath();
      g.moveTo(x + 3, y + 10); g.lineTo(x + 6, y + 13.5); g.lineTo(x + 11, y + 6); g.stroke();
    }
    return 18 + this.text(label, x + 18, y + 10.5, P.text, px);
  }
  icon(kind, cx, cy, c, s = 1) {
    // ikony narzędzi rysowane wektorowo (jak SVG w programie)
    const g = this.g; g.save(); g.translate(cx, cy); g.scale(s, s); g.strokeStyle = c; g.fillStyle = c; g.lineWidth = 1.3 / s; g.lineCap = 'round'; g.lineJoin = 'round';
    g.beginPath();
    if (kind === 'move') {
      g.moveTo(-8, 0); g.lineTo(8, 0); g.moveTo(0, -8); g.lineTo(0, 8);
      for (const [dx, dy] of [[-8, 0], [8, 0], [0, -8], [0, 8]]) {
        const px = dy ? 3 : 0, py = dx ? 3 : 0, bx = dx ? -Math.sign(dx) * 3 : 0, by = dy ? -Math.sign(dy) * 3 : 0;
        g.moveTo(dx + bx + px, dy + by + py); g.lineTo(dx, dy); g.lineTo(dx + bx - px, dy + by - py);
      }
    } else if (kind === 'crop') {
      g.moveTo(-5, -9); g.lineTo(-5, 5); g.lineTo(9, 5); g.moveTo(-9, -5); g.lineTo(5, -5); g.lineTo(5, 9);
    } else if (kind === 'eraser') {
      g.moveTo(-8, 3); g.lineTo(1, -6); g.lineTo(7, 0); g.lineTo(0, 7); g.lineTo(-4, 7); g.closePath(); g.moveTo(-3, -2); g.lineTo(4, 5); g.moveTo(-4, 7); g.lineTo(8, 7);
    } else if (kind === 'rect') {
      g.setLineDash([3, 2.5]); g.roundRect(-8, -7, 16, 14, 2); g.stroke(); g.setLineDash([]); g.beginPath();
    } else if (kind === 'undo' || kind === 'redo') {
      if (kind === 'redo') g.scale(-1, 1);
      g.moveTo(-6, -4); g.lineTo(3, -4); g.arc(3, 1.5, 5.5, -Math.PI / 2, Math.PI / 2); g.lineTo(-3, 7);
      g.moveTo(-3, -7); g.lineTo(-6, -4); g.lineTo(-3, -1);
    } else if (kind === 'folder') {
      g.moveTo(-7, -5); g.lineTo(-2, -5); g.lineTo(0, -3); g.lineTo(7, -3); g.lineTo(7, 5); g.lineTo(-7, 5); g.closePath();

    } else if (kind === 'search') {
      g.arc(-1, -1, 4.5, 0, Math.PI * 2); g.moveTo(2.5, 2.5); g.lineTo(6, 6);
    } else if (kind === 'sun') {
      for (let i = 0; i < 8; i++) { const a = i * Math.PI / 4; g.moveTo(Math.cos(a) * 1.2, Math.sin(a) * 1.2); g.lineTo(Math.cos(a) * 6, Math.sin(a) * 6); }
    } else if (kind === 'moon') {
      g.arc(0, 0, 6, 0.9, Math.PI * 2 - 0.9); g.arc(2.6, 0, 5.2, Math.PI * 2 - 1.15, 1.15, true); g.closePath(); g.fill(); g.beginPath();
    } else if (kind === 'gear') {
      // zębatka: wieniec z 8 zębami i otwór
      g.beginPath();
      for (let i = 0; i < 16; i++) {
        const a0 = (i - 0.5) * Math.PI / 8, a1 = (i + 0.5) * Math.PI / 8, r = i % 2 ? 4.2 : 5.8;
        g.arc(0, 0, r, a0, a1);
      }
      g.closePath(); g.fill(); g.beginPath(); g.fillStyle = this.P.field; g.arc(0, 0, 1.9, 0, Math.PI * 2); g.fill(); g.beginPath();
    } else if (kind === 'link') {
      g.arc(-2, 2, 3, 0, Math.PI * 2); g.moveTo(2, -2); g.arc(2, -2, 3, 0, Math.PI * 2);
    }
    g.stroke(); g.restore();
  }
  a1(x, y, size = 16) {
    // znak „A1”
    this.rr(x, y, size, size, 3, this.P.accent);
    this.text('A1', x + size / 2, y + size / 2 + 0.5, '#FFFFFF', 9.5, 'center', 500);
  }

  // --- całe okno ----------------------------------------------------------------------------------
  draw() {
    const g = this.g, P = this.P, T = this.T, s = this.st;
    g.setTransform(UI_S, 0, 0, UI_S, 0, 0);
    g.clearRect(0, 0, UI_W, UI_H);
    this.rect(0, 0, UI_W, UI_H, P.dock);
    const named = s.name.length > 0;
    const top = named ? 97 : 72; // pasek narzędzi w dwóch wierszach, gdy jest nazwa (podgląd nazwy pliku)
    this.titleBar(s);
    this.toolbar(s, named);
    this.toolStrip(top);
    if (s.files === 0) this.startScreen(top);
    else { this.leftDock(top); this.center(top); this.inspector(top); }
    if (s.menu) this.contextMenu(top);
    if (s.dialog) this.exportDialog();
    // obramowanie okna
    this.rect(0, 0, 1, UI_H, this.theme === 'dark' ? '#1A1A1A' : '#B8B8B8'); this.rect(UI_W - 1, 0, 1, UI_H, this.theme === 'dark' ? '#1A1A1A' : '#B8B8B8');
  }
  titleBar(s) {
    const P = this.P, T = this.T;
    this.rect(0, 0, UI_W, 32, P.chrome);
    this.rect(0, 30, UI_W, 3, this.theme === 'dark' ? '#161616' : P.sep);
    this.a1(7, 7, 18);
    let mx = 41;
    for (let i = 0; i < 5; i++) mx += this.text(T.menus[i], mx, 16, P.text, 13) + 14;
    const title = s.files ? `AsOne — ${s.titleFile}` : (this.lang === 'pl' ? 'AsOne — zdjęcia produktowe do internetu' : 'AsOne — product photos for the web');
    this.text(title, 720, 16, P.dim, 13, 'center');
    this.button('', 1121, 5, { w: 30, h: 23 }); this.icon('search', 1135.5, 16.5, P.icon, 0.9);
    this.button('', 1155, 5, { w: 30, h: 23 }); this.icon(this.theme === 'dark' ? 'sun' : 'moon', 1170, 16.5, P.icon, 0.9);
    this.button('−', 1191, 5, { w: 25, h: 23 });
    this.text('100 %', 1245, 16.5, P.text, 13, 'center');
    this.button('+', 1274, 5, { w: 24, h: 23 });
    const g = this.g; g.strokeStyle = P.icon; g.lineWidth = 1.1;
    g.beginPath(); g.moveTo(1322, 16.5); g.lineTo(1334, 16.5); g.stroke();
    g.strokeRect(1369.5, 10.5, 11, 11);
    g.beginPath(); g.moveTo(1415, 11); g.lineTo(1426, 22); g.moveTo(1426, 11); g.lineTo(1415, 22); g.stroke();
  }
  toolbar(s, named) {
    const P = this.P, T = this.T;
    const h = named ? 65 : 40;
    this.rect(0, 32, UI_W, h, P.card);
    this.rect(0, 32 + h, UI_W, 1, P.sep);
    let x = 11;
    x += this.text(T.pasteName, x, 50, P.text, 13) + 7;
    const fx = x;
    this.rr(fx + 0.5, 40.5, 208, 19, P.r, P.field, this.theme === 'dark' ? null : P.fieldBorder);
    if (named) this.text(s.name, fx + 4, 50.5, P.text, 13); else this.text(T.placeholder, fx + 4, 50.5, P.faint, 13);
    if (s.caret) this.rect(fx + 4 + this.tw(s.name, 13) + 1, 43, 1, 15, P.text);
    x = fx + 215; x += this.button(T.paste, x, 39, { pressed: s.pressed === 'paste' }) + 7;
    if (named) { x += this.button('×', x, 39, { w: 22, px: 12 }) + 7; }
    x += this.text(T.startNo, x, 50, P.text, 13) + 6;
    this.rr(x + 0.5, 40.5, 39, 19, P.r, P.field, this.theme === 'dark' ? null : P.fieldBorder);
    this.text('1', x + 20, 50.5, P.text, 13, 'center'); x += 45;
    const preview = named ? `» ${s.slug}-1.jpg` : '» …';
    x += this.text(preview, x, 50, P.dim, 13) + 9;
    this.rect(x, 41, 1, 18, P.sep === '#1A1A1A' ? '#3A3A3A' : P.sep); x += 9;
    // jakość, max dłuższy bok
    x += this.text(T.quality, x, 50, P.text, 13) + 6;
    this.rr(x, 48, 98, 4, 2, P.field); this.rr(x, 48, 84, 4, 2, P.field);
    this.rr(x + 77.5, 41.5, 12, 18, 3, P.field, P.text); x += 106;
    this.rr(x + 0.5, 40.5, 42, 20, P.r, P.field, P.btnBorder);
    this.text('90 %', x + 21.5, 50.5, P.text, 13, 'center'); x += 51;
    x += this.checkbox(T.maxSide, x, 40, false) + 6;
    this.rr(x + 0.5, 40.5, 59, 20, P.r, this.theme === 'dark' ? '#2F2F2F' : '#E8E8E8');
    this.text('1200 px', x + 30, 50.5, P.faint, 13, 'center');
    // prawa strona: folder, zmień nazwę, zapisz z zakładek
    const y2 = named ? 64 : 40, enabled = named && s.files > 0;
    const save = `${T.saveTabs} (${s.files})`;
    const folder = s.files ? T.folderWeb : T.folderNone;
    const ws = Math.max(170, this.tw(save, 13) + 24), wr = Math.max(91, this.tw(T.renameOnly, 13) + 16), wf = Math.max(s.files ? 84 : 72, this.tw(folder, 13) + 16);
    let rx = 1429 - ws;
    this.button(save, rx, y2, { w: ws, h: 25, accent: true, disabled: !enabled, pressed: s.pressed === 'save' });
    rx -= wr + 7; this.button(T.renameOnly, rx, y2, { w: wr, h: 25, disabled: !enabled });
    rx -= 38; this.rr(rx + 0.5, y2 + 0.5, 30, 24, P.r, this.theme === 'dark' ? '#2D2D2D' : P.field, P.btnBorder); this.icon('folder', rx + 15, y2 + 12.5, P.faint, 0.85);
    rx -= wf + 9; this.button(folder, rx, y2, { w: wf, h: 25 });
  }
  toolStrip(top) {
    const P = this.P;
    this.rect(0, top + 1, 46, UI_H - top - 1, P.chrome);
    this.rect(46, top + 1, 1, UI_H - top - 1, P.sep);
    const ys = [24, 64, 104, 143], kinds = ['move', 'crop', 'eraser', 'rect'];
    for (let i = 0; i < 4; i++) {
      const y = top + ys[i];
      if (i === 0) this.rr(5, y - 16, 36, 33, P.r, P.accent); else this.rr(5.5, y - 15.5, 35, 32, P.r, P.field, P.btnBorder);
      this.icon(kinds[i], 23, y, i === 0 ? '#FFFFFF' : P.icon);
    }
    this.rect(8, top + 172, 30, 1, P.sep);
    this.rr(5.5, top + 189.5, 35, 32, P.r, P.field, P.btnBorder); this.icon('undo', 23, top + 205, this.st.bg ? P.icon : P.faint);
    this.rr(5.5, top + 228.5, 35, 32, P.r, P.field, P.btnBorder); this.icon('redo', 23, top + 244, P.faint);
  }
  dockTab(label, x, y, w, stripW = 310) {
    const P = this.P;
    this.rect(47, y, stripW, 25, P.tabStrip);
    this.rr(x, y + 1, w, 24, [P.r, P.r, 0, 0], P.card);
    this.text(label, x + 6, y + 13, P.text, 13);
  }
  leftDock(top) {
    const P = this.P, T = this.T, s = this.st;
    this.rect(47, top + 1, 310, UI_H - top - 1, P.dock);
    this.dockTab(T.listOptions, 48, top + 1, this.tw(T.listOptions, 13) + 14);
    this.rect(48, top + 26, 309, (top === 72 ? 280 : 298) - top - 26, P.card);
    // przyciski zawijane jak w egui (prawa granica panelu), potem widok i rozmiar
    let x = 60, ry = top + 39;
    const put = (wd, draw) => { if (x > 60 && x + wd > 345) { x = 60; ry += 27; } draw(x, ry); x += wd + 7; };
    const btn = (label, opt) => put(this.tw(label, 13) + 16, (bx, by) => this.button(label, bx, by, opt));
    btn(T.addPhotos); btn(T.addFolder); btn(T.openAll, { disabled: true }); btn(T.deselect); btn(T.clearList);
    const ft = T.files(s.files, s.files); put(this.tw(ft, 13), (bx, by) => this.text(ft, bx, by + 11, P.dim, 13));
    const vy = ry + 31 + 11 - 39 + 39;
    x = 60;
    x += this.text(T.view, x, vy, P.text, 13) + 6;
    x += this.button(T.thumbs, x - 1, vy - 11, { selected: true }) + 14;
    x += this.text(T.list, x, vy, P.text, 13) + 22;
    this.text(T.details, x, vy, P.text, 13);
    const sy = vy + 23 - (top + 131);
    this.g.save(); this.g.translate(0, sy);
    const zx = 60 + this.text(T.size, 60, top + 131, P.text, 13) + 8;
    this.rr(zx, top + 133, 98, 4, 2, P.field); this.rr(zx + 24.5, top + 126.5, 12, 18, 3, P.field, P.text);
    this.checkbox(T.names, zx + 106, top + 125, true);
    this.g.restore();
    // „Lista plików” z miniaturami
    const ly = top === 72 ? 280 : 298;
    this.dockTab(T.fileList, 48, ly, this.tw(T.fileList, 13) + 14);
    this.rect(48, ly + 25, 309, UI_H - ly - 25, P.card);
    for (let i = 0; i < s.files; i++) this.thumb(i, 52 + (i % 2) * 140, ly + 33 + Math.floor(i / 2) * 174);
  }
  thumb(i, x, y) {
    const P = this.P, s = this.st, T = this.T;
    const sel = true;
    this.rr(x + 0.5, y + 0.5, 131, 164, P.r, this.theme === 'dark' ? '#383838' : '#FAFAFA', sel ? P.accent : P.fieldBorder);
    if (s.thumbHover === i) this.rr(x + 0.5, y + 0.5, 131, 164, P.r, null, P.text);
    const img = this.photos[i];
    if (img) this.g.drawImage(img, x + 6, y + 20, 120, 90);
    this.rr(x + 4, y + 3, 26, 18, 3, P.accent); this.text(`#${i + 1}`, x + 17, y + 12.5, '#FFFFFF', 12, 'center', 400);
    if (i === 1 && s.ver2) { this.rr(x + 34, y + 3, 36, 18, 3, P.warning); const g = this.g; this.text('ver2', x + 52, y + 12.5, '#000000', 12, 'center', 400); g.strokeStyle = '#000'; g.lineWidth = 0.45; g.strokeText('ver2', x + 52, y + 12.5); }
    this.text(this.fileNames[i], x + 66, y + 136, P.text, 12, 'center');
    this.text('1600×1200', x + 66, y + 152, P.dim, 11, 'center');
  }
  tabTitle(i) {
    const s = this.st;
    if (!s.slug) return `#${i + 1} · ${this.fileNames[i]}`;
    return `#${i + 1} · ${s.slug}-${i + 1}${i === 1 && s.ver2 ? '-ver2' : ''}.jpg`;
  }
  center(top) {
    const P = this.P, T = this.T, s = this.st;
    this.rect(357, top + 1, 1, UI_H - top - 1, P.sep);
    this.rect(358, top + 1, 802, UI_H - top - 1, P.card);
    // pasek zakładek zdjęć
    this.rect(358, top + 1, 802, 25, P.tabStrip);
    this.rect(358, top + 26, 802, 1, P.sep);
    let x = 359;
    for (let i = 0; i < s.files; i++) {
      const label = this.tabTitle(i), w = this.tw(label, 13) + 36;
      if (i === s.active) { this.rr(x + 0.5, top + 2.5, w + 1, 25, [P.r, P.r, 0, 0], P.card, P.accent); this.rect(x + 1, top + 26, w, 2, P.card); }
      this.rect(x + w + 2, top + 2, 2, 24, P.canvas);
      this.text(label, x + 7, top + 13.5, i === s.active ? P.text : P.dim, 13);
      const g = this.g; g.strokeStyle = P.dim; g.lineWidth = 1.1; g.beginPath();
      g.moveTo(x + w - 18, top + 9); g.lineTo(x + w - 9, top + 18); g.moveTo(x + w - 9, top + 9); g.lineTo(x + w - 18, top + 18); g.stroke();
      x += w + 5;
    }
    // płótno – sam podkład; zdjęcie rysuje scena WebGPU (efekty usuwania tła, kadru)
    // płótno: alfa 0.5 jako znacznik dla shadera (tam rysuje zdjęcie); menu i okna nad nim zostają nieprzezroczyste
    this.g.clearRect(364, top + 33, 790, UI_H - 29 - top - 33);
    this.rect(364, top + 33, 790, UI_H - 29 - top - 33, P.canvas + '80');
    const st = s.dims.status;
    const sw = this.text(s.inspect ? st + T.inspecting : st, 372, 884, P.text, 12);
    // podpowiedź za stanem; gdy brakuje miejsca – skrócona z „…” (jak w programie)
    let hint = T.statusHint;
    const hx = Math.max(372 + sw + 30, 1144 - this.tw(hint, 12));
    while (hint.length > 4 && hx + this.tw(hint, 12) > 1144) hint = hint.slice(0, -2) + '…';
    this.text(hint, hx, 884, P.dim, 12);
    if (s.toast) {
      const w = this.tw(s.toast, 13) + 14;
      this.rr(719 - w / 2 + 0.5, 856.5, w, 26, P.r + 1, P.card, P.cardBorder);
      this.text(s.toast, 719, 869.5, P.text, 13, 'center');
    }
  }
  // prostokąt zdjęcia na płótnie (współrzędne logiczne) – dla sceny WebGPU
  photoRect(top, aspect) {
    const ax = 364, ay = top + 33, aw = 790, ah = UI_H - 29 - top - 33;
    let w = aw * 0.935, h = w / aspect;
    if (h > ah * 0.935) { h = ah * 0.935; w = h * aspect; }
    return [ax + (aw - w) / 2, ay + (ah - h) / 2, w, h];
  }
  inspector(top) {
    // położenia z prawdziwego programu (środki wierszy względem górnej krawędzi doku)
    const P = this.P, T = this.T, s = this.st, X = 1172;
    this.rect(1160, top + 1, 2, UI_H - top - 1, P.sep);
    this.rect(1162, top + 1, 278, UI_H - top - 1, P.card);
    this.g.save(); this.g.beginPath(); this.g.rect(1162, top + 1, 278, 890 - top - 1); this.g.clip();
    this.rr(X, top + 10, 126, 23, P.r, P.accent); this.text(T.tools, 1235, top + 21.5, P.onAccent, 13, 'center');
    this.text(T.metadata, 1366, top + 21.5, P.text, 13, 'center');
    this.rect(X, top + 42, 256, 1, P.sep);
    const row = (dy) => top + dy;
    this.text(this.fileNames[s.active], X, row(58), P.text, 14);
    const out = s.slug ? `#${s.active + 1} » ${s.slug}-${s.active + 1}${s.active === 1 && s.ver2 ? '-ver2' : ''}.jpg` : `#${s.active + 1} » ${T.pasteAName}`;
    this.text(out, X, row(78), P.accent, 13);
    const d = s.dims;
    this.text(T.source, X, row(102), P.dim, 13); this.text(d.source, 1222, row(102), P.text, 13);
    this.text(T.result, X, row(126), P.dim, 13); this.text(d.result, 1222, row(126), P.text, 13);
    this.text(T.saved, X, row(150), P.dim, 13); this.text(d.saved, 1222, row(150), P.text, 13);
    this.rect(X, row(170), 256, 1, P.sep);
    this.text(T.background, X, row(190), P.text, 13);
    let w = this.button(T.removeBg, X, row(205), { pressed: s.pressed === 'bg' });
    this.button('', X + w + 6, row(208), { w: 28, h: 17 }); this.icon('gear', X + w + 20, row(216.5), P.icon, 0.75);
    let y = 243;
    this.checkbox(T.whiteBg, X, row(y - 10.5), true);
    if (s.bg) { y += 27; this.checkbox(T.activeMask, X, row(y - 10.5), true); }
    y += 27;
    if (s.bg) { const rw = this.button(T.resetMask, X, row(y - 11)); this.button(T.inspection, X + rw + 6, row(y - 11), { selected: s.inspect, flat: !s.inspect }); }
    else this.button(T.inspection, X, row(y - 11), { selected: s.inspect, flat: !s.inspect });
    const k = s.bg ? 0 : -27; // bez maski: o jeden wiersz wyżej
    this.rect(X, row(317 + k), 256, 1, P.sep);
    this.text(T.crop, X, row(337 + k), P.text, 13);
    this.text(d.cropLine, X, row(359 + k), P.text, 13);
    w = this.button(T.cropC, X, row(372 + k)); w += this.button(T.cropAuto, X + w + 6, row(372 + k), { pressed: s.pressed === 'crop' }) + 6;
    this.button(T.reset, X + w + 6, row(372 + k), { disabled: !s.crop });
    this.rect(X, row(403 + k), 256, 1, P.sep);
    this.text(T.margins, X, row(423 + k), P.text, 13);
    for (let i = 0; i < 4; i++) {
      const yy = row(449 + k + i * 26);
      this.text(T.sides[i], X, yy, P.text, 13);
      this.button('–', 1228, yy - 11, { w: 22 });
      const vw = this.tw(d.margins[i], 13) + 16;
      this.rr(1254.5, yy - 10.5, vw - 1, 21, P.r, P.field, P.btnBorder);
      this.text(d.margins[i], 1254 + vw / 2, yy + 0.5, P.text, 13, 'center');
      this.button('+', 1254 + vw + 4, yy - 11, { w: 22 });
    }
    this.icon('link', 1188, row(554 + k), P.icon, 0.7); this.text(T.link, 1196, row(554 + k), P.text, 13);
    w = this.button(T.square, X, row(570 + k), { pressed: s.pressed === 'square' }); this.button(T.zero, X + w + 6, row(570 + k));
    this.rect(X, row(601 + k), 256, 1, P.sep);
    this.text(T.quick, X, row(621 + k), P.text, 13);
    w = this.button(T.frame5, X, row(636 + k), { pressed: s.pressed === 'frame' }); this.button(T.barsLR, X + w + 6, row(636 + k));
    w = this.button(T.barsTB, X, row(663 + k)); this.button(T.min500, X + w + 6, row(663 + k));
    w = this.button(T.pastePhone, X, row(690 + k)); this.button(T.phoneFile, X + w + 6, row(690 + k));
    this.rect(X, row(721 + k), 256, 1, P.sep);
    this.text(T.aiTools, X, row(741 + k), P.text, 13);
    this.button('LaMa', X, row(756 + k), { selected: true }); this.text('waifu2x', 1233, row(767 + k), P.text, 13);
    const lines = this.wrap(T.lamaHint, 256, 13);
    for (let i = 0; i < lines.length; i++) this.text(lines[i], X, row(791 + k) + i * 16, P.dim, 13);
    const by = row(791 + k) + lines.length * 16 - 4;
    const sw = this.button(T.selectM, X, by); this.button(T.removeObj, X + sw + 7, by, { disabled: true });
    this.g.restore();
  }
  startScreen(top) {
    const P = this.P, T = this.T;
    this.rect(47, top + 1, 1113, UI_H - top - 1, P.dock);
    this.dockTab(T.listOptions, 48, top + 1, this.tw(T.listOptions, 13) + 14, 1113);
    this.rect(48, top + 26, 1111, 181, P.card);
    let x = 60, y = top + 40;
    x += this.button(T.addPhotos, x, y) + 6; x += this.button(T.addFolder, x, y) + 6;
    x += this.button(T.openAll, x, y, { disabled: true }) + 6; x += this.button(T.deselect, x, y, { disabled: true }) + 6;
    x += this.button(T.clearList, x, y, { disabled: true }) + 6; this.text(T.files(0, 0), x, y + 11, P.dim, 13);
    y += 31; x = 60; x += this.text(T.view, x, y + 11, P.text, 13) + 6; x += this.button(T.thumbs, x, y, { selected: true }) + 14;
    x += this.text(T.list, x, y + 11, P.text, 13) + 22; x += this.text(T.details, x, y + 11, P.text, 13) + 18;
    this.rect(x, y + 2, 1, 18, P.sep); x += 10;
    x += this.text(T.size, x, y + 11, P.text, 13) + 10; this.rr(x, y + 9, 98, 4, 2, P.field); this.rr(x + 24, y + 2, 12, 18, 3, P.field, P.fieldBorder);
    this.checkbox(T.names, x + 106, y + 1, true);
    const ly = top + 208;
    this.dockTab(T.fileList, 48, ly, this.tw(T.fileList, 13) + 14, 1113);
    this.rect(48, ly + 25, 1111, UI_H - ly - 25, P.card);
    this.rr(80, ly + 60, 176, 31, P.r, P.accent); this.text(T.addPhotos, 88, ly + 76, '#FFFFFF', 13);
    this.rr(80.5, ly + 100.5, 175, 30, P.r, P.field, P.fieldBorder); this.text(T.addFolder, 88, ly + 116, P.text, 13);
    this.text(T.website, 80, ly + 168, P.accent, 13); this.text(T.code, 80, ly + 188, P.accent, 13);
    this.text('AsOne', 302, ly + 75, P.text, 24);
    this.text(T.startTitle, 302, ly + 100, P.dim, 13);
    this.rect(302, ly + 124, 852, 1, P.sep);
    this.text(T.recent, 302, ly + 150, P.text, 17);
    this.text(T.recentHint, 302, ly + 178, P.dim, 13);
    this.text(T.dropHint, 302, ly + 216, P.dim, 13);
    // panel narzędzi z pustą podpowiedzią
    this.rect(1160, top + 1, 280, UI_H - top - 1, P.card); this.rect(1160, top + 1, 1, UI_H - top - 1, P.sep);
    this.rr(1172, top + 9, 126, 24, P.r, P.accent); this.text(T.tools, 1235, top + 21, '#FFFFFF', 13, 'center');
    this.text(T.metadata, 1366, top + 21, P.text, 13, 'center');
    this.rect(1172, top + 41, 256, 1, P.sep);
    const g = this.g; g.font = this.font(13); g.fillStyle = P.dim; g.textAlign = 'left';
    const words = T.emptyHint.split(' '); let line = '', ly2 = top + 56;
    for (const wd of words) { const t = line ? line + ' ' + wd : wd; if (g.measureText(t).width > 250) { g.fillText(line, 1172, ly2); line = wd; ly2 += 16; } else line = t; }
    g.fillText(line, 1172, ly2);
  }
  contextMenu(top) {
    const P = this.P, T = this.T, s = this.st;
    let mw = 0; for (const m of T.menu) if (m) mw = Math.max(mw, this.tw(m, 13));
    const x = 258, y = top + 313, w = Math.round(Math.max(150, mw + 20)), rowH = 27;
    let h = 8;
    for (const m of T.menu) h += m ? rowH : 11;
    this.shadowBox(x, y, w, h);
    let yy = y + 4;
    T.menu.forEach((m, i) => {
      if (!m) { this.rect(x + 6, yy + 5, w - 12, 1, P.sep); yy += 11; return; }
      if (s.menuHover === i) this.rr(x + 4, yy + 1, w - 8, rowH - 2, P.r, P.hover);
      this.text(m, x + 9, yy + rowH / 2, i === T.menu.length - 1 ? P.danger : P.text, 13);
      if (i === 2) { const g = this.g; g.fillStyle = P.text; g.beginPath(); g.moveTo(x + w - 14, yy + 10); g.lineTo(x + w - 9, yy + 13.5); g.lineTo(x + w - 14, yy + 17); g.fill(); }
      yy += rowH;
    });
    if (s.menu === 2) {
      const sx = x + w + 4, sy = y + 4 + rowH * 2 - 4, sw = 183;
      let sh = 8; for (const v of T.versions) sh += v ? rowH : 11;
      this.shadowBox(sx, sy, sw, sh);
      let y2 = sy + 4;
      T.versions.forEach((v, i) => {
        if (!v) { this.rect(sx + 6, y2 + 5, sw - 12, 1, P.sep); y2 += 11; return; }
        if (s.verHover === i) this.rr(sx + 4, y2 + 1, sw - 8, rowH - 2, P.r, P.hover);
        const last = i === T.versions.length - 1;
        this.text(v, sx + (last ? 9 : 20), y2 + rowH / 2, last && !s.ver2 ? P.faint : P.text, 13);
        y2 += rowH;
      });
    }
  }
  shadowBox(x, y, w, h) {
    // miękki cień warstwami (bez rozmycia Canvas2D – tanie przy odświeżaniu fragmentów)
    const P = this.P, dark = this.theme === 'dark';
    for (let i = 7; i >= 1; i--) this.rr(x - i * 1.6, y - i * 1.6 + 4, w + i * 3.2, h + i * 3.2, P.r + 1 + i * 1.6, dark ? 'rgba(0,0,0,0.055)' : 'rgba(0,0,0,0.025)');
    this.rr(x, y, w, h, P.r + 1, P.card, P.cardBorder);
    this.rr(x + 0.5, y + 0.5, w - 1, h - 1, P.r + 1, null, dark ? '#1A1A1A' : '#C8C8C8');
  }
  exportDialog() {
    // „Eksport zakończony” – położenia i liczby z prawdziwego okna
    const P = this.P, T = this.T, x = 462, y = 250, w = 518, h = 212, dark = this.theme === 'dark';
    this.shadowBox(x, y, w, h);
    this.rr(x + 1, y + 1, w - 2, 33, [P.r + 1, P.r + 1, 0, 0], dark ? '#4A4A4A' : '#E2E2E2');
    this.text(T.exportDone, x + (w - 30) / 2, y + 17.5, P.text, 17, 'center');
    const g = this.g; g.strokeStyle = P.text; g.lineWidth = 1.2; g.beginPath();
    g.moveTo(x + w - 25, y + 12); g.lineTo(x + w - 15, y + 22); g.moveTo(x + w - 15, y + 12); g.lineTo(x + w - 25, y + 22); g.stroke();
    this.rect(x + 1, y + 34, w - 2, 1, P.sep);
    this.text(REAL.export.summary[this.lang], x + 8, y + 47, P.text, 13);
    this.text(REAL.export.dir, x + 8, y + 68, P.dim, 13);
    this.rect(x + 8, y + 84, w - 16, 1, P.sep);
    let nameW = 0;
    for (const r of REAL.export.rows) nameW = Math.max(nameW, this.tw(r.name, 13));
    REAL.export.rows.forEach((r, i) => {
      const yy = y + 102 + i * 27;
      if (i === 1) this.rect(x + 3, yy - 13.5, 405, 27, dark ? '#424242' : '#E2E2E2');
      this.text('✓', x + 11, yy + 1, P.ok, 12, 'center');
      this.text(r.name, x + 51, yy - 1, P.text, 13);
      this.text(`${r.dims} · ${r.kb} KB · ${T.quality_} ${r.q}`, x + 51 + nameW + 3, yy + 2, P.text, 13);
    });
    this.rect(x + 8, y + 178, w - 16, 1, P.sep);
    let bx = x + 8; bx += this.button(T.openFolder, bx, y + 184) + 2; bx += this.button(T.packZip, bx, y + 184) + 3;
    this.button(T.close, bx, y + 184, { pressed: this.st.pressed === 'close' });
    // uchwyt zmiany rozmiaru w prawym dolnym rogu
    g.strokeStyle = P.faint; g.lineWidth = 1; g.beginPath();
    for (let i = 0; i < 3; i++) { g.moveTo(x + w - 3 - i * 4, y + h - 3); g.lineTo(x + w - 3, y + h - 3 - i * 4); }
    g.stroke();
  }
}
