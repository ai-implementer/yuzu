//! WebSocket ライブリロード（`yuzu dev` 用）。
//!
//! 再ビルド完了を接続中の全ブラウザへ broadcast する。
//! 「いつ notify するか」は呼び出し側（cli の watch クロージャ）が決める
//! （依存方向の凍結: server は render/config を知らない）。

use axum::extract::ws::{Message, WebSocket};
use tokio::sync::broadcast;

/// WS エンドポイントのパス。base_url に依らず常にルート直下
/// （dev サーバは yuzu がオリジン全体を占有するローカル用途のため）
pub const LIVERELOAD_PATH: &str = "/__livereload";

/// リロード通知のハンドル。clone して watch クロージャへ渡し、
/// **再ビルド成功後に** [`ReloadNotifier::notify`] を呼ぶ
#[derive(Clone)]
pub struct ReloadNotifier {
    tx: broadcast::Sender<()>,
}

impl ReloadNotifier {
    pub fn new() -> Self {
        // 通知は「最新の 1 回」だけ意味を持つので容量は小さくてよい
        Self {
            tx: broadcast::channel(16).0,
        }
    }

    /// 接続中の全クライアントに reload を送る。
    /// 同期メソッドなので監視スレッド（非 async）から直接呼べる。
    /// 受信者ゼロ（タブ未接続）は正常なので無視する
    pub fn notify(&self) {
        let _ = self.tx.send(());
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
}

impl Default for ReloadNotifier {
    fn default() -> Self {
        Self::new()
    }
}

/// 1 接続ぶんの WS ループ。reload 通知を送りつつ、切断を検知したら終了する
pub(crate) async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<()>) {
    loop {
        tokio::select! {
            result = rx.recv() => match result {
                // Lagged（通知の取りこぼし）も「再ビルドがあった」事実は同じなので reload 扱い
                Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                    if socket.send(Message::text("reload")).await.is_err() {
                        break; // クライアント切断
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            // 切断（None / Err）の検知用。受信内容は使わない
            // （Ping への Pong は axum が自動応答する）
            msg = socket.recv() => match msg {
                None | Some(Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReloadNotifier;

    #[test]
    fn 受信者ゼロでも_notify_は_panic_しない() {
        ReloadNotifier::new().notify();
    }
}
