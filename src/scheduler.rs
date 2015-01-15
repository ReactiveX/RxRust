

use std::time::duration::Duration;
use std::io::timer::Timer;
use time::precise_time_ns;
use reactive::Publisher;
use reactor::StreamBuf; 

/// This spins in a loop at a specific frame rate
/// it sleeps between tries
/// rate is the max number of loops per second that
/// it makes There is no minimum number of loops
pub fn fixed_loop<F>(rate: u64, mut f: F) where F : FnMut() -> bool {
    debug!("Starting loop");
    let mut timer = Timer::new().unwrap();
    let loop_time_ns = Duration::nanoseconds(1_000_000_000i64 / rate as i64);
    let mut run = true;

    loop {
        let start = precise_time_ns() as i64;
        if !f() {
            break;
        }
        let span = Duration::nanoseconds(precise_time_ns() as i64 - start);
        if let Some(diff) = loop_time_ns.checked_sub(&span) {
            timer.sleep(diff);
        }
    }

    debug!("Done with loop");
}

