use std::{thread, time::Duration};

use msu2_flasher_lib::{
    assets::{embedded_assets, FlashImage},
    device::{handshake, open_serial_port, scan_candidates},
    flasher::{flash_images_with_screen_status, preview_pages},
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

    for percent in [1, 2, 25, 50, 75, 100] {
        println!("screen progress {percent}%");
        status.update(&mut port, percent);
        thread::sleep(Duration::from_millis(800));
    }

    status.finish(&mut port);
    thread::sleep(Duration::from_millis(800));
}

#[test]
#[ignore = "rewrites the acquiring image on a connected MSU2 MINI"]
fn hardware_one_image_flash_with_screen_status_smoke() {
    let candidates = scan_candidates().expect("scan serial ports");
    let device = candidates
        .first()
        .expect("connected MSU2 MINI candidate")
        .clone();

    println!("using {}", device.port_name);
    let mut port = open_serial_port(&device.port_name).expect("open serial port");
    handshake(&mut port).expect("handshake");

    let assets = embedded_assets();
    let plan = [FlashImage {
        label: "acquiring",
        start_page: 3826,
        bytes: assets.acquiring,
    }];
    let mut seen = Vec::new();

    flash_images_with_screen_status(&mut port, &plan, |progress| {
        println!(
            "flash progress page {}/{} = {}%",
            progress.current_page, progress.total_pages, progress.percent
        );
        seen.push(progress.percent);
    })
    .expect("flash one image with screen status");

    assert!(seen.contains(&1), "expected 1% progress during flash");
    assert!(seen.contains(&2), "expected 2% progress during flash");
    assert_eq!(seen.last(), Some(&100));

    preview_pages(&mut port).expect("preview pages after smoke flash");
}
