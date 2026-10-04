//! 依存ライブラリの panic を回収して、ページの処理を続けるための仕組み。
//!
//! 使う場所は 2 つ:
//! - Mermaid の SSR（yuzu-render が呼ぶ tankan）— 利用者の入力をそのまま受けるので、
//!   tankan 側の不具合で panic してもビルド全体を落とさず、構文エラーと同じく
//!   クライアント描画へ切り替える
//! - comrak の整形（`markdown::catch_formatter_panic`）— 既知バグの panic で
//!   原文へ縮退する
//!
//! panic hook は `catch_unwind` で回収する panic でも先に実行される
//! （`std::panic::set_hook` の仕様）。回収する区間にいるかを [`is_recovering`] で
//! 公開し、CLI の hook はそれを見て既定の出力（`thread … panicked at …`）を抑える。
//! hook を差し替えて黙らせる方式は、並列に動く他スレッドの本物の panic まで
//! 黙らせるので使わない（区間の印はスレッドごと）。回収した panic は
//! 呼び出し側が警告 1 行で知らせる

use std::any::Any;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

thread_local! {
    static RECOVERING: Cell<bool> = const { Cell::new(false) };
}

/// このスレッドがいま panic を回収する区間にいるか（CLI の panic hook 用）
pub fn is_recovering() -> bool {
    RECOVERING.with(Cell::get)
}

/// `f` を実行し、panic したらメッセージを `Err` で返す。実行中は
/// [`is_recovering`] が true になる
pub fn catch<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    let prev = RECOVERING.with(|r| r.replace(true));
    let result = catch_unwind(AssertUnwindSafe(f));
    RECOVERING.with(|r| r.set(prev));
    result.map_err(|payload| panic_message(payload.as_ref()).to_string())
}

/// panic のペイロードからメッセージを取り出す（`panic!` の引数は `&str` か `String`）
pub fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s
    } else {
        "（メッセージなし）"
    }
}

#[cfg(test)]
mod tests {
    use super::{catch, is_recovering};

    #[test]
    fn panic_を回収してメッセージを返す() {
        let result: Result<(), String> = catch(|| panic!("テスト用の panic: {}", 1));
        assert_eq!(result.unwrap_err(), "テスト用の panic: 1");
        let result: Result<(), String> = catch(|| panic!("固定文字列"));
        assert_eq!(result.unwrap_err(), "固定文字列");
    }

    #[test]
    fn 回収区間の中だけ_is_recovering_が立つ() {
        assert!(!is_recovering());
        assert_eq!(catch(is_recovering), Ok(true));
        // panic で抜けても元に戻る
        let _ = catch(|| panic!("抜ける"));
        assert!(!is_recovering());
    }
}
