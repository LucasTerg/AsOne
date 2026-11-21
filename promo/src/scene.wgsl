// Scena: pustka studia, okno AsOne (tekstura UI + zdjęcie na płótnie liczone tutaj), zdjęcia jako panele,
// cząsteczki rozpadającego się tła i sprite'y (pliki, folder „web/”, kursor). Wyjście: kolor HDR + odległość.

struct Frame {
  view_proj: mat4x4f,
  cam_pos: vec3f, time: f32,
  cam_right: vec3f, aspect: f32,
  cam_up: vec3f, glow: f32,
};
@group(0) @binding(0) var<uniform> fr: Frame;
@group(0) @binding(1) var samp: sampler;

struct FOut { @location(0) color: vec4f, @location(1) depth: vec4f };

fn hash11(n: f32) -> f32 { return fract(sin(n * 127.1) * 43758.5453); }
fn hash21(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
fn noise2(p: vec2f) -> f32 {
  let i = floor(p); let f = fract(p); let u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash21(i), hash21(i + vec2f(1.0, 0.0)), u.x), mix(hash21(i + vec2f(0.0, 1.0)), hash21(i + vec2f(1.0, 1.0)), u.x), u.y);
}
fn view_depth(world: vec3f) -> f32 { return distance(world, fr.cam_pos); }

// ---------------------------------------------------------------------------------------------
// Pustka: prawie czarna, chłodna poświata za oknem i ledwo widoczna podłoga.
struct BgOut { @builtin(position) pos: vec4f, @location(0) ndc: vec2f };
@vertex fn vs_bg(@builtin(vertex_index) i: u32) -> BgOut {
  var p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: BgOut; o.pos = vec4f(p[i], 1.0, 1.0); o.ndc = p[i]; return o;
}
struct BgU { inv_view_proj: mat4x4f, tint: vec4f };
@group(1) @binding(0) var<uniform> bgu: BgU;
@fragment fn fs_bg(in: BgOut) -> FOut {
  let w = bgu.inv_view_proj * vec4f(in.ndc, 1.0, 1.0);
  let dir = normalize(w.xyz / w.w - fr.cam_pos);
  var col = mix(vec3f(0.0012, 0.0014, 0.002), vec3f(0.003, 0.0036, 0.0052), smoothstep(-0.5, 0.4, dir.y));
  let back = max(dot(dir, normalize(vec3f(0.0, 0.08, -1.0))), 0.0);
  col = col + vec3f(0.004, 0.007, 0.016) * pow(back, 6.0) * bgu.tint.x;
  col = col + vec3f(0.008, 0.014, 0.032) * pow(back, 40.0) * bgu.tint.x;
  if (dir.y < -0.02) {
    let t = (fr.cam_pos.y + 6.2) / -dir.y;
    let p = fr.cam_pos + dir * t;
    let fade = exp(-t * 0.04);
    let g = abs(fract(p.xz * 0.5) - 0.5);
    let line = smoothstep(0.02, 0.0, min(g.x, g.y));
    col = col + (vec3f(0.006, 0.01, 0.022) * line + vec3f(0.01, 0.016, 0.035) * exp(-length(p.xz) * 0.09)) * fade * bgu.tint.y;
  }
  var o: FOut; o.color = vec4f(col, 1.0); o.depth = vec4f(1000.0, 0.0, 0.0, 1.0);
  return o;
}

// ---------------------------------------------------------------------------------------------
// Zdjęcie na płótnie programu: surowe → usunięte tło → inspekcja tła → kadr do obiektu → kwadrat →
// ramka 5 px. Liczone w pikselach prawdziwego zdjęcia 1600×1200, w prostokącie płótna okna.
struct Canvas {
  rect: vec4f,       // prostokąt zdjęcia w pikselach okna 1440×900 (x0, y0, x1, y1)
  view: vec4f,       // widoczny wynik w pikselach wyniku (x0, y0, x1, y1)
  crop: vec4f,       // kadr w pikselach zdjęcia
  margins: vec4f,    // marginesy (góra, dół, lewo, prawo)
  p: vec4f,          // x: warstwa, y: usuwanie tła 0..1, z: inspekcja 0..1, w: widoczność
  fx: vec4f,         // x: błysk krawędzi kadru, yz: środek produktu w zdjęciu, w: –
};
@group(1) @binding(7) var<uniform> cv: Canvas;

fn removal_front(src: vec2f) -> f32 {
  // front usuwania: od produktu na zewnątrz, z postrzępioną krawędzią
  // od krawędzi zdjęcia do produktu: tło najdalej od produktu znika pierwsze
  let r = length((src - cv.fx.yz) * vec2f(1.0, 1.15)) / 950.0;
  return (1.0 - clamp(r, 0.0, 1.0)) * 0.8 + (noise2(src * 0.02) * 0.65 + noise2(src * 0.07) * 0.35) * 0.2;
}

// ---------------------------------------------------------------------------------------------
// Panele (okno UI, zdjęcia, sprite'y): instancja = środek, osie, rozmiar, rodzaj, parametry.
struct Panel {
  center: vec3f, kind: f32,       // kind: 0 okno UI, 1 zdjęcie surowe, 2 sprite atlasu, 3 zdjęcie po obróbce
  axis_x: vec3f, layer: f32,      // połowa szerokości w osi x
  axis_y: vec3f, alpha: f32,      // połowa wysokości w osi y
  uv: vec4f,                      // prostokąt w teksturze (u0, v0, u1, v1)
  fx: vec4f,                      // x: poświata krawędzi, y: światło (składanie okna), z: przejście motywu, w: emisja
};
@group(1) @binding(1) var<storage, read> panels: array<Panel>;
@group(1) @binding(2) var ui_dark: texture_2d<f32>;
@group(1) @binding(3) var ui_light: texture_2d<f32>;
@group(1) @binding(4) var photos: texture_2d_array<f32>;
@group(1) @binding(5) var atlas: texture_2d<f32>;
@group(1) @binding(6) var masks: texture_2d_array<f32>;
@group(1) @binding(8) var ui_prev: texture_2d<f32>;   // poprzedni stan interfejsu (przenikanie, ilość w p.layer)

struct POut {
  @builtin(position) pos: vec4f,
  @location(0) uv: vec2f,
  @location(1) local: vec2f,
  @location(2) @interpolate(flat) id: u32,
  @location(3) world: vec3f,
  @location(4) half: vec2f,
};
@vertex fn vs_panel(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> POut {
  let p = panels[ii];
  var c = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0));
  let l = c[vi];
  let world = p.center + p.axis_x * l.x + p.axis_y * l.y;
  var o: POut;
  o.pos = fr.view_proj * vec4f(world, 1.0);
  o.local = l;
  o.uv = vec2f(mix(p.uv.x, p.uv.z, l.x * 0.5 + 0.5), mix(p.uv.w, p.uv.y, l.y * 0.5 + 0.5));
  o.id = ii;
  o.world = world;
  o.half = vec2f(length(p.axis_x), length(p.axis_y));
  return o;
}
@fragment fn fs_panel(in: POut) -> FOut {
  let p = panels[in.id];
  let kind = i32(p.kind + 0.5);
  // wszystkie próbki z jawnymi pochodnymi (jednorodny przepływ sterowania)
  let gx = dpdx(in.uv); let gy = dpdy(in.uv);
  let ugx = gx * 0.8; let ugy = gy * 0.8;   // interfejs nieco ostrzej (cienki tekst nie blednie przy pomniejszeniu)
  // piksel okna i odpowiadający mu piksel zdjęcia na płótnie
  let lp = in.uv * vec2f(1440.0, 900.0);
  let rk = (lp - cv.rect.xy) / max(cv.rect.zw - cv.rect.xy, vec2f(1.0));
  let out_px = mix(cv.view.xy, cv.view.zw, rk);
  let src = out_px - vec2f(cv.margins.z, cv.margins.x) + cv.crop.xy;
  let puv = src / vec2f(1600.0, 1200.0);
  let pgx = dpdx(puv); let pgy = dpdy(puv);
  let fw = fwidth(lp) * 0.75;
  let li = i32(cv.p.x + 0.5);
  let raw = textureSampleGrad(photos, samp, puv, li, pgx, pgy).rgb;
  let pm = textureSampleGrad(masks, samp, puv, li, pgx, pgy).r;
  let d = mix(textureSampleGrad(ui_dark, samp, in.uv, ugx, ugy), textureSampleGrad(ui_prev, samp, in.uv, ugx, ugy), select(0.0, p.layer, kind == 0));
  let l = textureSampleGrad(ui_light, samp, in.uv, ugx, ugy);
  let pl = textureSampleGrad(photos, samp, in.uv, i32(p.layer), gx, gy).rgb;
  let plm = textureSampleGrad(masks, samp, in.uv, i32(p.layer), gx, gy).r;
  let at = textureSampleGrad(atlas, samp, in.uv, gx, gy);
  // odległość do krawędzi panelu w jednostkach świata (gładka poświata, bez schodków z pochodnych)
  let edge_w = min((1.0 - abs(in.local.x)) * in.half.x, (1.0 - abs(in.local.y)) * in.half.y);
  var col = vec4f(0.0);
  if (kind == 0) {
    // okno: przejście ciemny → jasny jednym ruchem (ukośna linia światła)
    let wipe = p.fx.z * 2.6 - 0.8 - (in.uv.x * 0.7 + in.uv.y * 0.3);
    let k = smoothstep(0.0, 0.04, wipe);
    let ui = mix(d, l, k);
    // płótno: tekstura UI ma tam alfę 0.5 (znacznik); menu i okna dialogowe nad płótnem są nieprzezroczyste
    let canvasness = 1.0 - smoothstep(0.55, 0.98, ui.a);
    let cover = smoothstep(cv.rect.xy - fw, cv.rect.xy + fw, lp) * (1.0 - smoothstep(cv.rect.zw - fw, cv.rect.zw + fw, lp));
    let inside_photo = cover.x * cover.y * cv.p.w;
    let in_crop = all(src >= cv.crop.xy - 0.5) && all(src <= cv.crop.zw + 0.5);
    // usuwanie tła: front od produktu na zewnątrz; krawędź świeci
    let n = removal_front(src);
    let front = cv.p.y * 1.25 - 0.1;
    let gone = smoothstep(n - 0.015, n + 0.015, front) * (1.0 - pm);
    let burn = exp(-abs(front - n) * 45.0) * (1.0 - pm) * step(0.001, cv.p.y) * step(cv.p.y, 0.999);
    var rgb = mix(raw, vec3f(1.0), gone);
    if (!in_crop) { rgb = vec3f(1.0); }
    // inspekcja tła: biel zostaje białą, każdy inny piksel – czarny (jak w programie)
      // inspekcja tła jak w AsOne: clamp((rgb − 0,9) / 0,1) – czysta biel zostaje, prawie biel szarzeje, produkt czarny
    let srgb_v = pow(max(rgb, vec3f(0.0)), vec3f(1.0 / 2.2));
    let insp = pow(clamp((srgb_v - vec3f(0.9)) / 0.1, vec3f(0.0), vec3f(1.0)), vec3f(2.2));
    rgb = mix(rgb, insp, cv.p.z);
    rgb = rgb + vec3f(0.35, 0.6, 1.3) * burn * 1.0;
    let e = min(min(rk.x, 1.0 - rk.x), min(rk.y, 1.0 - rk.y)) * (cv.rect.z - cv.rect.x);
    rgb = rgb + vec3f(0.4, 0.65, 1.3) * exp(-max(e, 0.0) * 0.6) * cv.fx.x * inside_photo;
    let under = mix(ui.rgb, rgb, inside_photo);
    var c = mix(ui.rgb, under, canvasness);
    let sweep_line = exp(-abs(wipe) * 60.0) * step(0.001, p.fx.z) * step(p.fx.z, 0.999);
    c = c + vec3f(0.6, 0.8, 1.2) * sweep_line * 1.5;
    // składanie okna „ze światła”: kawałek leci jako niebieski hologram interfejsu z jasną krawędzią
    // i linią skanu, a po dotarciu na miejsce stygnie do prawdziwych kolorów
    let lightk = p.fx.y;
    let lum_ui = dot(c, vec3f(0.3, 0.55, 0.15));
    let holo = vec3f(0.18, 0.4, 1.0) * (0.25 + lum_ui * 2.2);
    let rim = exp(-edge_w / 0.025) * lightk * 4.9;
    let scan = exp(-abs((in.local.y * -0.5 + 0.5) - (1.0 - lightk)) * 70.0) * lightk * step(0.02, lightk) * 2.1;
    c = mix(c, holo, smoothstep(0.0, 0.6, lightk)) + vec3f(0.35, 0.6, 1.4) * (rim + scan);
    col = vec4f(c * p.fx.w, 1.0);
  } else if (kind == 1 || kind == 3) {
    var t = pl;
    if (kind == 3) {
      // gotowe zdjęcie: białe tło (maska z sylwetki produktu), poza kadrem biel
      t = mix(vec3f(1.0), pl, plm);
      if (any(in.uv < vec2f(0.0)) || any(in.uv > vec2f(1.0))) { t = vec3f(1.0); }
    }
    let glow = exp(-edge_w / 0.03) * p.fx.x;
    col = vec4f(t * p.fx.w + vec3f(0.45, 0.7, 1.4) * glow, 1.0);
  } else {
    // atlas ma przemnożoną alfę
    col = vec4f(at.rgb * p.fx.w / max(at.a, 1e-4), at.a);
  }
  col.a = col.a * p.alpha;
  if (col.a < 0.004) { discard; }
  var o: FOut;
  o.color = vec4f(col.rgb * col.a, col.a);
  o.depth = vec4f(view_depth(in.world) * col.a, 0.0, 0.0, col.a);
  return o;
}

// ---------------------------------------------------------------------------------------------
// Cząsteczki: każda zaczyna w pikselu szarego tła zdjęcia i odlatuje, gdy dotrze do niej front usuwania.
struct Parts { center: vec3f, removal: f32, axis_x: vec3f, count: f32, axis_y: vec3f, alpha: f32, view: vec4f };
@group(1) @binding(10) var<uniform> pt: Parts;
@group(1) @binding(11) var photos_p: texture_2d_array<f32>;
@group(1) @binding(12) var masks_p: texture_2d_array<f32>;
struct PtOut { @builtin(position) pos: vec4f, @location(0) col: vec4f, @location(1) q: vec2f, @location(2) world: vec3f };
@vertex fn vs_parts(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> PtOut {
  let fi = f32(ii);
  let u = vec2f(hash11(fi * 1.37 + 0.1), hash11(fi * 2.11 + 0.7));
  let src = mix(pt.view.xy, pt.view.zw, u);
  let tex = vec2i(clamp(src, vec2f(0.0), vec2f(1599.0, 1199.0)));
  let m = textureLoad(masks_p, tex, 0, 0).r;
  let c = textureLoad(photos_p, tex, 0, 0).rgb;
  let n = removal_front(src);
  let front = pt.removal * 1.25 - 0.1;
  let born = clamp((front - n) * 3.0, 0.0, 1.0); // 0 – jeszcze tło, >0 – leci
  var o: PtOut;
  let base = pt.center + pt.axis_x * (u.x * 2.0 - 1.0) + pt.axis_y * (1.0 - u.y * 2.0);
  let t = born;
  let away = normalize(vec3f(u.x - cv.fx.y / 1600.0, cv.fx.z / 1200.0 - u.y, 0.0) + vec3f(0.0, 0.0, 0.35));
  let swirl = vec3f(sin(fi * 0.37 + t * 7.0), cos(fi * 0.53 + t * 6.0), sin(fi * 0.11)) * 0.12 * t;
  let world = base + (away * (0.6 + hash11(fi * 3.3) * 1.6) + vec3f(0.0, 0.5, 0.6)) * t * t * 2.2 + swirl;
  var corner = array<vec2f, 6>(vec2f(-1.0, -1.0), vec2f(1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, -1.0), vec2f(1.0, 1.0), vec2f(-1.0, 1.0));
  let q = corner[vi];
  let size = (0.009 + hash11(fi * 5.1) * 0.012) * (1.0 - t * 0.5);
  let wp = world + (fr.cam_right * q.x + fr.cam_up * q.y) * size;
  o.pos = fr.view_proj * vec4f(wp, 1.0);
  let visible = step(0.001, born) * (1.0 - m) * (1.0 - smoothstep(0.55, 1.0, born)) * pt.alpha;
  if (visible < 0.01 || fi >= pt.count) { o.pos = vec4f(0.0, 0.0, -2.0, 1.0); }
  o.col = vec4f(mix(c * 1.3, vec3f(0.5, 0.75, 1.6) * 2.2, 0.35 + 0.65 * smoothstep(0.0, 0.3, t)), visible);
  o.q = q;
  o.world = world;
  return o;
}
@fragment fn fs_parts(in: PtOut) -> FOut {
  let r = 1.0 - dot(in.q, in.q);
  if (r <= 0.0) { discard; }
  var o: FOut;
  let a = in.col.a * r * r;
  o.color = vec4f(in.col.rgb * a, a);
  o.depth = vec4f(view_depth(in.world) * a, 0.0, 0.0, a);
  return o;
}
