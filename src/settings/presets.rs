const BUCKETS: [(u32, &str); 6] = [
    (120, "ldpi"),
    (160, "mdpi"),
    (240, "hdpi"),
    (320, "xhdpi"),
    (480, "xxhdpi"),
    (640, "xxxhdpi"),
];

pub(crate) struct Device {
    pub(crate) brand: &'static str,
    pub(crate) model: &'static str,
    pub(crate) abi: &'static str,
    pub(crate) dpi: u32,
}

impl Device {
    pub(crate) fn density(&self) -> &'static str {
        BUCKETS
            .iter()
            .min_by_key(|(dpi, _)| self.dpi.abs_diff(*dpi))
            .map(|(_, name)| *name)
            .unwrap_or("xxhdpi")
    }
}

pub(crate) const DEVICES: &[Device] = &[
    Device { brand: "Google Pixel", model: "6", abi: "arm64_v8a", dpi: 411 },
    Device { brand: "Google Pixel", model: "6a", abi: "arm64_v8a", dpi: 431 },
    Device { brand: "Google Pixel", model: "7", abi: "arm64_v8a", dpi: 418 },
    Device { brand: "Google Pixel", model: "7a", abi: "arm64_v8a", dpi: 431 },
    Device { brand: "Google Pixel", model: "8", abi: "arm64_v8a", dpi: 424 },
    Device { brand: "Google Pixel", model: "8 Pro", abi: "arm64_v8a", dpi: 490 },
    Device { brand: "Google Pixel", model: "9", abi: "arm64_v8a", dpi: 421 },
    Device { brand: "Google Pixel", model: "9a", abi: "arm64_v8a", dpi: 421 },
    Device { brand: "Google Pixel", model: "9 Pro", abi: "arm64_v8a", dpi: 497 },
    Device { brand: "Google Pixel", model: "11", abi: "arm64_v8a", dpi: 421 },
    Device { brand: "Google Pixel", model: "11 Pro", abi: "arm64_v8a", dpi: 497 },
    Device { brand: "Google Pixel", model: "11 Pro XL", abi: "arm64_v8a", dpi: 486 },
    Device { brand: "Google Pixel", model: "10", abi: "arm64_v8a", dpi: 421 },
    Device { brand: "Google Pixel", model: "10 Pro", abi: "arm64_v8a", dpi: 497 },
    Device { brand: "Google Pixel", model: "10 Pro XL", abi: "arm64_v8a", dpi: 482 },
    Device { brand: "Samsung", model: "Galaxy S22", abi: "arm64_v8a", dpi: 422 },
    Device { brand: "Samsung", model: "Galaxy S23", abi: "arm64_v8a", dpi: 422 },
    Device { brand: "Samsung", model: "Galaxy S23 Ultra", abi: "arm64_v8a", dpi: 501 },
    Device { brand: "Samsung", model: "Galaxy S24", abi: "arm64_v8a", dpi: 416 },
    Device { brand: "Samsung", model: "Galaxy S24+", abi: "arm64_v8a", dpi: 513 },
    Device { brand: "Samsung", model: "Galaxy S24 Ultra", abi: "arm64_v8a", dpi: 505 },
    Device { brand: "Samsung", model: "Galaxy S25", abi: "arm64_v8a", dpi: 416 },
    Device { brand: "Samsung", model: "Galaxy S25 Ultra", abi: "arm64_v8a", dpi: 498 },
    Device { brand: "Samsung", model: "Galaxy A15", abi: "arm64_v8a", dpi: 396 },
    Device { brand: "Samsung", model: "Galaxy A34", abi: "arm64_v8a", dpi: 390 },
    Device { brand: "Samsung", model: "Galaxy A54", abi: "arm64_v8a", dpi: 403 },
    Device { brand: "Samsung", model: "Galaxy A55", abi: "arm64_v8a", dpi: 390 },
    Device { brand: "Samsung", model: "Galaxy S26", abi: "arm64_v8a", dpi: 409 },
    Device { brand: "Samsung", model: "Galaxy S26 Ultra", abi: "arm64_v8a", dpi: 498 },
    Device { brand: "Samsung", model: "Galaxy A07", abi: "arm64_v8a", dpi: 262 },
    Device { brand: "Samsung", model: "Galaxy A17", abi: "arm64_v8a", dpi: 385 },
    Device { brand: "Samsung", model: "Galaxy A26", abi: "arm64_v8a", dpi: 385 },
    Device { brand: "Samsung", model: "Galaxy A36", abi: "arm64_v8a", dpi: 385 },
    Device { brand: "Samsung", model: "Galaxy A56", abi: "arm64_v8a", dpi: 385 },
    Device { brand: "POCO", model: "F5", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "M6 Pro", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "X5 Pro", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "X6", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "X6 Pro", abi: "arm64_v8a", dpi: 446 },
    Device { brand: "POCO", model: "F6", abi: "arm64_v8a", dpi: 446 },
    Device { brand: "POCO", model: "X7 Pro", abi: "arm64_v8a", dpi: 446 },
    Device { brand: "POCO", model: "F3", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "F4", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "F4 GT", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "X3 Pro", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "X3 NFC", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "POCO", model: "M3 Pro", abi: "arm64_v8a", dpi: 405 },
    Device { brand: "Xiaomi", model: "13", abi: "arm64_v8a", dpi: 414 },
    Device { brand: "Xiaomi", model: "13 Ultra", abi: "arm64_v8a", dpi: 521 },
    Device { brand: "Xiaomi", model: "14", abi: "arm64_v8a", dpi: 460 },
    Device { brand: "Xiaomi", model: "14 Ultra", abi: "arm64_v8a", dpi: 521 },
    Device { brand: "Xiaomi", model: "14T Pro", abi: "arm64_v8a", dpi: 446 },
    Device { brand: "Xiaomi", model: "17", abi: "arm64_v8a", dpi: 464 },
    Device { brand: "Redmi", model: "Note 12 Pro", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "Redmi", model: "Note 13", abi: "arm64_v8a", dpi: 395 },
    Device { brand: "Redmi", model: "Note 13 Pro", abi: "arm64_v8a", dpi: 446 },
    Device { brand: "Redmi", model: "Note 14 Pro", abi: "arm64_v8a", dpi: 446 },
    Device { brand: "Redmi", model: "13", abi: "arm64_v8a", dpi: 396 },
    Device { brand: "OnePlus", model: "11", abi: "arm64_v8a", dpi: 526 },
    Device { brand: "OnePlus", model: "12", abi: "arm64_v8a", dpi: 510 },
    Device { brand: "OnePlus", model: "12R", abi: "arm64_v8a", dpi: 450 },
    Device { brand: "OnePlus", model: "13", abi: "arm64_v8a", dpi: 510 },
    Device { brand: "OnePlus", model: "Nord 3", abi: "arm64_v8a", dpi: 394 },
    Device { brand: "OnePlus", model: "Nord 4", abi: "arm64_v8a", dpi: 472 },
    Device { brand: "OnePlus", model: "Nord CE 3 Lite", abi: "arm64_v8a", dpi: 392 },
    Device { brand: "OnePlus", model: "15", abi: "arm64_v8a", dpi: 450 },
    Device { brand: "Motorola", model: "Edge 40", abi: "arm64_v8a", dpi: 402 },
    Device { brand: "Motorola", model: "Edge 50 Pro", abi: "arm64_v8a", dpi: 444 },
    Device { brand: "Motorola", model: "Moto G84", abi: "arm64_v8a", dpi: 405 },
    Device { brand: "Motorola", model: "Moto G Power 5G (2024)", abi: "arm64_v8a", dpi: 393 },
    Device { brand: "Sony", model: "Xperia 1 V", abi: "arm64_v8a", dpi: 643 },
    Device { brand: "Sony", model: "Xperia 5 V", abi: "arm64_v8a", dpi: 449 },
    Device { brand: "Sony", model: "Xperia 10 V", abi: "arm64_v8a", dpi: 449 },
    Device { brand: "Nothing", model: "Phone (2)", abi: "arm64_v8a", dpi: 394 },
    Device { brand: "Nothing", model: "Phone (2a)", abi: "arm64_v8a", dpi: 394 },
    Device { brand: "Fairphone", model: "Fairphone 5", abi: "arm64_v8a", dpi: 466 },
    Device { brand: "Huawei", model: "Mate 60", abi: "arm64_v8a", dpi: 448 },
    Device { brand: "Huawei", model: "P60 Pro", abi: "arm64_v8a", dpi: 453 },
    Device { brand: "Huawei", model: "nova 11 Pro", abi: "arm64_v8a", dpi: 429 },
    Device { brand: "Honor", model: "Magic6 Pro", abi: "arm64_v8a", dpi: 452 },
    Device { brand: "Honor", model: "90", abi: "arm64_v8a", dpi: 436 },
    Device { brand: "Honor", model: "X9a", abi: "arm64_v8a", dpi: 387 },
    Device { brand: "OPPO", model: "Find X7", abi: "arm64_v8a", dpi: 447 },
    Device { brand: "OPPO", model: "Reno 11", abi: "arm64_v8a", dpi: 394 },
    Device { brand: "OPPO", model: "Reno 12 Pro", abi: "arm64_v8a", dpi: 458 },
    Device { brand: "vivo", model: "X100", abi: "arm64_v8a", dpi: 460 },
    Device { brand: "vivo", model: "X90 Pro", abi: "arm64_v8a", dpi: 453 },
    Device { brand: "vivo", model: "V29", abi: "arm64_v8a", dpi: 388 },
    Device { brand: "realme", model: "GT 6", abi: "arm64_v8a", dpi: 450 },
    Device { brand: "realme", model: "12 Pro+", abi: "arm64_v8a", dpi: 394 },
    Device { brand: "ASUS", model: "ROG Phone 8", abi: "arm64_v8a", dpi: 388 },
];

pub(crate) fn brands() -> Vec<&'static str> {
    let mut brands: Vec<&'static str> = Vec::new();
    for device in DEVICES {
        if !brands.contains(&device.brand) {
            brands.push(device.brand);
        }
    }
    brands
}
