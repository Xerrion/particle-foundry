//! Explicit native adapter test. Failure is not converted to a skipped success.

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

struct Notify(std::thread::Thread);
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn main() {
    let waker: Waker = Arc::new(Notify(std::thread::current())).into();
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(particle_sim_gpu::GpuContext::new());
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(Ok(device)) => {
                println!("Native device initialized: {}", device.backend());
                device.dispose();
                return;
            }
            Poll::Ready(Err(error)) => panic!("Native GPU initialization failed: {error}"),
            Poll::Pending => {
                assert!(
                    Instant::now() < deadline,
                    "Native adapter initialization timed out"
                );
                std::thread::park_timeout(Duration::from_millis(100));
            }
        }
    }
}
