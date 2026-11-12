//! Read-only diagnostic: send a *query* frame (FA/FB only) to the vendor interface and dump replies.
//! Usage: rawquery fa 19 00 01
fn main() {
    let bytes: Vec<u8> = std::env::args().skip(1).map(|a| u8::from_str_radix(&a, 16).expect("hex byte")).collect();
    assert!(matches!(bytes.first(), Some(0xFA) | Some(0xFB)), "only FA/FB query frames are allowed");
    let api = hidapi::HidApi::new().unwrap();
    let info = api.device_list().find(|d| d.vendor_id() == 0x514c && d.usage_page() == 0xff00).expect("vendor interface");
    let dev = info.open_device(&api).unwrap();
    let mut buf = [0u8; 64];
    while dev.read_timeout(&mut buf, 50).unwrap_or(0) > 0 {}
    let mut f = [0u8; 65];
    f[0] = 3;
    f[1..1 + bytes.len()].copy_from_slice(&bytes);
    dev.write(&f).unwrap();
    for i in 0.. {
        match dev.read_timeout(&mut buf, 1000) {
            Ok(n) if n > 0 => {
                let end = buf.iter().rposition(|&b| b != 0).map_or(1, |p| p + 1);
                println!("{i:2}: {}", buf[..end].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
            }
            _ => { println!("({i} replies)"); break; }
        }
    }
}
