//! rust-embed は埋め込み対象フォルダへの**新規ファイル追加**を cargo の
//! 再コンパイル判定に伝えられない（マクロ展開が生成する include_bytes! は
//! 展開時点で存在したファイルの変更しか追跡しない）。その結果、テンプレートや
//! アセットを追加してもリリースビルドが古い埋め込みを使い回し、
//! 「debug では動くのに release で template not found」になる。
//! assets/ をディレクトリごと監視対象に登録して、追加・削除・変更のすべてで
//! この crate を確実に再コンパイルさせる（cargo はディレクトリ指定の
//! rerun-if-changed を再帰的にスキャンする）

fn main() {
    println!("cargo:rerun-if-changed=assets");
}
