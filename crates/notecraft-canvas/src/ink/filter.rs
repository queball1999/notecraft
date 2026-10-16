//! The one-euro filter (Casiez, Roussel, and Vogel, 2012): a low-pass filter whose cutoff
//! rises with speed. Slow, shaky movement is smoothed hard; fast strokes follow the pen
//! closely, so smoothing adds little lag where lag is noticeable.

use std::f32::consts::TAU;

/// Filter tuning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OneEuroParams {
    /// Cutoff in hertz when the signal is still. Lower smooths jitter more.
    pub min_cutoff: f32,
    /// How fast the cutoff rises with speed, in hertz per unit per second. Higher cuts lag.
    pub beta: f32,
    /// Cutoff in hertz for the speed estimate itself.
    pub d_cutoff: f32,
}

impl OneEuroParams {
    /// Tuned for pen position in page units (96 per inch): jitter of about half a unit at
    /// rest is smoothed, and a stroke at writing speed (100 to 1,500 units per second) trails
    /// the pen by under two units. See the tests in this file.
    pub const POSITION: Self = Self {
        min_cutoff: 2.5,
        beta: 0.1,
        d_cutoff: 1.0,
    };

    /// For pressure, which is 0 to 1 and noisy: fixed light smoothing.
    pub const PRESSURE: Self = Self {
        min_cutoff: 6.0,
        beta: 0.0,
        d_cutoff: 1.0,
    };
}

impl Default for OneEuroParams {
    fn default() -> Self {
        Self::POSITION
    }
}

/// The smoothing factor of a first-order low-pass filter with `cutoff` hertz at a step of `dt`
/// seconds.
fn alpha(cutoff: f32, dt: f32) -> f32 {
    let tau = 1.0 / (TAU * cutoff.max(1.0e-3));
    1.0 / (1.0 + tau / dt)
}

/// A one-euro filter over `N` values that share one cutoff (so a 2D position is smoothed
/// along its direction of travel, not axis by axis).
#[derive(Clone, Debug)]
pub struct OneEuro<const N: usize> {
    params: OneEuroParams,
    /// The last filtered value and the last filtered derivative.
    state: Option<([f32; N], [f32; N])>,
}

impl<const N: usize> OneEuro<N> {
    pub fn new(params: OneEuroParams) -> Self {
        Self {
            params,
            state: None,
        }
    }

    /// Filter `value`, which arrived `dt` seconds after the previous one. The first value
    /// passes through. A `dt` that is not positive and finite counts as 1/120 s. A value that
    /// is not finite is ignored and the last output is returned.
    pub fn filter(&mut self, value: [f32; N], dt: f32) -> [f32; N] {
        if value.iter().any(|v| !v.is_finite()) {
            return self.state.map_or([0.0; N], |(x, _)| x);
        }
        let dt = if dt.is_finite() && dt > 0.0 {
            dt.min(0.25)
        } else {
            1.0 / 120.0
        };
        let Some((prev_x, prev_d)) = self.state else {
            self.state = Some((value, [0.0; N]));
            return value;
        };
        let a_d = alpha(self.params.d_cutoff, dt);
        let mut d_hat = [0.0; N];
        let mut speed_sq = 0.0;
        for i in 0..N {
            let raw = (get(&value, i) - get(&prev_x, i)) / dt;
            let smoothed = get(&prev_d, i) + a_d * (raw - get(&prev_d, i));
            set(&mut d_hat, i, smoothed);
            speed_sq += smoothed * smoothed;
        }
        let cutoff = self.params.min_cutoff + self.params.beta * speed_sq.sqrt();
        let a = alpha(cutoff, dt);
        let mut x_hat = [0.0; N];
        for i in 0..N {
            set(
                &mut x_hat,
                i,
                get(&prev_x, i) + a * (get(&value, i) - get(&prev_x, i)),
            );
        }
        self.state = Some((x_hat, d_hat));
        x_hat
    }
}

fn get<const N: usize>(values: &[f32; N], i: usize) -> f32 {
    values.get(i).copied().unwrap_or_default()
}

fn set<const N: usize>(values: &mut [f32; N], i: usize, v: f32) {
    if let Some(slot) = values.get_mut(i) {
        *slot = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 120.0;

    /// A deterministic pseudo-random value in -1 to 1 (an LCG, so tests need no crate).
    fn noise(seed: &mut u32) -> f32 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    #[test]
    fn the_first_value_passes_through() {
        let mut f = OneEuro::<2>::new(OneEuroParams::POSITION);
        assert_eq!(f.filter([3.0, 4.0], DT), [3.0, 4.0]);
    }

    #[test]
    fn a_still_signal_stays_put() {
        let mut f = OneEuro::<2>::new(OneEuroParams::POSITION);
        let mut out = [0.0; 2];
        for _ in 0..200 {
            out = f.filter([10.0, -5.0], DT);
        }
        assert!((out[0] - 10.0).abs() < 1.0e-4 && (out[1] + 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn jitter_at_rest_is_cut_by_more_than_half() {
        let mut f = OneEuro::<2>::new(OneEuroParams::POSITION);
        let mut seed = 7;
        let (mut raw_sq, mut out_sq, mut n) = (0.0, 0.0, 0.0);
        for i in 0..600 {
            let jitter = [0.5 * noise(&mut seed), 0.5 * noise(&mut seed)];
            let out = f.filter([100.0 + jitter[0], 100.0 + jitter[1]], DT);
            if i >= 60 {
                raw_sq += jitter[0] * jitter[0] + jitter[1] * jitter[1];
                out_sq += (out[0] - 100.0).powi(2) + (out[1] - 100.0).powi(2);
                n += 1.0;
            }
        }
        let (raw_rms, out_rms) = ((raw_sq / n).sqrt(), (out_sq / n).sqrt());
        assert!(out_rms < raw_rms * 0.5, "raw {raw_rms}, filtered {out_rms}");
    }

    #[test]
    fn a_fast_line_is_followed_closely() {
        for speed in [100.0_f32, 400.0, 1500.0] {
            let mut f = OneEuro::<2>::new(OneEuroParams::POSITION);
            let mut lag = 0.0;
            for i in 0..240 {
                let x = speed * DT * i as f32;
                let out = f.filter([x, 0.5 * x], DT);
                lag = ((x - out[0]).powi(2) + (0.5 * x - out[1]).powi(2)).sqrt();
            }
            // At steady speed the filter trails by speed * tau; the speed-adaptive cutoff
            // keeps that under two page units across writing speeds.
            assert!(lag < 2.0, "lag {lag} at {speed} units per second");
        }
    }

    #[test]
    fn bad_input_never_propagates() {
        let mut f = OneEuro::<2>::new(OneEuroParams::POSITION);
        f.filter([1.0, 1.0], DT);
        let out = f.filter([f32::NAN, 5.0], DT);
        assert_eq!(out, [1.0, 1.0]);
        // Zero, negative, NaN, and huge steps are all tolerated.
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY, 1.0e9] {
            let out = f.filter([2.0, 2.0], dt);
            assert!(out.iter().all(|v| v.is_finite()));
        }
        let mut fresh = OneEuro::<1>::new(OneEuroParams::PRESSURE);
        assert_eq!(fresh.filter([f32::INFINITY], DT), [0.0]);
    }
}
