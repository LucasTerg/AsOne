//! Niedestrukcyjne parametry edycji i operacje na masce (wspólne dla GUI, CLI i MCP).

/// Indeksy w `EditParams::margins` (kolejność jak w CSS: góra, prawo, dół, lewo).
pub const TOP: usize = 0;
pub const RIGHT: usize = 1;
pub const BOTTOM: usize = 2;
pub const LEFT: usize = 3;

pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Niedestrukcyjne parametry edycji jednego zdjęcia.
#[derive(Clone, Debug, PartialEq)]
pub struct EditParams {
    /// Kadr w znormalizowanych współrzędnych źródła: `[lewo, góra, prawo, dół]`, zakres 0..=1.
    pub crop: [f32; 4],
    /// Marginesy w pikselach obrazu źródłowego: `[góra, prawo, dół, lewo]`.
    pub margins: [u32; 4],
    /// Kolor tła pod usuniętymi obszarami i w marginesach (`None` = przezroczystość).
    pub bg_color: Option<[f32; 4]>,
    /// Czy maska (usunięte tło / gumka) jest stosowana.
    pub use_bg_removal: bool,
}

impl Default for EditParams {
    fn default() -> Self {
        Self {
            crop: [0.0, 0.0, 1.0, 1.0],
            margins: [0; 4],
            bg_color: Some(WHITE),
            use_bg_removal: false,
        }
    }
}

impl EditParams {
    /// Kadr w pikselach źródła (zaokrąglony do pełnych pikseli, min. 1×1).
    pub fn crop_px(&self, src: [u32; 2]) -> PxRect {
        let axis = |lo: f32, hi: f32, len: u32| -> (u32, u32) {
            let len_f = len as f32;
            let a = (lo.clamp(0.0, 1.0) * len_f).round() as u32;
            let b = (hi.clamp(0.0, 1.0) * len_f).round() as u32;
            let (a, b) = (a.min(b), a.max(b));
            let b = b.clamp(1, len.max(1));
            let a = a.min(b - 1);
            (a, b)
        };
        let (x0, x1) = axis(self.crop[0], self.crop[2], src[0]);
        let (y0, y1) = axis(self.crop[1], self.crop[3], src[1]);
        PxRect { x0, y0, x1, y1 }
    }

    /// Ustawia kadr z prostokąta w pikselach źródła.
    pub fn set_crop_px(&mut self, rect: PxRect, src: [u32; 2]) {
        let (w, h) = (src[0].max(1) as f32, src[1].max(1) as f32);
        self.crop = [
            rect.x0 as f32 / w,
            rect.y0 as f32 / h,
            rect.x1 as f32 / w,
            rect.y1 as f32 / h,
        ];
    }

    pub fn crop_is_full(&self, src: [u32; 2]) -> bool {
        let c = self.crop_px(src);
        c.x0 == 0 && c.y0 == 0 && c.x1 == src[0] && c.y1 == src[1]
    }

    /// Rozmiar obrazu wynikowego (kadr + marginesy) w pikselach, przed ewentualnym zmniejszeniem.
    pub fn output_size(&self, src: [u32; 2]) -> [u32; 2] {
        let c = self.crop_px(src);
        let m = self.margins;
        [
            c.width() + m[LEFT] + m[RIGHT],
            c.height() + m[TOP] + m[BOTTOM],
        ]
    }

    /// „Dopasuj do kwadratu 1:1”: dokłada marginesy na krótszym boku, aż wynik będzie kwadratem.
    pub fn fit_square(&mut self, src: [u32; 2]) {
        let [w, h] = self.output_size(src);
        if w > h {
            let d = w - h;
            self.margins[TOP] += d / 2;
            self.margins[BOTTOM] += d - d / 2;
        } else if h > w {
            let d = h - w;
            self.margins[LEFT] += d / 2;
            self.margins[RIGHT] += d - d / 2;
        }
    }
}

/// Prostokąt w pikselach: `[x0, x1) × [y0, y1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl PxRect {
    pub fn width(&self) -> u32 {
        self.x1.saturating_sub(self.x0)
    }

    pub fn height(&self) -> u32 {
        self.y1.saturating_sub(self.y0)
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn union(self, other: PxRect) -> PxRect {
        PxRect {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    /// Prostokąt rozpięty między dwoma punktami (w px, ułamkowo), przycięty do `bounds`.
    pub fn from_points(a: [f32; 2], b: [f32; 2], bounds: [u32; 2]) -> Option<PxRect> {
        let clamp = |v: f32, max: u32| v.clamp(0.0, max as f32);
        let x0 = clamp(a[0].min(b[0]), bounds[0]).floor() as u32;
        let y0 = clamp(a[1].min(b[1]), bounds[1]).floor() as u32;
        let x1 = clamp(a[0].max(b[0]), bounds[0]).ceil() as u32;
        let y1 = clamp(a[1].max(b[1]), bounds[1]).ceil() as u32;
        let r = PxRect { x0, y0, x1, y1 };
        (!r.is_empty()).then_some(r)
    }
}

pub fn union_opt(a: Option<PxRect>, b: Option<PxRect>) -> Option<PxRect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// Odbija okrągły „stempel” pędzla w masce. `restore = false` usuwa (maluje 0),
/// `restore = true` przywraca (maluje 255). Krawędź jest wygładzona na ~1 px.
/// Zwraca zmieniony obszar (*dirty rect*).
pub fn stamp_circle(
    mask: &mut [u8],
    size: [u32; 2],
    center: [f32; 2],
    radius: f32,
    restore: bool,
) -> Option<PxRect> {
    let [w, h] = size;
    let r = radius.max(0.5);
    let x0 = (center[0] - r - 1.0).floor().clamp(0.0, w as f32) as u32;
    let y0 = (center[1] - r - 1.0).floor().clamp(0.0, h as f32) as u32;
    let x1 = (center[0] + r + 1.0).ceil().clamp(0.0, w as f32) as u32;
    let y1 = (center[1] + r + 1.0).ceil().clamp(0.0, h as f32) as u32;
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    for y in y0..y1 {
        let dy = y as f32 + 0.5 - center[1];
        let row = (y as usize) * (w as usize);
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - center[0];
            let coverage = (r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }
            let v = (coverage * 255.0).round() as u8;
            let px = &mut mask[row + x as usize];
            *px = if restore { (*px).max(v) } else { (*px).min(255 - v) };
        }
    }
    Some(PxRect { x0, y0, x1, y1 })
}

/// Pociągnięcie pędzlem od `from` do `to` (stemple co ¼ promienia, żeby nie było przerw).
pub fn stamp_segment(
    mask: &mut [u8],
    size: [u32; 2],
    from: [f32; 2],
    to: [f32; 2],
    radius: f32,
    restore: bool,
) -> Option<PxRect> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let step = (radius * 0.25).max(0.5);
    let steps = (len / step).ceil() as usize;
    let mut dirty = None;
    for i in 0..=steps {
        let t = if steps == 0 { 1.0 } else { i as f32 / steps as f32 };
        let p = [from[0] + dx * t, from[1] + dy * t];
        dirty = union_opt(dirty, stamp_circle(mask, size, p, radius, restore));
    }
    dirty
}

/// Wypełnia prostokąt maski jedną wartością (wycinanie / przywracanie zaznaczenia).
pub fn fill_rect(mask: &mut [u8], size: [u32; 2], rect: PxRect, value: u8) -> Option<PxRect> {
    let r = PxRect {
        x0: rect.x0.min(size[0]),
        y0: rect.y0.min(size[1]),
        x1: rect.x1.min(size[0]),
        y1: rect.y1.min(size[1]),
    };
    if r.is_empty() {
        return None;
    }
    let w = size[0] as usize;
    for y in r.y0..r.y1 {
        let row = y as usize * w;
        mask[row + r.x0 as usize..row + r.x1 as usize].fill(value);
    }
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kadr_i_rozmiar_wyniku() {
        let mut e = EditParams::default();
        assert_eq!(e.output_size([4000, 3000]), [4000, 3000]);
        e.crop = [0.25, 0.0, 0.75, 1.0];
        e.margins = [10, 20, 30, 40];
        assert_eq!(e.crop_px([4000, 3000]), PxRect { x0: 1000, y0: 0, x1: 3000, y1: 3000 });
        assert_eq!(e.output_size([4000, 3000]), [2000 + 40 + 20, 3000 + 10 + 30]);
    }

    #[test]
    fn kadr_nigdy_nie_jest_pusty() {
        let e = EditParams { crop: [0.5, 0.5, 0.5, 0.5], ..Default::default() };
        let c = e.crop_px([100, 100]);
        assert_eq!((c.width(), c.height()), (1, 1));
        let e = EditParams { crop: [1.0, 1.0, 1.0, 1.0], ..Default::default() };
        let c = e.crop_px([100, 100]);
        assert_eq!(c, PxRect { x0: 99, y0: 99, x1: 100, y1: 100 });
    }

    #[test]
    fn kwadrat_1_1() {
        let mut e = EditParams { margins: [0, 5, 0, 5], ..Default::default() };
        e.fit_square([3000, 2001]);
        assert_eq!(e.output_size([3000, 2001]), [3010, 3010]);
        assert_eq!(e.margins[TOP] + e.margins[BOTTOM], 1009);

        let mut e = EditParams::default();
        e.fit_square([1000, 1500]);
        assert_eq!(e.output_size([1000, 1500]), [1500, 1500]);
        assert_eq!((e.margins[LEFT], e.margins[RIGHT]), (250, 250));
    }

    #[test]
    fn gumka_i_przywracanie() {
        let size = [20, 20];
        let mut mask = vec![255u8; 400];
        let dirty = stamp_circle(&mut mask, size, [10.0, 10.0], 3.0, false).unwrap();
        assert_eq!(mask[10 * 20 + 10], 0);
        assert_eq!(mask[0], 255);
        assert!(dirty.x0 <= 7 && dirty.x1 >= 13);
        stamp_circle(&mut mask, size, [10.0, 10.0], 5.0, true);
        assert_eq!(mask[10 * 20 + 10], 255);
    }

    #[test]
    fn pedzel_poza_obrazem() {
        let mut mask = vec![255u8; 100];
        assert!(stamp_circle(&mut mask, [10, 10], [-50.0, -50.0], 3.0, false).is_none());
        assert!(mask.iter().all(|&v| v == 255));
    }

    #[test]
    fn pociagniecie_bez_przerw() {
        let size = [100, 10];
        let mut mask = vec![255u8; 1000];
        stamp_segment(&mut mask, size, [5.0, 5.0], [95.0, 5.0], 2.0, false);
        assert!((5..95).all(|x| mask[5 * 100 + x] == 0));
    }

    #[test]
    fn prostokat_z_punktow_i_wypelnienie() {
        let r = PxRect::from_points([8.5, 9.2], [-3.0, 2.0], [10, 10]).unwrap();
        assert_eq!(r, PxRect { x0: 0, y0: 2, x1: 9, y1: 10 });
        assert!(PxRect::from_points([3.0, 3.0], [3.0, 8.0], [10, 10]).is_none());

        let mut mask = vec![255u8; 100];
        fill_rect(&mut mask, [10, 10], PxRect { x0: 2, y0: 2, x1: 4, y1: 3 }, 0);
        assert_eq!(mask.iter().filter(|&&v| v == 0).count(), 2);
    }
}
