// Proceduralne zdjęcia produktów (SDF, raymarching): kubek, lampka, but, butelka na szarym tle studia.
// Wynik: kolor zdjęcia „z aparatu” (mode 0), ten sam kadr po usunięciu tła – białe tło z cieniem (mode 1)
// albo maska produktu (mode 2, kanał R).

struct Shot {
  cam_pos: vec3f, fov: f32,
  cam_target: vec3f, product: f32,
  bg_light: vec3f, mode: f32,
  bg_dark: vec3f, seed: f32,
  shift: vec2f, size: vec2f,
};
@group(0) @binding(0) var<uniform> shot: Shot;

struct VOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
  var p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VOut;
  o.pos = vec4f(p[i], 0.0, 1.0);
  o.uv = p[i] * vec2f(0.5, -0.5) + 0.5;
  return o;
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
  let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
  return mix(b, a, h) - k * h * (1.0 - h);
}
fn sd_round_cyl(p: vec3f, r: f32, h: f32, rr: f32) -> f32 {
  let d = vec2f(length(p.xz) - r + rr, abs(p.y) - h + rr);
  return min(max(d.x, d.y), 0.0) + length(max(d, vec2f(0.0))) - rr;
}
fn sd_torus_xy(p: vec3f, R: f32, r: f32) -> f32 {
  let q = vec2f(length(p.xy) - R, p.z);
  return length(q) - r;
}
fn sd_cone(p: vec3f, r1: f32, r2: f32, h: f32) -> f32 {
  // ścięty stożek: dół r1 (y=-h), góra r2 (y=+h)
  let q = vec2f(length(p.xz), p.y);
  let k1 = vec2f(r2, h);
  let k2 = vec2f(r2 - r1, 2.0 * h);
  let ca = vec2f(q.x - min(q.x, select(r2, r1, q.y < 0.0)), abs(q.y) - h);
  let cb = q - k1 + k2 * clamp(dot(k1 - q, k2) / dot(k2, k2), 0.0, 1.0);
  let s = select(1.0, -1.0, cb.x < 0.0 && ca.y < 0.0);
  return s * sqrt(min(dot(ca, ca), dot(cb, cb)));
}
fn sd_ellipsoid(p: vec3f, r: vec3f) -> f32 {
  let k0 = length(p / r);
  let k1 = length(p / (r * r));
  return k0 * (k0 - 1.0) / k1;
}
fn sd_round_box(p: vec3f, b: vec3f, r: f32) -> f32 {
  let q = abs(p) - b + r;
  return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

// Materiał: x – identyfikator (1 korpus, 2 akcent, 3 metal/ciemny, 4 jasny), y – połysk
var<private> mat: vec2f;

fn mug(p: vec3f) -> f32 {
  let q = p - vec3f(0.0, 0.46, 0.0);
  var body = sd_round_cyl(q, 0.36, 0.46, 0.06);
  let inner = sd_round_cyl(q - vec3f(0.0, 0.08, 0.0), 0.315, 0.46, 0.05);
  body = max(body, -inner);
  var handle = sd_torus_xy(q - vec3f(0.36, -0.02, 0.0), 0.2, 0.045);
  handle = max(handle, -(q.x - 0.33));
  mat = vec2f(1.0, 0.85);
  return smin(body, handle, 0.03);
}
fn lamp(p: vec3f) -> f32 {
  let base = sd_round_cyl(p - vec3f(0.0, 0.03, 0.0), 0.32, 0.03, 0.02);
  let stem = sd_round_cyl(p - vec3f(0.0, 0.5, 0.0), 0.025, 0.47, 0.01);
  var shade = sd_cone(p - vec3f(0.0, 1.08, 0.0), 0.42, 0.2, 0.24);
  shade = abs(shade) - 0.008;
  shade = max(shade, abs(p.y - 1.08) - 0.24);
  let bulb = length(p - vec3f(0.0, 0.96, 0.0)) - 0.07;
  var d = min(base, stem);
  mat = vec2f(3.0, 0.6);
  if (bulb < d) { d = bulb; mat = vec2f(4.0, 0.2); }
  if (shade < d) { d = shade; mat = vec2f(1.0, 0.35); }
  return d;
}
fn sneaker(p: vec3f) -> f32 {
  // podeszwa z uniesionym czubkiem (x: pięta -0.6 … czubek +0.6)
  let toe = smoothstep(0.2, 0.62, p.x);
  let ps = p - vec3f(0.0, 0.055 + toe * toe * 0.07, 0.0);
  let sole = sd_round_box(ps, vec3f(0.6, 0.045, 0.19 - 0.035 * toe), 0.04);
  // cholewka: czubek, śródstopie (wyższe), zapiętek
  var upper = sd_ellipsoid(p - vec3f(0.32, 0.12, 0.0), vec3f(0.3, 0.12, 0.175));
  upper = smin(upper, sd_ellipsoid(p - vec3f(-0.02, 0.2, 0.0), vec3f(0.38, 0.25, 0.18)), 0.1);
  upper = smin(upper, sd_round_box(p - vec3f(-0.42, 0.27, 0.0), vec3f(0.14, 0.2, 0.15), 0.11), 0.08);
  // otwór na stopę i ścięcie poniżej podeszwy
  upper = max(upper, -sd_ellipsoid(p - vec3f(-0.3, 0.5, 0.0), vec3f(0.23, 0.11, 0.12)));
  upper = max(upper, -(p.y - 0.08));
  // język
  let pt = p - vec3f(-0.1, 0.43, 0.0);
  let tongue = sd_round_box(vec3f(pt.x * 0.94 + pt.y * 0.34, pt.y * 0.94 - pt.x * 0.34, pt.z), vec3f(0.07, 0.025, 0.08), 0.02);
  var d = sole;
  mat = vec2f(4.0, 0.25);
  if (upper < d) {
    d = upper;
    // pasek wzdłuż boku (akcent)
    mat = select(vec2f(1.0, 0.3), vec2f(2.0, 0.4), abs(p.y - 0.17) < 0.028 && abs(p.z) > 0.11);
  }
  if (tongue < d) { d = tongue; mat = vec2f(1.0, 0.3); }
  // sznurówki na grzbiecie śródstopia
  for (var i = 0; i < 5; i = i + 1) {
    let x = 0.2 - f32(i) * 0.075;
    let top = 0.2 + 0.25 * sqrt(max(1.0 - (x / 0.38) * (x / 0.38), 0.0));
    let lace = sd_round_box(p - vec3f(x, top + 0.004, 0.0), vec3f(0.011, 0.011, 0.075), 0.009);
    if (lace < d) { d = lace; mat = vec2f(4.0, 0.2); }
  }
  return d;
}
fn bottle(p: vec3f) -> f32 {
  let body = sd_round_cyl(p - vec3f(0.0, 0.4, 0.0), 0.27, 0.4, 0.08);
  let shoulder = sd_cone(p - vec3f(0.0, 0.88, 0.0), 0.25, 0.1, 0.1);
  let neck = sd_round_cyl(p - vec3f(0.0, 1.04, 0.0), 0.095, 0.1, 0.02);
  let cap = sd_round_cyl(p - vec3f(0.0, 1.16, 0.0), 0.11, 0.06, 0.02);
  var d = smin(smin(body, shoulder, 0.08), neck, 0.04);
  mat = vec2f(1.0, 0.95);
  // etykieta: pas na korpusie
  if (abs(p.y - 0.42) < 0.16 && d < 0.01) { mat = vec2f(4.0, 0.15); }
  if (cap < d) { d = cap; mat = vec2f(2.0, 0.5); }
  return d;
}
fn product(p: vec3f) -> f32 {
  let id = i32(shot.product + 0.5);
  if (id == 0) { return mug(p); }
  if (id == 1) { return lamp(p); }
  if (id == 2) { return sneaker(p); }
  return bottle(p);
}
fn product_color(id: f32, m: f32) -> vec3f {
  let pid = i32(id + 0.5);
  if (m > 3.5) { return vec3f(0.92, 0.9, 0.86); }
  if (m > 2.5) { return vec3f(0.11, 0.13, 0.16); }
  if (pid == 0) { return vec3f(0.86, 0.33, 0.1); }   // kubek – pomarańczowa glazura
  if (pid == 1) { return select(vec3f(0.17, 0.4, 0.82), vec3f(1.0, 0.78, 0.32), m > 1.5); }
  if (pid == 2) { return select(vec3f(0.1, 0.42, 0.48), vec3f(0.96, 0.55, 0.18), m > 1.5); }
  return select(vec3f(0.1, 0.36, 0.24), vec3f(0.82, 0.82, 0.8), m > 1.5); // butelka
}

// Studio: podłoga y=0 przechodząca łukiem w ścianę z=-2.4 (cyklorama).
fn studio(p: vec3f) -> f32 {
  let r = 1.2;
  let zc = -2.4 + r;
  if (p.z > zc) { return p.y; }
  let q = vec2f(p.z - zc, p.y - r);
  if (q.y > 0.0) { return p.z + 2.4; }
  return r - length(q);
}

fn scene(p: vec3f) -> f32 { return min(product(p), studio(p)); }

fn normal_at(p: vec3f) -> vec3f {
  let e = vec2f(0.0008, 0.0);
  return normalize(vec3f(
    scene(p + e.xyy) - scene(p - e.xyy),
    scene(p + e.yxy) - scene(p - e.yxy),
    scene(p + e.yyx) - scene(p - e.yyx)));
}
fn soft_shadow(ro: vec3f, rd: vec3f, k: f32) -> f32 {
  var res = 1.0;
  var t = 0.01;
  for (var i = 0; i < 64; i = i + 1) {
    let h = product(ro + rd * t);
    res = min(res, k * h / t);
    t = t + clamp(h, 0.01, 0.2);
    if (res < 0.002 || t > 4.0) { break; }
  }
  return clamp(res, 0.0, 1.0);
}
fn ao(p: vec3f, n: vec3f) -> f32 {
  var occ = 0.0;
  var w = 1.0;
  for (var i = 1; i <= 5; i = i + 1) {
    let h = 0.03 * f32(i);
    occ = occ + (h - product(p + n * h)) * w;
    w = w * 0.7;
  }
  return clamp(1.0 - 2.2 * occ, 0.0, 1.0);
}

struct Hit { t: f32, is_product: bool, m: vec2f };
fn march(ro: vec3f, rd: vec3f) -> Hit {
  var t = 0.0;
  var h: Hit;
  h.t = -1.0;
  for (var i = 0; i < 200; i = i + 1) {
    let p = ro + rd * t;
    let dp = product(p);
    let m = mat;
    let ds = studio(p);
    let d = min(dp, ds);
    if (d < 0.0004 * t + 0.0002) {
      h.t = t;
      h.is_product = dp < ds;
      h.m = m;
      return h;
    }
    t = t + d * 0.9;
    if (t > 30.0) { break; }
  }
  return h;
}

fn shade_pixel(uv: vec2f) -> vec4f {
  // uv 0..1, (0,0) – lewy górny róg; przesunięcie kadru = nierówne kadrowanie „z aparatu”.
  let aspect = shot.size.x / shot.size.y;
  let ndc = vec2f((uv.x * 2.0 - 1.0) * aspect, 1.0 - uv.y * 2.0) + shot.shift;
  let fw = normalize(shot.cam_target - shot.cam_pos);
  let rt = normalize(cross(fw, vec3f(0.0, 1.0, 0.0)));
  let up = cross(rt, fw);
  let f = 1.0 / tan(shot.fov * 0.5);
  let rd = normalize(fw * f + rt * ndc.x + up * ndc.y);
  let ro = shot.cam_pos;
  let hit = march(ro, rd);
  let key_dir = normalize(vec3f(-0.55, 0.85, 0.6));
  let fill_dir = normalize(vec3f(0.8, 0.4, 0.3));
  let mode = i32(shot.mode + 0.5);
  if (hit.t < 0.0) {
    return vec4f(select(tonemap(shot.bg_dark), vec3f(1.0), mode == 1), 0.0);
  }
  let p = ro + rd * hit.t;
  let n = normal_at(p);
  let sh = soft_shadow(p + n * 0.003, key_dir, 10.0);
  if (!hit.is_product) {
    // tło studia: gradient jaśniejszy za produktem (miękkie światło), cień kontaktowy
    let g = clamp(1.0 - length((uv - vec2f(0.5 + shot.shift.x * 0.25, 0.45)) * vec2f(0.9, 1.1)), 0.0, 1.0);
    let base = mix(shot.bg_dark, shot.bg_light, smoothstep(0.0, 1.0, g));
    let occ = ao(p, n);
    let shadow = mix(0.55, 1.0, sh) * mix(0.6, 1.0, occ);
    if (mode == 2) { return vec4f(0.0, 0.0, 0.0, 1.0); }
    if (mode == 1) {
      // po usunięciu tła: białe tło, zostaje miękki cień pod produktem
      let s = (1.0 - shadow) * smoothstep(1.4, 0.2, length(p.xz));
      return vec4f(vec3f(1.0 - s * 0.55), 0.0);
    }
    return vec4f(tonemap(base * shadow), 0.0);
  }
  if (mode == 2) { return vec4f(1.0, 1.0, 1.0, 1.0); }
  let albedo = product_color(shot.product, hit.m.x);
  let gloss = hit.m.y;
  let v = -rd;
  let occ = ao(p, n);
  let dif_k = max(dot(n, key_dir), 0.0) * mix(0.15, 1.0, sh);
  let dif_f = max(dot(n, fill_dir), 0.0) * 0.35;
  let amb = (0.5 + 0.5 * n.y) * 0.32 * occ;
  let h = normalize(key_dir + v);
  let spec_pow = mix(16.0, 180.0, gloss);
  let spec = pow(max(dot(n, h), 0.0), spec_pow) * gloss * 1.4 * sh;
  let fres = pow(1.0 - max(dot(n, v), 0.0), 4.0) * 0.35 * gloss;
  let rim = pow(1.0 - max(dot(n, v), 0.0), 2.0) * max(dot(n, normalize(vec3f(0.3, 0.4, -1.0))), 0.0) * 0.4;
  var col = albedo * (dif_k * 1.05 + dif_f + amb) + vec3f(spec + fres + rim * 0.5);
  if (i32(shot.product + 0.5) == 1 && hit.m.x > 3.5) { col = vec3f(1.6, 1.35, 0.9); } // żarówka
  return vec4f(tonemap(col), 1.0);
}

fn tonemap(c: vec3f) -> vec3f {
  // ACES (przybliżenie Narkowicza) + gamma sRGB robi format docelowy
  let a = c * 0.92;
  return clamp((a * (2.51 * a + 0.03)) / (a * (2.43 * a + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}

@fragment fn fs(in: VOut) -> @location(0) vec4f {
  // 2×2 nadpróbkowanie – czyste krawędzie (zdjęcia liczymy raz, przy starcie)
  let px = 1.0 / shot.size;
  var acc = vec4f(0.0);
  for (var j = 0; j < 2; j = j + 1) {
    for (var i = 0; i < 2; i = i + 1) {
      let o = (vec2f(f32(i), f32(j)) - 0.5) * 0.5 * px;
      acc = acc + shade_pixel(in.uv + o);
    }
  }
  acc = acc * 0.25;
  let mode = i32(shot.mode + 0.5);
  if (mode == 2) { return vec4f(acc.rrr, 1.0); }
  // delikatny szum matrycy tylko na „surowym” zdjęciu
  var col = acc.rgb;
  if (mode == 0) {
    let n = fract(sin(dot(in.uv * shot.size + shot.seed, vec2f(12.9898, 78.233))) * 43758.5453) - 0.5;
    col = col + vec3f(n * 0.012);
  }
  return vec4f(col, 1.0);
}
