pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn decode_utf8_lenient(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    let continuation = |index: usize| bytes.get(index).is_some_and(|byte| byte & 0xC0 == 0x80);
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte < 0x80 {
            units.push(u16::from(byte));
            index += 1;
        } else if byte & 0xE0 == 0xC0 && continuation(index + 1) {
            units.push(u16::from(byte & 0x1F) << 6 | u16::from(bytes[index + 1] & 0x3F));
            index += 2;
        } else if byte & 0xF0 == 0xE0 && continuation(index + 1) && continuation(index + 2) {
            let value = u16::from(byte & 0x0F) << 12
                | u16::from(bytes[index + 1] & 0x3F) << 6
                | u16::from(bytes[index + 2] & 0x3F);
            units.push(value);
            index += 3;
        } else if byte & 0xF8 == 0xF0 && continuation(index + 1) && continuation(index + 2) && continuation(index + 3) {
            let code = (u32::from(byte & 0x07) << 18)
                | (u32::from(bytes[index + 1] & 0x3F) << 12)
                | (u32::from(bytes[index + 2] & 0x3F) << 6)
                | u32::from(bytes[index + 3] & 0x3F);
            let mut buffer = [0u16; 2];
            units.extend_from_slice(char::from_u32(code).unwrap_or('\u{FFFD}').encode_utf16(&mut buffer));
            index += 4;
        } else {
            units.push(0xFFFD);
            index += 1;
        }
    }
    String::from_utf16_lossy(&units)
}
