//! Deterministic randomness and the price path.
//!
//! Every value is a pure function of (seed, time): there is no state and no
//! history to replay, so any range can be computed directly, every visitor
//! sees the same market, and bars of different intervals are cut from the
//! same path.

/// SplitMix64 finaliser.
pub fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Hash of a seed and two coordinates.
pub fn hash(seed: u64, a: u64, b: u64) -> u64 {
    mix(seed ^ mix(a ^ mix(b)))
}

/// Uniform in (0, 1).
pub fn unit(h: u64) -> f64 {
    ((h >> 11) as f64 + 0.5) / (1u64 << 53) as f64
}

/// Standard normal (Box–Muller).
pub fn gauss(h: u64) -> f64 {
    let u1 = unit(h);
    let u2 = unit(mix(h));
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

/// Approximately standard normal, cheap: the sum of four 16-bit uniforms
/// (Irwin–Hall), scaled to unit variance. For the path's lattice values,
/// where it is evaluated millions of times per request.
pub fn quick_gauss(h: u64) -> f64 {
    let s = (h & 0xFFFF) + ((h >> 16) & 0xFFFF) + ((h >> 32) & 0xFFFF) + (h >> 48);
    (s as f64 / 65_535.0 - 2.0) * 3f64.sqrt()
}

/// Seed of a name (FNV-1a).
pub fn seed_of(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    mix(h)
}

/// Octaves 0..OCTAVES have wavelengths 1 s, 2 s, ... 2^(OCTAVES-1) s (~2 years).
pub const OCTAVES: u32 = 26;
/// Octaves below this are scaled by the volatility regime (up to ~4.5 h).
const FAST: u32 = 14;

/// Linear value noise at octave `k` (wavelength 2^k s), unit variance at
/// lattice points.
fn octave(seed: u64, k: u32, t_sec: f64) -> f64 {
    let x = t_sec / (1u64 << k) as f64;
    let i = x.floor();
    let f = x - i;
    let i = i as i64 as u64;
    let a = quick_gauss(hash(seed, u64::from(k), i));
    let b = quick_gauss(hash(seed, u64::from(k), i.wrapping_add(1)));
    a + (b - a) * f
}

/// Octave `k` with smoothstep interpolation: no corners, for slow
/// "fair value" curves.
fn octave_smooth(seed: u64, k: u32, t_sec: f64) -> f64 {
    let x = t_sec / (1u64 << k) as f64;
    let i = x.floor();
    let f = x - i;
    let f = f * f * (3.0 - 2.0 * f);
    let i = i as i64 as u64;
    let a = quick_gauss(hash(seed, u64::from(k), i));
    let b = quick_gauss(hash(seed, u64::from(k), i.wrapping_add(1)));
    a + (b - a) * f
}

/// Smooth noise over octaves `lo..hi`, normalised to about unit variance.
pub fn smooth(seed: u64, t_ms: i64, lo: u32, hi: u32) -> f64 {
    let t = t_ms as f64 / 1000.0;
    let mut sum = 0.0;
    for k in lo..hi {
        sum += octave(seed, k, t);
    }
    sum / f64::from((hi - lo).max(1)).sqrt() / 0.8
}

/// A log-price path: a sum of octaves with amplitudes ∝ √wavelength, which
/// behaves like Brownian motion from seconds to months and stays bounded
/// over years. Fast octaves are scaled by a slowly moving volatility regime.
#[derive(Clone, Debug)]
pub struct Path {
    seed: u64,
    amp: [f64; OCTAVES as usize],
    /// Correlated with another path: `w·parent + √(1−w²)·own`.
    link: Option<(Box<Path>, f64)>,
}

impl Path {
    /// `daily_vol`: standard deviation of one-day log returns.
    pub fn new(seed: u64, daily_vol: f64) -> Self {
        // Calibrated so one-day returns have about `daily_vol` std (see the
        // test below); increments over Δ get ~σ²Δ from the octaves near Δ.
        let per_sec = daily_vol / 86_400f64.sqrt();
        let c = per_sec * 0.6;
        let mut amp = [0.0; OCTAVES as usize];
        for (k, a) in amp.iter_mut().enumerate() {
            *a = c * ((1u64 << k) as f64).sqrt();
        }
        Self {
            seed,
            amp,
            link: None,
        }
    }

    /// This path correlated with `parent` (weight `w` in 0..1 of the
    /// parent's moves; the variance stays the same for equal volatilities).
    pub fn linked(mut self, parent: Path, w: f64) -> Self {
        self.link = Some((Box::new(parent), w.clamp(0.0, 1.0)));
        self
    }

    /// Volatility multiplier (~0.5–2), changing over days.
    pub fn regime(&self, t_ms: i64) -> f64 {
        (0.45 * smooth(self.seed ^ 0xA5A5, t_ms, 15, 21)).exp()
    }

    /// Log price relative to the base at `t_ms`. Octaves below `min_k` are
    /// left out (finer than a caller's sampling step).
    pub fn log_at(&self, t_ms: i64, min_k: u32) -> f64 {
        let t = t_ms as f64 / 1000.0;
        let mut fast = 0.0;
        for k in min_k.min(FAST)..FAST {
            fast += self.amp[k as usize] * octave(self.seed, k, t);
        }
        let mut slow = 0.0;
        for k in FAST..OCTAVES {
            slow += self.amp[k as usize] * octave(self.seed, k, t);
        }
        let own = slow + self.regime(t_ms) * fast;
        match &self.link {
            Some((p, w)) => w * p.log_at(t_ms, min_k) + (1.0 - w * w).sqrt() * own,
            None => own,
        }
    }

    /// The slow part only (octaves `from_k..`, smoothly interpolated): a
    /// "fair value" the price oscillates around.
    pub fn trend(&self, t_ms: i64, from_k: u32) -> f64 {
        let t = t_ms as f64 / 1000.0;
        let own: f64 = (from_k.min(OCTAVES)..OCTAVES)
            .map(|k| self.amp[k as usize] * octave_smooth(self.seed, k, t))
            .sum();
        match &self.link {
            Some((p, w)) => w * p.trend(t_ms, from_k) + (1.0 - w * w).sqrt() * own,
            None => own,
        }
    }
}

/// Lowest octave worth evaluating for samples `step_ms` apart.
pub fn min_octave(step_ms: i64) -> u32 {
    let s = (step_ms as f64 / 2000.0).max(1.0);
    (s.log2().floor() as u32).min(OCTAVES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_bounded_uniform() {
        assert_eq!(hash(1, 2, 3), hash(1, 2, 3));
        assert_ne!(hash(1, 2, 3), hash(1, 3, 2));
        for i in 0..10_000 {
            let u = unit(mix(i));
            assert!(u > 0.0 && u < 1.0);
        }
    }

    #[test]
    fn daily_returns_have_the_requested_volatility() {
        let p = Path::new(seed_of("TEST"), 0.03);
        let day = 86_400_000;
        let t0 = 1_700_000_000_000;
        let n = 2000;
        let rets: Vec<f64> = (0..n)
            .map(|i| p.log_at(t0 + (i + 1) * day, 0) - p.log_at(t0 + i * day, 0))
            .collect();
        let mean = rets.iter().sum::<f64>() / n as f64;
        let sd = (rets.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / n as f64).sqrt();
        assert!((0.018..0.045).contains(&sd), "daily sd {sd}");
    }
}
