use std::sync::atomic::AtomicU32;

/// A simple counter that can yields unique values
#[derive(Default)]
pub(crate) struct Counter {
    value: AtomicU32,
}

impl Counter {
    pub(crate) fn new(start: u32) -> Self {
        Counter {
            value: AtomicU32::new(start),
        }
    }

    pub(crate) fn next(&self) -> u32 {
        self.value.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
