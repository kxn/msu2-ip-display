use std::{thread, time::Duration};

use msu2_flasher_lib::{
    device::{handshake, open_serial_port, scan_candidates},
    screen_status::ScreenStatus,
};

#[test]
#[ignore = "requires a connected MSU2 MINI on the verified serial path"]
fn hardware_screen_status_smoke() {
    let candidates = scan_candidates().expect("scan serial ports");
    let device = candidates
        .first()
        .expect("connected MSU2 MINI candidate")
        .clone();

    println!("using {}", device.port_name);
    let mut port = open_serial_port(&device.port_name).expect("open serial port");
    handshake(&mut port).expect("handshake");

    let mut status = ScreenStatus::probe(&mut port);
    assert!(status.is_enabled(), "direct LCD probe should be enabled");

    status.start(&mut port);
    thread::sleep(Duration::from_millis(800));

    for percent in [25, 50, 75, 100] {
        println!("screen progress {percent}%");
        status.update(&mut port, percent);
        thread::sleep(Duration::from_millis(800));
    }

    status.finish(&mut port);
    thread::sleep(Duration::from_millis(800));
}
