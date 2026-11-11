//! Read-only HID metadata probe: never opens a target or captures reports.
use drdj_usb::{HidCommand, HidReply, HidWorker};
use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let worker = HidWorker::native()?;
    let mut ticket = worker.submit(HidCommand::Enumerate)?;
    while start.elapsed() < Duration::from_secs(3) {
        if let Some(reply) = ticket.poll() {
            let HidReply::Devices(devices) = reply? else {
                return Err("unexpected HID inventory completion".into());
            };
            println!(
                "HID collections: {}; metadata: {:.2} ms",
                devices.len(),
                start.elapsed().as_secs_f64() * 1000.0
            );
            println!("Metadata only: no explicit device open, input capture or output request.");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("HID metadata probe exceeded three-second deadline".into())
}
