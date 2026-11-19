//! Historia zmian zdjęcia: Cofnij / Ponów (osobna dla każdego zdjęcia).
//!
//! Krok historii to stan parametrów sprzed zmiany (kadr, marginesy, tło…) oraz – jeśli zmieniła
//! się maska – tylko zmieniony prostokąt maski („łatka”), a nie cała maska. Kopia całej maski
//! powstaje na czas jednej czynności (np. jednego pociągnięcia gumką) i po zatwierdzeniu znika.
//!
//! Zmiany są zatwierdzane, gdy nic nie jest przeciągane ani wpisywane – dzięki temu przeciągnięcie
//! suwaka, kadru czy gumki to jeden krok, a nie setki.

use asone_core::edits::{EditParams, PxRect};

/// Maks. liczba kroków na zdjęcie.
const MAX_STEPS: usize = 100;
/// Maks. pamięć łatek maski na zdjęcie.
const MAX_MASK_BYTES: usize = 192 << 20;

#[derive(Clone, PartialEq)]
struct State {
    edits: EditParams,
    mask_touched: bool,
}

struct MaskPatch {
    rect: PxRect,
    /// Piksele maski w `rect` (wiersz po wierszu) – stan „po drugiej stronie” kroku.
    pixels: Vec<u8>,
}

struct Step {
    state: State,
    mask: Option<MaskPatch>,
}

/// Edytowalne dane zdjęcia, na których działa historia.
pub struct Target<'a> {
    pub edits: &'a mut EditParams,
    pub mask_touched: &'a mut bool,
    pub mask: &'a mut Vec<u8>,
    /// Szerokość maski w pikselach.
    pub width: u32,
}

#[derive(Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// Stan po ostatnim zatwierdzeniu; `None` = jeszcze nie ustalony (np. zaraz po wczytaniu).
    committed: Option<State>,
    /// Kopia maski sprzed bieżącej czynności.
    mask_before: Option<Vec<u8>>,
}

impl History {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Wołać PRZED zmianą maski (gumka, prostokąt, AI, reset maski).
    pub fn begin_mask_change(&mut self, mask: &[u8]) {
        if self.mask_before.is_none() && self.committed.is_some() && !mask.is_empty() {
            self.mask_before = Some(mask.to_vec());
        }
    }

    /// Zatwierdza zmiany od ostatniego kroku. Zwraca `true`, gdy powstał nowy krok.
    pub fn commit(&mut self, edits: &EditParams, mask_touched: bool, mask: &[u8], width: u32) -> bool {
        let Some(committed) = &self.committed else {
            self.committed = Some(State { edits: edits.clone(), mask_touched });
            self.mask_before = None;
            return false;
        };
        let state_changed = committed.edits != *edits || committed.mask_touched != mask_touched;
        if !state_changed && self.mask_before.is_none() {
            return false;
        }
        let patch = self.mask_before.take().and_then(|before| {
            let rect = diff_rect(&before, mask, width as usize)?;
            Some(MaskPatch { rect, pixels: extract(&before, width as usize, rect) })
        });
        if !state_changed && patch.is_none() {
            return false;
        }
        let new = State { edits: edits.clone(), mask_touched };
        let old = self.committed.replace(new).expect("stan zatwierdzony");
        self.undo.push(Step { state: old, mask: patch });
        self.redo.clear();
        self.trim();
        true
    }

    /// Cofa ostatni krok. `Some(obszar maski do odświeżenia)`, gdy było co cofać.
    pub fn undo(&mut self, t: Target<'_>) -> Option<Option<PxRect>> {
        let step = self.undo.pop()?;
        let back = self.apply(step, t);
        self.redo.push(back.0);
        Some(back.1)
    }

    /// Ponawia ostatnio cofnięty krok.
    pub fn redo(&mut self, t: Target<'_>) -> Option<Option<PxRect>> {
        let step = self.redo.pop()?;
        let back = self.apply(step, t);
        self.undo.push(back.0);
        Some(back.1)
    }

    /// Przywraca stan z `step`; zwraca krok odwrotny i obszar maski do odświeżenia.
    fn apply(&mut self, mut step: Step, t: Target<'_>) -> (Step, Option<PxRect>) {
        let current = State { edits: t.edits.clone(), mask_touched: *t.mask_touched };
        let dirty = step.mask.as_mut().map(|p| {
            swap(t.mask, t.width as usize, p);
            p.rect
        });
        *t.edits = step.state.edits.clone();
        *t.mask_touched = step.state.mask_touched;
        self.committed = Some(step.state);
        self.mask_before = None;
        (Step { state: current, mask: step.mask }, dirty)
    }

    fn trim(&mut self) {
        let bytes = |s: &Step| s.mask.as_ref().map_or(0, |p| p.pixels.len());
        let mut total: usize = self.undo.iter().map(bytes).sum();
        let mut drop = 0;
        while self.undo.len() - drop > 1 && (self.undo.len() - drop > MAX_STEPS || total > MAX_MASK_BYTES) {
            total -= bytes(&self.undo[drop]);
            drop += 1;
        }
        self.undo.drain(..drop);
    }
}

/// Najmniejszy prostokąt obejmujący różnice dwóch masek (tej samej wielkości).
fn diff_rect(a: &[u8], b: &[u8], w: usize) -> Option<PxRect> {
    if w == 0 || a.len() != b.len() || !a.len().is_multiple_of(w) {
        return None;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (usize::MAX, 0, usize::MAX, 0);
    for (y, (ra, rb)) in a.chunks_exact(w).zip(b.chunks_exact(w)).enumerate() {
        if ra == rb {
            continue;
        }
        let first = ra.iter().zip(rb).position(|(p, q)| p != q).unwrap_or(0);
        let last = w - 1 - ra.iter().rev().zip(rb.iter().rev()).position(|(p, q)| p != q).unwrap_or(0);
        y0 = y0.min(y);
        y1 = y;
        x0 = x0.min(first);
        x1 = x1.max(last);
    }
    (y0 != usize::MAX).then(|| PxRect { x0: x0 as u32, y0: y0 as u32, x1: x1 as u32 + 1, y1: y1 as u32 + 1 })
}

fn rows(w: usize, r: PxRect) -> impl Iterator<Item = std::ops::Range<usize>> {
    (r.y0 as usize..r.y1 as usize).map(move |y| y * w + r.x0 as usize..y * w + r.x1 as usize)
}

fn extract(mask: &[u8], w: usize, r: PxRect) -> Vec<u8> {
    let mut out = Vec::with_capacity(r.width() as usize * r.height() as usize);
    for range in rows(w, r) {
        out.extend_from_slice(&mask[range]);
    }
    out
}

/// Zamienia piksele maski w prostokącie łatki z zawartością łatki.
fn swap(mask: &mut [u8], w: usize, p: &mut MaskPatch) {
    let rw = p.rect.width() as usize;
    for (range, chunk) in rows(w, p.rect).zip(p.pixels.chunks_exact_mut(rw)) {
        mask[range].swap_with_slice(chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 8;

    struct Doc {
        edits: EditParams,
        touched: bool,
        mask: Vec<u8>,
        h: History,
    }

    impl Doc {
        fn new() -> Self {
            let mut d = Doc { edits: EditParams::default(), touched: false, mask: vec![255; 64], h: History::default() };
            d.commit();
            d
        }
        fn commit(&mut self) -> bool {
            self.h.commit(&self.edits, self.touched, &self.mask, W)
        }
        fn target(&mut self) -> Target<'_> {
            Target { edits: &mut self.edits, mask_touched: &mut self.touched, mask: &mut self.mask, width: W }
        }
        fn undo(&mut self) -> Option<Option<PxRect>> {
            let mut h = std::mem::take(&mut self.h);
            let r = h.undo(self.target());
            self.h = h;
            r
        }
        fn redo(&mut self) -> Option<Option<PxRect>> {
            let mut h = std::mem::take(&mut self.h);
            let r = h.redo(self.target());
            self.h = h;
            r
        }
    }

    #[test]
    fn pierwsze_zatwierdzenie_to_stan_bazowy() {
        let mut d = Doc::new();
        assert!(!d.h.can_undo());
        assert!(!d.commit(), "bez zmian nie ma kroku");
    }

    #[test]
    fn cofnij_i_ponow_parametry() {
        let mut d = Doc::new();
        d.edits.margins[0] = 10;
        assert!(d.commit());
        d.edits.margins[0] = 20;
        assert!(d.commit());
        assert_eq!(d.undo(), Some(None));
        assert_eq!(d.edits.margins[0], 10);
        assert_eq!(d.undo(), Some(None));
        assert_eq!(d.edits.margins[0], 0);
        assert_eq!(d.undo(), None);
        assert_eq!(d.redo(), Some(None));
        assert_eq!(d.edits.margins[0], 10);
        // Nowa zmiana kasuje „Ponów”.
        d.edits.margins[1] = 5;
        assert!(d.commit());
        assert!(!d.h.can_redo());
    }

    #[test]
    fn maska_jako_lata() {
        let mut d = Doc::new();
        d.h.begin_mask_change(&d.mask);
        d.mask[2 * 8 + 3] = 0; // (3,2)
        d.mask[4 * 8 + 5] = 0; // (5,4)
        d.touched = true;
        assert!(d.commit());
        let patch = d.h.undo.last().unwrap().mask.as_ref().unwrap();
        assert_eq!(patch.rect, PxRect { x0: 3, y0: 2, x1: 6, y1: 5 });
        assert_eq!(patch.pixels.len(), 9, "tylko zmieniony prostokąt");

        let dirty = d.undo().unwrap().unwrap();
        assert_eq!(dirty, PxRect { x0: 3, y0: 2, x1: 6, y1: 5 });
        assert!(d.mask.iter().all(|&v| v == 255));
        assert!(!d.touched);

        d.redo().unwrap();
        assert_eq!(d.mask[2 * 8 + 3], 0);
        assert_eq!(d.mask[4 * 8 + 5], 0);
        assert_eq!(d.mask.iter().filter(|&&v| v == 0).count(), 2);
        assert!(d.touched);
    }

    #[test]
    fn begin_bez_zmiany_nie_tworzy_kroku() {
        let mut d = Doc::new();
        d.h.begin_mask_change(&d.mask);
        assert!(!d.commit());
        assert!(!d.h.can_undo());
    }

    #[test]
    fn limit_krokow() {
        let mut d = Doc::new();
        for i in 0..(MAX_STEPS as u32 + 20) {
            d.edits.margins[0] = i + 1;
            d.commit();
        }
        assert_eq!(d.h.undo.len(), MAX_STEPS);
    }
}
