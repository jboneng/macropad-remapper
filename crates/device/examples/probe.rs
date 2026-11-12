//! Read-only diagnostic: print every interface's HID report descriptor.
fn main() {
    let api = hidapi::HidApi::new().unwrap();
    for d in api.device_list() {
        if !macropad_remapper_proto::VENDOR_IDS.contains(&d.vendor_id()) || !macropad_remapper_proto::PRODUCT_IDS.contains(&d.product_id()) {
            continue;
        }
        println!("== {:04x}:{:04x} if={} usage={:04x}:{:04x} path={:?}", d.vendor_id(), d.product_id(), d.interface_number(), d.usage_page(), d.usage(), d.path());
        match d.open_device(&api) {
            Ok(dev) => {
                let mut buf = [0u8; 4096];
                match dev.get_report_descriptor(&mut buf) {
                    Ok(n) => println!("   descriptor ({n} B): {}", buf[..n].iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")),
                    Err(e) => println!("   descriptor error: {e}"),
                }
            }
            Err(e) => println!("   open error: {e}"),
        }
    }
}
