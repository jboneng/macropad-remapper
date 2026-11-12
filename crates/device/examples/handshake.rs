//! Read-only diagnostic: send only the handshake (03 FB FB FB) to the vendor interface and dump replies.
fn main() {
    let api = hidapi::HidApi::new().unwrap();
    let info = api.device_list().find(|d| d.vendor_id() == 0x514c && d.usage_page() == 0xff00).expect("vendor interface");
    let dev = info.open_device(&api).unwrap();
    let f = macropad_remapper_proto::frame::handshake();
    println!("write -> {:?}", dev.write(&f));
    for i in 0..5 {
        let mut buf = [0u8; 64];
        match dev.read_timeout(&mut buf, 1000) {
            Ok(0) => println!("read {i}: timeout"),
            Ok(n) => println!("read {i} ({n} B): {}", buf[..n].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")),
            Err(e) => println!("read {i}: error {e}"),
        }
    }
}
