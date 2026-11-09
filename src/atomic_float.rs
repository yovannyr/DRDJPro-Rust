//! Lock-free floating-point observations backed by the platform's 32-bit atomics.
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Default)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub const fn new(value: f32) -> Self {
        Self(AtomicU32::new(value.to_bits()))
    }

    pub fn load(&self, ordering: Ordering) -> f32 {
        f32::from_bits(self.0.load(ordering))
    }

    pub fn store(&self, value: f32, ordering: Ordering) {
        self.0.store(value.to_bits(), ordering);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_float_bits() {
        let atomic = AtomicF32::new(-0.0);
        assert_eq!(
            atomic.load(Ordering::Relaxed).to_bits(),
            (-0.0f32).to_bits()
        );
        atomic.store(0.375, Ordering::Release);
        assert_eq!(atomic.load(Ordering::Acquire), 0.375);
    }
}
