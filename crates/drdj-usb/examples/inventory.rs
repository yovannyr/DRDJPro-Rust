//! Automated read-only native discovery probe: no handles, claims or transfers.
use drdj_usb::{DeviceRegistry, DiscoveryWorker};
use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let worker = DiscoveryWorker::native(DeviceRegistry::default())?;
    while start.elapsed() < Duration::from_secs(3) {
        if let Some(snapshot) = worker
            .try_snapshot()
            .filter(|snapshot| snapshot.revision > 0)
        {
            if let Some(error) = &snapshot.error {
                return Err(error.clone().into());
            }
            println!(
                "USB devices: {}; interfaces: {}; first snapshot: {:.2} ms",
                snapshot.devices.len(),
                snapshot
                    .devices
                    .iter()
                    .map(|d| d.interfaces.len())
                    .sum::<usize>(),
                start.elapsed().as_secs_f64() * 1000.0
            );
            println!("Discovery only: device strings, claims and transfers were not requested.");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err("USB discovery exceeded the probe's three-second deadline".into())
}
