//! パーサが任意入力で panic / hang しないこと（不正な UTF-8 境界は前段で弾く）

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = core::str::from_utf8(data) {
        let _ = kabosu::Document::parse(src);
    }
});
