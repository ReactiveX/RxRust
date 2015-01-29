

use std::time::duration::Duration;
use std::old_io::timer::Timer;
use std::num::Int;
use time::precise_time_ns;
use reactive::Publisher;
use reactor::StreamBuf;
use libc::{timespec, nanosleep};

/// This spins in a loop at a specific frame rate
/// it sleeps between tries
/// rate is the max number of loops per second that
/// it makes There is no minimum number of loops
pub fn fixed_loop<F>(rate: u64, mut f: F) where F : FnMut() -> bool {
    debug!("Starting loop");
    let mut timer = Timer::new().unwrap();
    let loop_time_ns = 1_000_000_000i64 / rate as i64;
    let mut run = true;
    let mut nein = timespec { tv_sec: 0, tv_nsec: 0 };

    loop {
        let start = precise_time_ns() as i64;
        if !f() {
            break;
        }
        let span = precise_time_ns() as i64 - start;
        if let Some(diff) = loop_time_ns.checked_sub(span) {
            let ts = timespec { tv_sec: 0, tv_nsec: diff };
            unsafe { nanosleep(&ts as *const timespec, &mut nein as *mut timespec) };
        }
    }

    debug!("Done with loop");
}

