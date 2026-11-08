//! 図の描画（tankan の入口 `render_svg`）が、任意の入力でハング・メモリ超過をしないこと。
//! panic は yuzu がクライアント描画に切り替えて回収するが、ハングとメモリ超過は build ごと止まる
//! （v0.18 Phase 77 の gantt の panic・state の循環によるメモリ超過）。種は tests/corpus の .mmd
//! （fuzz.yml が平らにして corpus に置く。Phase 92）

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(source) = core::str::from_utf8(data) {
        let _ = tankan::render_svg(source, &tankan::Options::default());
    }
});
