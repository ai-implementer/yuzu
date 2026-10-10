//! rust-embed は埋め込み対象フォルダへの**新規ファイル追加**を cargo の
//! 再コンパイル判定に伝えられない（マクロ展開が生成する include_bytes! は
//! 展開時点で存在したファイルの変更しか追跡しない）。assets/search/ に
//! ファイルを足してもリリースビルドが古い埋め込みを使い回さないよう、
//! assets/ をディレクトリごと監視対象に登録する（yuzu-theme の build.rs と同じ）

fn main() {
    println!("cargo:rerun-if-changed=assets");
}
