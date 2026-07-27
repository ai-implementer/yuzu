//! yuzu CLI の標準出力を 1 箇所に集約する。
//!
//! Rust std は起動時に SIGPIPE を `SIG_IGN` にするため、`println!` は下流が閉じた
//! パイプ（`| head` / `| grep -q`）へ書くと内部の unwrap で panic する
//! （`failed printing to stdout: Broken pipe`・終了コード 101）。
//!
//! ここでは **BrokenPipe を「以降の出力を捨てる合図」**として扱い、
//! **終了コード規約 0 / 1 / 2 には一切影響させない**（`yuzu fmt --diff | head` は
//! 整形差分が実在する以上 1 のまま。下流が読むのをやめただけで事実は変わらない）。
//! BrokenPipe 以外の I/O エラー（リダイレクト先のディスクフル等）はラッチして
//! [`finish`] が返し、`main` が 2 にする。
//!
//! この規律を機械的に守るため、`main.rs` で `clippy::print_stdout` を deny している。
//! 新しく `println!` を書くと clippy が即座に落とす。

use std::io::{self, Write};
use std::sync::Mutex;

/// 標準出力の状態。**I/O の宛先を引数で受ける純粋な中核**にしてテスト可能にする
/// （グローバル状態をテストが触ると、同一プロセス並列の `cargo test` で漏れる）
#[derive(Default)]
struct Sink {
    /// 下流が閉じた（BrokenPipe）。以降の書き出しは捨てる
    closed: bool,
    /// BrokenPipe 以外の I/O エラー。最初の 1 件だけ保持して finish が返す
    latched: Option<io::Error>,
}

impl Sink {
    fn write(&mut self, w: &mut impl Write, s: &str, newline: bool) {
        if self.closed || self.latched.is_some() {
            return;
        }
        let result = match newline {
            true => writeln!(w, "{s}"),
            false => write!(w, "{s}"),
        };
        if let Err(e) = result {
            match e.kind() {
                io::ErrorKind::BrokenPipe => self.closed = true,
                _ => self.latched = Some(e),
            }
        }
    }

    /// 溜まっているぶんを書き出す。BrokenPipe は無視する
    fn flush(&mut self, w: &mut impl Write) {
        if self.closed {
            return;
        }
        if let Err(e) = w.flush() {
            match e.kind() {
                io::ErrorKind::BrokenPipe => self.closed = true,
                _ => {
                    self.latched.get_or_insert(e);
                }
            }
        }
    }
}

static SINK: Mutex<Sink> = Mutex::new(Sink {
    closed: false,
    latched: None,
});

/// 1 行書く（末尾に改行を付ける）
pub(crate) fn line(s: &str) {
    let mut stdout = io::stdout().lock();
    SINK.lock().unwrap().write(&mut stdout, s, true);
}

/// 改行を付けずに書く（`llms` の全文一括・`fmt --diff` のファイル単位）
pub(crate) fn str(s: &str) {
    let mut stdout = io::stdout().lock();
    SINK.lock().unwrap().write(&mut stdout, s, false);
}

/// 標準出力を flush し、BrokenPipe 以外の I/O エラーがあれば返す
/// （`main` がこれを 2 として報告する）
pub(crate) fn finish() -> Result<(), io::Error> {
    let mut stdout = io::stdout().lock();
    let mut sink = SINK.lock().unwrap();
    sink.flush(&mut stdout);
    match sink.latched.take() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// `println!` の置き換え。書式指定はそのまま使える
macro_rules! outln {
    () => { $crate::out::line("") };
    ($($arg:tt)*) => { $crate::out::line(&format!($($arg)*)) };
}

pub(crate) use outln;

#[cfg(test)]
mod tests {
    use super::*;

    /// 指定した ErrorKind を返すだけの Writer
    struct FailingWriter {
        kind: io::ErrorKind,
        /// 実際に書けたバイト数（打ち切り後に書きに来ていないかの検査用）
        writes: usize,
    }

    impl Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            Err(io::Error::new(self.kind, format!("{} bytes", buf.len())))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn 下流が閉じたら打ち切りになりエラーは残らない() {
        let mut w = FailingWriter {
            kind: io::ErrorKind::BrokenPipe,
            writes: 0,
        };
        let mut sink = Sink::default();
        sink.write(&mut w, "1 行目", true);
        assert!(sink.closed);
        assert!(sink.latched.is_none(), "BrokenPipe は失敗として扱わない");
    }

    #[test]
    fn 打ち切り後の書き出しは_writer_へ届かない() {
        let mut w = FailingWriter {
            kind: io::ErrorKind::BrokenPipe,
            writes: 0,
        };
        let mut sink = Sink::default();
        sink.write(&mut w, "1 行目", true);
        sink.write(&mut w, "2 行目", true);
        sink.write(&mut w, "3 行目", false);
        assert_eq!(w.writes, 1, "閉じた後は書きに行かない");
    }

    #[test]
    fn broken_pipe_以外の_io_エラーはラッチされる() {
        let mut w = FailingWriter {
            kind: io::ErrorKind::StorageFull,
            writes: 0,
        };
        let mut sink = Sink::default();
        sink.write(&mut w, "1 行目", true);
        assert!(!sink.closed);
        let latched = sink.latched.as_ref().expect("エラーが保持される");
        assert_eq!(latched.kind(), io::ErrorKind::StorageFull);
        // 以降は書きに行かない（同じエラーを繰り返し出さない）
        sink.write(&mut w, "2 行目", true);
        assert_eq!(w.writes, 1);
    }

    #[test]
    fn 正常な_writer_には改行付きと改行なしが書き分けられる() {
        let mut buf: Vec<u8> = Vec::new();
        let mut sink = Sink::default();
        sink.write(&mut buf, "行", true);
        sink.write(&mut buf, "続き", false);
        assert_eq!(String::from_utf8(buf).unwrap(), "行\n続き");
    }
}
