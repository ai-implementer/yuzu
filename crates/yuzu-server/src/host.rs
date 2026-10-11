//! dev / preview が受け付ける Host / Origin の判定（DNS リバインディング対策。Phase 83）。
//!
//! 手元のサーバは既定で 127.0.0.1 にしか bind しないが、DNS リバインディングでは
//! 外部のサイトが自分のドメインを 127.0.0.1 へ向け直し、ブラウザ経由で配信内容
//! （原稿の `.md` を含む）を読める。このとき要求の `Host` は攻撃側のドメイン名になるので、
//! 次のものだけを受け付ける（Vite の `server.allowedHosts` と同じ考え方）。
//!
//! - `localhost` と `*.localhost`
//! - IP アドレス（v4・v6）。リバインディングはドメイン名を使うので、IP で開く要求は対象外。
//!   `dev.host = "0.0.0.0"` で LAN に公開し、LAN の IP で開く使い方は設定なしで通る
//! - 設定 `dev.allowed_hosts` に書いたホスト名（`.` で始めるとそのドメインとサブドメイン）

use std::net::IpAddr;

/// 受け付ける Host の規則（[`HostPolicy::allows`]）
#[derive(Debug, Clone, Default)]
pub(crate) struct HostPolicy {
    /// 正規化済みの `dev.allowed_hosts`（小文字・末尾の `.` とポートを除いたもの）
    allowed: Vec<String>,
}

impl HostPolicy {
    pub(crate) fn new(allowed_hosts: &[String]) -> Self {
        let allowed = allowed_hosts
            .iter()
            .map(|entry| normalize(strip_port(entry.trim())))
            .filter(|entry| !entry.is_empty() && entry != ".")
            .collect();
        Self { allowed }
    }

    /// `Host` ヘッダ（または HTTP/2 の `:authority`）の値を受け付けるか。
    /// ポートは見ない（bind 先のポート以外で届くことは無い）
    pub(crate) fn allows_authority(&self, authority: &str) -> bool {
        let host = normalize(strip_port(authority.trim()));
        if host.is_empty() {
            return false;
        }
        if host == "localhost" || host.ends_with(".localhost") {
            return true;
        }
        if host.parse::<IpAddr>().is_ok() {
            return true;
        }
        self.allowed
            .iter()
            .any(|entry| match entry.strip_prefix('.') {
                Some(domain) => host == domain || host.ends_with(entry.as_str()),
                None => host == *entry,
            })
    }

    /// WebSocket の `Origin`（`http://localhost:5173` の形）を受け付けるか。
    /// `null`（ファイルやサンドボックスから開いたページ）と読めない値は拒否する
    pub(crate) fn allows_origin(&self, origin: &str) -> bool {
        let Some((_, rest)) = origin.trim().split_once("://") else {
            return false;
        };
        let authority = rest.split('/').next().unwrap_or("");
        // `user@host` の形は Origin に現れないが、念のため @ より後ろだけを見る
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        self.allows_authority(authority)
    }
}

/// 末尾のポートを外す（`localhost:5173` → `localhost`・`[::1]:5173` → `::1`）。
/// 括弧の無い IPv6（`::1`）はそのまま返す
fn strip_port(authority: &str) -> &str {
    if let Some(rest) = authority.strip_prefix('[') {
        // `[v6]` または `[v6]:port`
        return rest.split(']').next().unwrap_or(rest);
    }
    match authority.rsplit_once(':') {
        // `:` が 1 つだけなら host:port。2 つ以上は括弧なしの IPv6
        Some((host, port))
            if !host.contains(':')
                && !port.is_empty()
                && port.bytes().all(|b| b.is_ascii_digit()) =>
        {
            host
        }
        _ => authority,
    }
}

/// 大文字小文字を区別せず、末尾の `.`（完全修飾の書き方）を外す
fn normalize(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(allowed: &[&str]) -> HostPolicy {
        HostPolicy::new(&allowed.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn localhost_と_ip_アドレスは設定なしで受け付ける() {
        let p = policy(&[]);
        for host in [
            "localhost",
            "localhost:5173",
            "LOCALHOST:5173",
            "localhost.",
            "docs.localhost:5173",
            "127.0.0.1",
            "127.0.0.1:5173",
            "192.168.1.20:5173", // dev.host = "0.0.0.0" で LAN の IP から開く
            "[::1]:5173",
            "[::1]",
            "::1",
            "[fe80::1]:8080",
        ] {
            assert!(p.allows_authority(host), "{host}");
        }
    }

    #[test]
    fn それ以外のホスト名は拒否する() {
        let p = policy(&[]);
        for host in [
            "evil.example",
            "evil.example:5173",
            "localhost.evil.example",
            "127.0.0.1.nip.io",
            "",
            ":5173",
        ] {
            assert!(!p.allows_authority(host), "{host}");
        }
    }

    #[test]
    fn 許可リストは完全一致と先頭の点によるサブドメインを受け付ける() {
        let p = policy(&["mypc.local", ".example.internal", "Docs.Corp:8080"]);
        assert!(p.allows_authority("mypc.local:5173"));
        assert!(p.allows_authority("MyPC.local"));
        assert!(
            !p.allows_authority("other.mypc.local"),
            "点で始めなければ完全一致"
        );
        assert!(p.allows_authority("example.internal"));
        assert!(p.allows_authority("a.b.example.internal:5173"));
        assert!(!p.allows_authority("notexample.internal"));
        // 書き手がポートや大文字を付けても照合できる
        assert!(p.allows_authority("docs.corp:5173"));
        assert!(!p.allows_authority("evil.example"));
    }

    #[test]
    fn 空や点だけの許可は何も許さない() {
        let p = policy(&["", " ", "."]);
        assert!(!p.allows_authority("evil.example"));
    }

    #[test]
    fn origin_はホスト部を同じ規則で照合する() {
        let p = policy(&["mypc.local"]);
        assert!(p.allows_origin("http://localhost:5173"));
        assert!(p.allows_origin("http://127.0.0.1:5173"));
        assert!(p.allows_origin("http://[::1]:5173"));
        assert!(p.allows_origin("https://mypc.local"));
        assert!(!p.allows_origin("http://evil.example"));
        assert!(!p.allows_origin("http://evil.example:5173/path"));
        assert!(!p.allows_origin("null"));
        assert!(!p.allows_origin(""));
    }
}
