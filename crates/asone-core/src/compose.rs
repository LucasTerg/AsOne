//! Kompozycja obrazu wynikowego na CPU – ta sama semantyka co shader `wgpu` w trybie eksportu:
//! kadr źródła + marginesy, piksele łączone z tłem przez maskę: `mix(bg, src, alpha)`.
//!
//! Używana przez CLI/MCP (bez GPU) i przez wersję przeglądarkową (gdzie odczyt z GPU jest asynchroniczny).

use image::RgbaImage;

use crate::edits::{EditParams, LEFT, TOP, WHITE};
use crate::imageio::{lanczos3, resize_rgba};

/// Nakłada warstwę (z przezroczystością) na obraz wynikowy RGB8: `rect` = `[x, y, szer., wys.]`
/// w pikselach obrazu; części poza obrazem są obcinane.
pub fn draw_overlay(size: [u32; 2], rgb: &mut [u8], overlay: &RgbaImage, rect: [f32; 4]) -> Result<(), String> {
    let (w, h) = (rect[2].round() as i64, rect[3].round() as i64);
    if w < 1 || h < 1 {
        return Ok(());
    }
    let (x0, y0) = (rect[0].round() as i64, rect[1].round() as i64);
    let (cw, ch) = (size[0] as i64, size[1] as i64);
    // Część warstwy widoczna w obrazie.
    let (vx0, vy0, vx1, vy1) = (x0.max(0), y0.max(0), (x0 + w).min(cw), (y0 + h).min(ch));
    if vx0 >= vx1 || vy0 >= vy1 {
        return Ok(());
    }
    let scaled = resize_rgba(overlay, [w as u32, h as u32], lanczos3())?;
    for y in vy0..vy1 {
        for x in vx0..vx1 {
            let p = scaled.get_pixel((x - x0) as u32, (y - y0) as u32);
            let a = p[3] as u32;
            if a == 0 {
                continue;
            }
            let i = ((y * cw + x) * 3) as usize;
            for c in 0..3 {
                rgb[i + c] = ((p[c] as u32 * a + rgb[i + c] as u32 * (255 - a) + 127) / 255) as u8;
            }
        }
    }
    Ok(())
}

/// Zwraca rozmiar i piksele RGB8 obrazu wynikowego. `mask` (1 bajt/piksel źródła) jest
/// stosowana tylko gdy `edits.use_bg_removal`. Bez wybranego tła spłaszcza na biało (JPEG).
pub fn compose_rgb(src: &RgbaImage, mask: Option<&[u8]>, edits: &EditParams) -> ([u32; 2], Vec<u8>) {
    let s = [src.width(), src.height()];
    let crop = edits.crop_px(s);
    let [w, h] = edits.output_size(s);
    let bg = edits.bg_color.unwrap_or(WHITE);
    let bg = [0, 1, 2].map(|i| (bg[i].clamp(0.0, 1.0) * 255.0).round() as u32);
    let mask = mask.filter(|m| edits.use_bg_removal && m.len() == s[0] as usize * s[1] as usize);

    let mut out = Vec::with_capacity(w as usize * h as usize * 3);
    for _ in 0..w as usize * h as usize {
        out.extend_from_slice(&[bg[0] as u8, bg[1] as u8, bg[2] as u8]);
    }
    let (ml, mt) = (edits.margins[LEFT] as usize, edits.margins[TOP] as usize);
    let raw = src.as_raw();
    for y in 0..crop.height() as usize {
        let sy = crop.y0 as usize + y;
        let dst_row = ((mt + y) * w as usize + ml) * 3;
        for x in 0..crop.width() as usize {
            let sx = crop.x0 as usize + x;
            let si = sy * s[0] as usize + sx;
            let p = &raw[si * 4..si * 4 + 3];
            let d = &mut out[dst_row + x * 3..dst_row + x * 3 + 3];
            match mask.map(|m| m[si] as u32) {
                None | Some(255) => d.copy_from_slice(p),
                Some(a) => {
                    for c in 0..3 {
                        d[c] = ((p[c] as u32 * a + bg[c] * (255 - a) + 127) / 255) as u8;
                    }
                }
            }
        }
    }
    ([w, h], out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edits::{BOTTOM, RIGHT};

    #[test]
    fn kadr_marginesy_maska() {
        // 4×2: lewa połowa czerwona, prawa zielona
        let mut src = RgbaImage::new(4, 2);
        for (x, _, p) in src.enumerate_pixels_mut() {
            *p = if x < 2 { image::Rgba([255, 0, 0, 255]) } else { image::Rgba([0, 255, 0, 255]) };
        }
        let mut e = EditParams { crop: [0.25, 0.0, 1.0, 1.0], ..Default::default() };
        e.margins[TOP] = 1;
        e.margins[LEFT] = 1;
        e.margins[RIGHT] = 0;
        e.margins[BOTTOM] = 0;
        let (size, rgb) = compose_rgb(&src, None, &e);
        assert_eq!(size, [4, 3]);
        let px = |x: usize, y: usize| &rgb[(y * 4 + x) * 3..(y * 4 + x) * 3 + 3];
        assert_eq!(px(0, 0), &[255, 255, 255]); // margines
        assert_eq!(px(0, 1), &[255, 255, 255]); // lewy margines
        assert_eq!(px(1, 1), &[255, 0, 0]); // x=1 źródła
        assert_eq!(px(2, 2), &[0, 255, 0]);

        // maska: piksel (1,0) usunięty → tło; tylko gdy use_bg_removal
        let mut mask = vec![255u8; 8];
        mask[1] = 0;
        assert_eq!(compose_rgb(&src, Some(&mask), &e).1, rgb);
        e.use_bg_removal = true;
        e.bg_color = Some([0.0, 0.0, 1.0, 1.0]);
        let (_, rgb) = compose_rgb(&src, Some(&mask), &e);
        let px = |x: usize, y: usize| rgb[(y * 4 + x) * 3..(y * 4 + x) * 3 + 3].to_vec();
        assert_eq!(px(1, 1), vec![0, 0, 255]);
        assert_eq!(px(0, 0), vec![0, 0, 255]);
        assert_eq!(px(2, 1), vec![0, 255, 0]);
    }

    #[test]
    fn warstwa_z_przezroczystoscia_i_obcieciem() {
        let size = [4, 2];
        let mut rgb = vec![255u8; 4 * 2 * 3];
        // 2×1: czerwony nieprzezroczysty + przezroczysty; skala ×1, częściowo poza obrazem z prawej.
        let mut ov = RgbaImage::new(2, 1);
        ov.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        ov.put_pixel(1, 0, image::Rgba([0, 0, 255, 0]));
        draw_overlay(size, &mut rgb, &ov, [3.0, 1.0, 2.0, 1.0]).unwrap();
        let px = |x: usize, y: usize| rgb[(y * 4 + x) * 3..(y * 4 + x) * 3 + 3].to_vec();
        assert_eq!(px(3, 1), vec![255, 0, 0]);
        assert_eq!(px(2, 1), vec![255, 255, 255]);
        assert_eq!(px(3, 0), vec![255, 255, 255]);
        // Całkiem poza obrazem – nic.
        draw_overlay(size, &mut rgb, &ov, [10.0, 10.0, 2.0, 1.0]).unwrap();
    }

    #[test]
    fn polprzezroczystosc() {
        let src = RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
        let e = EditParams { use_bg_removal: true, ..Default::default() };
        let (_, rgb) = compose_rgb(&src, Some(&[128]), &e);
        assert_eq!(rgb, vec![127, 127, 127]);
    }
}
