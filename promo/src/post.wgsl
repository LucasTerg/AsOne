// Postproces: (1) jasne miejsca do poświaty w 1/4 rozdzielczości, (2) rozmycie poświaty,
// (3) głębia ostrości z mapy odległości, (4) rozmycie ruchu kamery + poświata + tony + winieta + napisy.

struct Post {
  inv_view_proj: mat4x4f,
  prev_view_proj: mat4x4f,
  cam_pos: vec3f, focus: f32,
  size: vec2f, aperture: f32, max_coc: f32,
  bloom: f32, exposure: f32, grain: f32, time: f32,
  mblur: f32, fade: f32, vignette: f32, overlay_alpha: f32,
};
@group(0) @binding(0) var<uniform> pp: Post;
@group(0) @binding(1) var lin: sampler;
@group(0) @binding(2) var hdr: texture_2d<f32>;
@group(0) @binding(3) var depth: texture_2d<f32>;
@group(0) @binding(4) var aux: texture_2d<f32>;      // poświata albo wynik DOF
@group(0) @binding(5) var aux2: texture_2d<f32>;     // poświata (ostatnie przejście)

struct VOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
  var p = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
  var o: VOut; o.pos = vec4f(p[i], 0.0, 1.0); o.uv = p[i] * vec2f(0.5, -0.5) + 0.5; return o;
}

// (1) jasne miejsca: 4×4 próbki z rozdzielczości pełnej, miękki próg
@fragment fn fs_bright(in: VOut) -> @location(0) vec4f {
  let px = 1.0 / pp.size;
  var c = vec3f(0.0);
  for (var j = -1; j <= 2; j = j + 1) {
    for (var i = -1; i <= 2; i = i + 1) {
      c = c + textureSampleLevel(hdr, lin, in.uv + vec2f(f32(i) - 0.5, f32(j) - 0.5) * px, 0.0).rgb;
    }
  }
  c = c / 16.0;
  let l = max(c.r, max(c.g, c.b));
  let k = smoothstep(1.05, 2.2, l);
  return vec4f(c * k, 1.0);
}
// (2) rozmycie poświaty: 9 próbek gaussa w osi (kierunek w pp.vignette – tylko w tym przejściu)
struct Blur { dir: vec2f, pad: vec2f };
@group(1) @binding(0) var<uniform> bl: Blur;
@fragment fn fs_blur(in: VOut) -> @location(0) vec4f {
  let w = array<f32, 5>(0.227, 0.194, 0.121, 0.054, 0.016);
  var c = textureSampleLevel(aux, lin, in.uv, 0.0).rgb * w[0];
  for (var i = 1; i < 5; i = i + 1) {
    let o = bl.dir * f32(i) * 1.6;
    c = c + (textureSampleLevel(aux, lin, in.uv + o, 0.0).rgb + textureSampleLevel(aux, lin, in.uv - o, 0.0).rgb) * w[i];
  }
  return vec4f(c, 1.0);
}

fn coc_px(d: f32) -> f32 {
  return clamp(pp.aperture * abs(d - pp.focus) / max(d, 0.05) * pp.size.y, 0.0, pp.max_coc);
}
// (3) głębia ostrości: dysk Vogela 28 próbek; próbka wnosi tyle, ile jej własny krąg rozmycia sięga
@fragment fn fs_dof(in: VOut) -> @location(0) vec4f {
  let c0 = textureSampleLevel(hdr, lin, in.uv, 0.0);
  let d0 = textureSampleLevel(depth, lin, in.uv, 0.0).r;
  let r0 = coc_px(d0);
  if (r0 < 0.6) { return vec4f(c0.rgb, r0); }
  var acc = c0.rgb;
  var wsum = 1.0;
  let golden = 2.39996323;
  let px = 1.0 / pp.size;
  for (var i = 1; i < 28; i = i + 1) {
    let r = sqrt(f32(i) / 28.0) * r0;
    let a = f32(i) * golden;
    let uv = in.uv + vec2f(cos(a), sin(a)) * r * px;
    let s = textureSampleLevel(hdr, lin, uv, 0.0).rgb;
    let ds = textureSampleLevel(depth, lin, uv, 0.0).r;
    let rs = coc_px(ds);
    // tło nie przenika na ostry pierwszy plan; pierwszy plan może rozlewać się na tło
    let w = select(smoothstep(r - 1.0, r + 1.0, rs), 1.0, ds > d0);
    acc = acc + s * w;
    wsum = wsum + w;
  }
  return vec4f(acc / wsum, r0);
}

// krzywa tonalna: do 0.9 bez zmian (kolory interfejsu i zdjęć jak w programie), wyżej miękkie ściskanie poświat
fn knee(x: vec3f) -> vec3f {
  let k = 0.95;
  return select(x, k + (1.0 - k) * (1.0 - exp(-(x - k) / (1.0 - k))), x > vec3f(k));
}
// napisy i plansza końcowa: prostokąty z atlasu rysowane wprost tutaj (bez kopiowania nakładki na klatkę)
struct Over { r: array<vec4f, 10>, k: vec4f };  // pary: prostokąt ekranu (uv), prostokąt atlasu (uv); k: alfa napisu, alfa planszy, góra cienia, –
@group(1) @binding(1) var atlas_t: texture_2d<f32>;
@group(1) @binding(2) var<uniform> ov: Over;
// (4) rozmycie ruchu kamery (rekonstrukcja położenia z odległości) + poświata + wynik końcowy
@fragment fn fs_final(in: VOut) -> @location(0) vec4f {
  let d = textureSampleLevel(depth, lin, in.uv, 0.0).r;
  let ndc = vec2f(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
  let w = pp.inv_view_proj * vec4f(ndc, 1.0, 1.0);
  let dir = normalize(w.xyz / w.w - pp.cam_pos);
  let world = pp.cam_pos + dir * min(d, 400.0);
  let prev = pp.prev_view_proj * vec4f(world, 1.0);
  let prev_uv = vec2f(prev.x / prev.w * 0.5 + 0.5, 0.5 - prev.y / prev.w * 0.5);
  var vel = (in.uv - prev_uv) * pp.mblur;
  let vmax = 40.0 / pp.size.x;
  let vl = length(vel);
  if (vl > vmax) { vel = vel * (vmax / vl); }
  var col = vec3f(0.0);
  let n = 10;
  let jitter = fract(sin(dot(in.uv * pp.size + pp.time, vec2f(12.9898, 78.233))) * 43758.5453) - 0.5;
  for (var i = 0; i < n; i = i + 1) {
    let t = (f32(i) + 0.5 + jitter * 0.5) / f32(n) - 0.5;
    col = col + textureSampleLevel(aux, lin, in.uv - vel * t, 0.0).rgb;
  }
  col = col / f32(n);
  col = col + textureSampleLevel(aux2, lin, in.uv, 0.0).rgb * pp.bloom;
  col = knee(col * pp.exposure);
  // winieta i ziarno filmowe (stałe w czasie – bez migotania na płaskich polach)
  let v = in.uv - 0.5;
  col = col * (1.0 - dot(v, v) * pp.vignette);
  let g = fract(sin(dot(floor(in.uv * pp.size), vec2f(12.9898, 78.233)) + floor(pp.time * 24.0) * 7.13) * 43758.5453) - 0.5;
  col = col + vec3f(g * pp.grain);
  col = col * pp.fade;
  // cień pod napisem (miękki gradient do dołu kadru)
  let gt = ov.k.z;
  let gk = smoothstep(gt, gt + (1.0 - gt) * 0.45, in.uv.y) * 0.5 + smoothstep(gt + (1.0 - gt) * 0.45, 1.0, in.uv.y) * 0.22;
  col = col * (1.0 - gk * ov.k.x * 0.0);
  let cr = ov.r[0];
  let pad = vec2f(0.012 * pp.size.y / pp.size.x, 0.012);
  let dq = max(max(cr.xy - pad - in.uv, in.uv - cr.zw - pad), vec2f(0.0)) * pp.size;
  col = col * (1.0 - 0.85 * ov.k.x * (1.0 - smoothstep(0.0, 26.0, length(dq))) * step(0.0001, cr.z - cr.x));
  var o = vec4f(0.0);
  for (var i = 0; i < 5; i = i + 1) {
    let sr = ov.r[i * 2]; let ar = ov.r[i * 2 + 1];
    let q = (in.uv - sr.xy) / max(sr.zw - sr.xy, vec2f(1e-6));
    let a = select(ov.k.y, ov.k.x, i == 0);
    let lod = log2(max(1.0, (ar.z - ar.x) * 2048.0 / max((sr.z - sr.x) * pp.size.x, 1.0)));
    let s = textureSampleLevel(atlas_t, lin, mix(ar.xy, ar.zw, clamp(q, vec2f(0.0), vec2f(1.0))), lod);
    if (all(q >= vec2f(0.0)) && all(q <= vec2f(1.0)) && a > 0.0) { o = s * a + o * (1.0 - s.a * a); }
  }
  col = col * (1.0 - o.a) + o.rgb;
  // dokładne kodowanie sRGB (tekstury czytane jako *-srgb) – szarości interfejsu wychodzą 1:1
  let c = clamp(col, vec3f(0.0), vec3f(1.0));
  return vec4f(select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3f(0.0031308)), 1.0);
}
