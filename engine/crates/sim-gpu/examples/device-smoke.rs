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
    let mut future = Box::pin(async {
        let device = particle_sim_gpu::GpuContext::new().await?;
        let grid = particle_sim::Grid::new(480.0, 270.0).map_err(str::to_string)?;
        let plan = device.preflight_core_grid(grid, 2)?;
        let report = device.validate_abi().await?;
        Ok::<_, String>((device, plan, report))
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(Ok((device, plan, report))) => {
                println!("Native ABI sentinel passed: {report:?}");
                println!("480x270 two-component core field preflight: {plan:?}");
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
