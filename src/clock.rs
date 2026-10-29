//! Monotonic timing that also works in the browser.
//!
//! `std::time::Instant::now()` panics on `wasm32-unknown-unknown`, which has no clock.
//! Native builds re-export the std type; WebAssembly builds read the page's
//! `performance.now()` through one imported function supplied by `web/engine-worker.js`.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

#[cfg(target_arch = "wasm32")]
pub use wasm::Instant;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::time::Duration;

    #[link(wasm_import_module = "env")]
    extern "C" {
        /// Milliseconds from `performance.now()`, provided by the JS host.
        fn rsashield_now_ms() -> f64;
    }

    fn now_ms() -> f64 {
        // SAFETY: a pure function import with no arguments or memory access.
        unsafe { rsashield_now_ms() }
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Instant(f64);

    impl Instant {
        pub fn now() -> Self {
            Instant(now_ms())
        }

        pub fn elapsed(&self) -> Duration {
            Duration::from_secs_f64((now_ms() - self.0).max(0.0) / 1000.0)
        }
    }
}
