// Wspólny shader podglądu (płótno w egui) i eksportu (renderowanie offscreen).
//
// Rysujemy jeden trójkąt pokrywający cały viewport. Każdy fragment zamienia swoją
// pozycję w buforze ramki na współrzędne obrazu wynikowego:
//
//     img = (frag_px - origin_px) / zoom_px
//
// Obraz wynikowy = kadr źródła + marginesy. Piksele kadru łączone są z maską i tłem:
//     rgb = mix(bg.rgb, src.rgb, alpha)   gdzie alpha = maska (0 w marginesach).

struct Params {
    // xy: pozycja piksela (0,0) obrazu wynikowego w px bufora; z: px bufora na px obrazu; w: poziom mip
    view: vec4<f32>,
    // xy: rozmiar obrazu wynikowego; zw: rozmiar tekstury źródłowej
    sizes: vec4<f32>,
    // xy: początek kadru (px źródła); zw: rozmiar kadru
    crop: vec4<f32>,
    // x: lewy margines; y: górny margines; z: bok kratki szachownicy (px bufora); w: 1 = próbkowanie nearest
    margin: vec4<f32>,
    // kolor tła (białe tło)
    bg: vec4<f32>,
    // kolor płótna wokół obrazu (tylko podgląd)
    canvas: vec4<f32>,
    // x: tło włączone; y: maska włączona; z: tryb podglądu; w: cel w formacie sRGB
    flags: vec4<f32>,
    // x: inspekcja tła (tylko podgląd)
    extra: vec4<f32>,
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var src_tex: texture_2d<f32>;
@group(0) @binding(2) var mask_tex: texture_2d<f32>;
// Jeden sampler (liniowy) na wszystkie tekstury: w GLSL/WebGL2 tekstura nie może być
// próbkowana dwoma różnymi samplerami. „Nearest” przy dużym zoomie = UV przyciągnięte do środka teksela.
@group(0) @binding(3) var samp_linear: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    // (0,0), (2,0), (0,2) – trójkąt przykrywający cały viewport
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var out: VsOut;
    out.pos = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn checker(frag: vec2<f32>) -> vec3<f32> {
    let cell = floor(frag / max(p.margin.z, 1.0));
    let odd = (i32(cell.x) + i32(cell.y)) & 1;
    return select(vec3<f32>(1.0), vec3<f32>(0.8), odd == 1);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let frag = in.pos.xy;
    let img = (frag - p.view.xy) / p.view.z;
    let out_size = p.sizes.xy;
    let src_size = p.sizes.zw;
    let preview = p.flags.z > 0.5;

    // Współrzędne w kadrze i w teksturze źródłowej.
    let in_crop = img - p.margin.xy;
    let uv = (p.crop.xy + in_crop) / src_size;

    var suv = uv;
    if (p.margin.w > 0.5) {
        suv = (floor(uv * src_size) + vec2<f32>(0.5)) / src_size;
    }
    let color = textureSampleLevel(src_tex, samp_linear, suv, p.view.w);
    let m = textureSampleLevel(mask_tex, samp_linear, suv, 0.0).r;

    let inside_out = all(img >= vec2<f32>(0.0)) && all(img < out_size);
    let inside_crop = all(in_crop >= vec2<f32>(0.0)) && all(in_crop < p.crop.zw);

    var alpha = select(0.0, 1.0, inside_crop);
    if (p.flags.y > 0.5) {
        alpha = alpha * m;
    }

    var rgb: vec3<f32>;
    var a: f32 = 1.0;
    if (p.flags.x > 0.5) {
        rgb = mix(p.bg.rgb, color.rgb, alpha);
    } else if (preview) {
        rgb = mix(checker(frag), color.rgb, alpha);
    } else {
        // Eksport bez tła: kolor + kanał alfa (spłaszczany dalej po stronie CPU).
        rgb = color.rgb;
        a = alpha;
    }

    // Inspekcja tła: ekstremalny kontrast przy bieli (widać resztki tła i nierówną biel).
    // Czysta biel zostaje biała, każdy „prawie biały” piksel (cień, kurz, obwódka po wycięciu)
    // staje się wyraźnie szary, a obiekt – czarną sylwetką.
    if (preview && p.extra.x > 0.5) {
        rgb = clamp((rgb - vec3<f32>(0.90)) / 0.10, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    if (preview && !inside_out) {
        rgb = p.canvas.rgb;
        a = 1.0;
    }

    // Tekstury trzymają wartości gamma (Rgba8Unorm). Jeśli cel jest sRGB,
    // sprzęt zakoduje gammę sam – wtedy podajemy wartości liniowe.
    if (p.flags.w > 0.5) {
        rgb = srgb_to_linear(rgb);
    }
    return vec4<f32>(rgb, a);
}
