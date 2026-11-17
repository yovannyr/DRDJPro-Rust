use std::sync::atomic::{AtomicU64, Ordering};

/// Process-local identity shared by USB and HID workers. Never wraps/reuses IDs.
pub(crate) fn next_id() -> Option<u64> {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let mut current = NEXT.load(Ordering::Relaxed);
    loop {
        let next = current.checked_add(1)?;
        match NEXT.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Some(current),
            Err(value) => current = value,
        }
    }
}
