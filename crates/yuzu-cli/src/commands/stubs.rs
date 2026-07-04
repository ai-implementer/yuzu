//! 後続フェーズのサブコマンドのスタブ

/// "not implemented" を表示して非ゼロ終了する
pub fn not_implemented(name: &str) -> ! {
    eprintln!("yuzu {name}: not implemented in v0.1 (see roadmap in README.md)");
    std::process::exit(1);
}
