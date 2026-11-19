//! „Kadruj (auto)” z Asystenta PIM: kadr do obiektu na jasnym, jednolitym tle.
//!
//! Piksel należy do obiektu, gdy jego jasność (jak `L` w PIL) jest poniżej progu (domyślnie 240)
//! i nie jest przezroczysty. Jeśli maska jest aktywna (usunięte tło), liczy się sama maska.

use image::RgbaImage;

use crate::edits::PxRect;

pub const DEFAULT_THRESHOLD: u8 = 240;

pub fn luma(p: [u8; 3]) -> u8 {
    ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8
}

/// Najmniejszy prostokąt zawierający obiekt albo `None`, gdy obiektu nie ma.
pub fn content_bbox(img: &RgbaImage, mask: Option<&[u8]>, threshold: u8) -> Option<PxRect> {
    let (w, h) = img.dimensions();
    let mask = mask.filter(|m| m.len() == w as usize * h as usize);
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        let i = (y * w + x) as usize;
        let content = match mask {
            Some(m) => m[i] > 16,
            None => p[3] > 16 && luma([p[0], p[1], p[2]]) < threshold,
        };
        if content {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    (x1 > x0 && y1 > y0).then_some(PxRect { x0, y0, x1, y1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obiekt_na_bialym_tle() {
        let mut img = RgbaImage::from_pixel(10, 8, image::Rgba([250, 250, 250, 255]));
        img.put_pixel(3, 2, image::Rgba([10, 10, 10, 255]));
        img.put_pixel(6, 5, image::Rgba([200, 100, 50, 255]));
        assert_eq!(content_bbox(&img, None, 240), Some(PxRect { x0: 3, y0: 2, x1: 7, y1: 6 }));
        let blank = RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]));
        assert_eq!(content_bbox(&blank, None, 240), None);
    }

    #[test]
    fn maska_ma_pierwszenstwo() {
        let img = RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 0, 255]));
        let mut mask = vec![0u8; 16];
        mask[4 + 2] = 255; // (x=2, y=1)
        assert_eq!(content_bbox(&img, Some(&mask), 240), Some(PxRect { x0: 2, y0: 1, x1: 3, y1: 2 }));
    }
}
