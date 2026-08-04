use std::collections::VecDeque;

// https://jsgroth.dev/blog/posts/gb-rewrite-apu/
// https://github.com/jsgroth/jgb/blob/main/jgb-core/src/apu/filter.rs
/*
In Octave: https://octave.org/

>>> pkg load signal
>>> printf("%.16e,\n", fir1(45, 24000 / (4194304 / 2), 'low'))

Low-pass filtering solution from JS Groth by convolving the samples over
a FIR kernel to remove high frequencies that cause ringing. Changed
parameter to the GB clock speed
*/
#[allow(clippy::excessive_precision)]
const FIR_KERNEL: [f64; 46] = [
    3.0340257031444750e-03,
    3.2303458884755001e-03,
    3.7700476138008885e-03,
    4.6505234761282221e-03,
    5.8616229429089371e-03,
    7.3857798364708703e-03,
    9.1983110951861183e-03,
    1.1267881527559056e-02,
    1.3557125257348417e-02,
    1.6023410735889698e-02,
    1.8619732667827644e-02,
    2.1295711047587422e-02,
    2.3998674816114087e-02,
    2.6674805489548363e-02,
    2.9270314539496509e-02,
    3.1732627359889992e-02,
    3.4011546364101641e-02,
    3.6060366127739475e-02,
    3.7836914520607688e-02,
    3.9304495432530562e-02,
    4.0432710953050822e-02,
    4.1198143660500626e-02,
    4.1584882944093002e-02,
    4.1584882944093009e-02,
    4.1198143660500626e-02,
    4.0432710953050816e-02,
    3.9304495432530569e-02,
    3.7836914520607688e-02,
    3.6060366127739475e-02,
    3.4011546364101634e-02,
    3.1732627359890006e-02,
    2.9270314539496509e-02,
    2.6674805489548373e-02,
    2.3998674816114077e-02,
    2.1295711047587422e-02,
    1.8619732667827651e-02,
    1.6023410735889702e-02,
    1.3557125257348411e-02,
    1.1267881527559063e-02,
    9.1983110951861270e-03,
    7.3857798364708772e-03,
    5.8616229429089440e-03,
    4.6505234761282195e-03,
    3.7700476138008920e-03,
    3.2303458884755006e-03,
    3.0340257031444750e-03,
];

pub struct LowPassFilter {
    samples: VecDeque<f64>,
}

impl LowPassFilter {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(FIR_KERNEL.len()),
        }
    }

    pub fn collect_sample(&mut self, sample: f64) {
        self.samples.push_back(sample);
        if self.samples.len() > FIR_KERNEL.len() {
            self.samples.pop_front();
        }
    }

    pub fn convolve(&self) -> f64 {
        self.samples
            .iter()
            .copied()
            .zip(FIR_KERNEL.iter().copied())
            .map(|(a, b)| a * b)
            .sum()
    }
}
