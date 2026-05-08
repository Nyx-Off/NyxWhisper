//! Génère programmatiquement le logo NyxWhisper :
//! un "N" stylisé blanc cassé, légèrement érodé (effet grunge), sur fond noir.
//!
//! Utilisé pour :
//! - L'icône system tray
//! - L'icône de la fenêtre principale (eframe ViewportBuilder::with_icon)
//! - L'icône du .exe Windows (via build.rs + winres + assets/icon.ico)

/// PRNG simple et déterministe (xorshift32) pour avoir un grunge reproductible
/// sans dépendance externe.
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed)
    }
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn next_byte(&mut self) -> u8 {
        (self.next_u32() & 0xFF) as u8
    }
    fn chance(&mut self, percent: u8) -> bool {
        self.next_byte() < (percent as f32 * 2.55) as u8
    }
}

/// Couleur d'arrière-plan (noir profond, légèrement bleuté).
const BG: [u8; 4] = [10, 10, 14, 255];
/// Couleur principale du N (blanc cassé un peu chaud).
const FG: [u8; 4] = [240, 232, 220, 255];

/// Génère une icône RGBA carrée de `size` pixels.
pub fn n_grunge_rgba(size: u32) -> Vec<u8> {
    let w = size as i32;
    let h = size as i32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // Fond
    for px in rgba.chunks_exact_mut(4) {
        px.copy_from_slice(&BG);
    }

    // Géométrie
    let margin = ((w as f32) * 0.16) as i32;
    let stroke = ((w as f32) * 0.18).max(2.0) as i32;
    let left = margin;
    let right = w - margin;
    let top = margin;
    let bottom = h - margin;

    let mut rng = Rng::new(0xC0FFEE ^ size);

    let paint = |buf: &mut [u8], x: i32, y: i32, intensity: f32, rng: &mut Rng| {
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        if rng.chance(8) {
            return;
        } // érosion ~8%
        let i = ((y * w + x) as usize) * 4;
        let a = (intensity.clamp(0.0, 1.0) * 255.0) as u32;
        let blend = |bg: u8, fg: u8| -> u8 {
            ((bg as u32) * (255 - a) / 255 + (fg as u32) * a / 255) as u8
        };
        buf[i] = blend(buf[i], FG[0]);
        buf[i + 1] = blend(buf[i + 1], FG[1]);
        buf[i + 2] = blend(buf[i + 2], FG[2]);
        buf[i + 3] = 255;
    };

    // Barres verticales
    for y in top..=bottom {
        for dx in 0..stroke {
            paint(&mut rgba, left + dx, y, 1.0, &mut rng);
            paint(&mut rgba, right - 1 - dx, y, 1.0, &mut rng);
        }
    }

    // Diagonale haut-gauche -> bas-droite
    let dx_total = (right - 1) - (left + stroke);
    let dy_total = bottom - top;
    let steps = dx_total.max(dy_total).max(1);
    for s in 0..=steps {
        let t = s as f32 / steps as f32;
        let cx = (left + stroke / 2) as f32 + t * (dx_total - stroke / 2) as f32;
        let cy = top as f32 + t * dy_total as f32;
        for off in -(stroke / 2)..=(stroke / 2) {
            paint(&mut rgba, cx as i32 + off, cy as i32, 1.0, &mut rng);
            paint(&mut rgba, cx as i32 + off, cy as i32 + 1, 0.8, &mut rng);
        }
    }

    // Bruit de fond léger pour texture
    let mut nrng = Rng::new(0xFEEDBEEF ^ size);
    for px in rgba.chunks_exact_mut(4) {
        if nrng.chance(4) {
            let delta = (nrng.next_byte() as i16) - 128;
            let adj = (delta / 8).clamp(-12, 12);
            for c in 0..3 {
                px[c] = (px[c] as i16 + adj).clamp(0, 255) as u8;
            }
        }
    }

    rgba
}

pub const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
