//! `yuzu build [--watch]`: サイトのビルド（と監視・配信）
//!
//! ビルドは常にインクリメンタル（`.yuzu/cache/`）。正しさはキャッシュ層が
//! envKey / routesKey / sourceHash で担保し、ここでは配線だけを行う。
//! `--force` でキャッシュを破棄してフルビルドに戻せる。

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;

use yuzu_config::ResolvedConfig;
use yuzu_core::{BuildCache, IgnoreMatcher, MarkdownOptions, OutputTracker, output};
use yuzu_render::{LiveReloadMode, RenderCtx, RenderParams, RenderShared};

use crate::commands::preview;

/// エディタの連続保存をまとめる debounce 幅（build --watch / dev 共通）
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(300);

/// 監視除外（build --watch / dev 共通）。
/// **出力ディレクトリの除外は必須**（含めると再ビルド → 変更検知の無限ループ）。
/// `.yuzu` / `.git` 等の隠しディレクトリは yuzu_server 側で常に無視される。
///
/// これに加えて `build.watchIgnore` の glob をプロジェクトルート相対で評価する
/// （既定は `target/` と `node_modules/`。ビルド生成物の大量イベントで
/// 再ビルドが暴発するのを防ぐ）。glob の解釈は `input.ignore` と同じ
/// yuzu-core の [`IgnoreMatcher`] を通す
pub(crate) fn watch_ignore(rc: &ResolvedConfig) -> anyhow::Result<yuzu_server::WatchIgnore> {
    let matcher = IgnoreMatcher::new(&rc.config.build.watch_ignore)
        .context("build.watchIgnore のパターンが不正です")?;
    let root = rc.root.clone();
    Ok(
        yuzu_server::WatchIgnore::new(vec![rc.output_dir.clone(), rc.root.join(".yuzu")])
            .with_extra(move |path| {
                // 監視イベントは絶対パスで来る。ルート外（シンボリックリンク先など）は
                // 相対化できないので除外しない。
                // 祖先まで見るのは、ディレクトリ作成イベント（`target` そのもの）と
                // その配下のファイルを 1 つのパターンで扱うため
                path.strip_prefix(&root)
                    .is_ok_and(|rel| matcher.is_match_or_ancestor(rel))
            }),
    )
}

/// CLI フラグによる設定の上書き（`--base-url` / `--host`）。
/// watch 中に `yuzu.jsonc` を読み直してもフラグ優先の契約を保つため保持する
#[derive(Clone, Default)]
pub(crate) struct Overrides {
    /// `--base-url`（build）。site/build の設定より優先
    pub(crate) base_url: Option<String>,
    /// `--host`（dev）。dev.host より優先（コンテナ内から 0.0.0.0 で配信する用途）
    pub(crate) host: Option<String>,
}

impl Overrides {
    fn apply(&self, rc: &mut ResolvedConfig) {
        if let Some(raw) = &self.base_url {
            rc.base_url = yuzu_config::normalize_base_url(raw);
        }
        if let Some(host) = &self.host {
            rc.config.dev.host = host.clone();
        }
    }
}

/// プロジェクトルートを探して設定を読み、CLI 上書きを当ててから
/// `.yuzu/settings.json` へ書き出す（build / dev 共通の入口）
pub(crate) fn load_config(overrides: &Overrides) -> anyhow::Result<ResolvedConfig> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let mut rc = yuzu_config::load(&root)?;
    // 上書きは write_resolved より前に当てる（.yuzu/settings.json にも反映する）
    overrides.apply(&mut rc);
    // ツール管理ディレクトリの経路も検証する。ここは settings.json の書き出しと
    // `BuildSession::new` のキャッシュ破棄（--force）の**両方より前**なので、
    // `.yuzu` 系の書き込み・削除を 1 箇所で覆える
    // （yuzu-config は yuzu-core に依存しないので、settings.json の検証も cli の責務）
    for rel in [".yuzu", ".yuzu/settings.json", ".yuzu/cache"] {
        let path = rc.root.join(rel);
        yuzu_core::output::ensure_no_symlink_under(&rc.root, &path)
            .with_context(|| format!("{} を使えません", path.display()))?;
    }
    yuzu_config::write_resolved(&rc)?;
    Ok(rc)
}

pub fn run(watch: bool, base_url: Option<String>, force: bool, drafts: bool) -> anyhow::Result<()> {
    let overrides = Overrides {
        base_url,
        host: None,
    };
    let rc = load_config(&overrides)?;

    // --watch のときだけオートリフレッシュ JS（ポーリング式）を注入する
    let mode = if watch {
        LiveReloadMode::Poll
    } else {
        LiveReloadMode::None
    };
    let mut session = BuildSession::new(&rc, force)?;
    build_once(&rc, mode, &mut session, drafts)?;

    if !watch {
        return Ok(());
    }

    // プロジェクトルート全体を監視する（コンテンツインクルード `file=` の
    // 参照先は content/ の外にもあるため）。出力ディレクトリは必ず除外する
    let paths = vec![rc.root.clone()];
    let ignore = watch_ignore(&rc)?;
    // session と設定はクロージャへ move してセッション全体で再利用する
    //（キャッシュ・テンプレート Env・ハイライタ・トークナイザ）
    let mut watcher = WatchBuild::new(rc.clone(), overrides, mode, drafts, session);
    let _watch_handle = yuzu_server::watch(&paths, ignore, DEBOUNCE, move || {
        tracing::info!("変更を検知 → 再ビルド");
        if let Err(e) = watcher.rebuild() {
            // 執筆中の一時的な構文エラー等でプロセスは落とさない
            tracing::error!("再ビルドに失敗しました: {e:#}");
        }
    })?;

    // 受け入れ条件「編集 → ブラウザ自動更新」を 1 コマンドで満たすため、
    // preview と同じ静的サーバも起動する（ブロッキング）
    preview::serve_dist(&rc, None)
}

/// 監視ビルド 1 本ぶんの状態（`build --watch` / `dev` 共通）。
///
/// **設定の持ち主はここ**。`yuzu.jsonc` も監視対象なので、起動時の設定で
/// 固定すると「保存 → 再ビルドもライブリロードも走るのに設定だけ効かない」
/// という気づきにくい状態になる（Phase 42 の副作用）
pub(crate) struct WatchBuild {
    rc: ResolvedConfig,
    session: BuildSession,
    overrides: Overrides,
    live_reload: LiveReloadMode,
    drafts: bool,
    /// 最後に読み込んだ `yuzu.jsonc` の生テキスト。
    /// 差分があるときだけ読み直す（無変更なら再ビルド 1 回あたり
    /// 小さいファイルの読み込み 1 回で済み、セッション再構築も起きない）
    config_text: String,
}

impl WatchBuild {
    pub(crate) fn new(
        rc: ResolvedConfig,
        overrides: Overrides,
        live_reload: LiveReloadMode,
        drafts: bool,
        session: BuildSession,
    ) -> Self {
        // 起動時に読めているので失敗はまず無い（空なら次回の差分判定で読み直す）
        let config_text =
            fs::read_to_string(rc.root.join(yuzu_config::CONFIG_FILE_NAME)).unwrap_or_default();
        Self {
            rc,
            session,
            overrides,
            live_reload,
            drafts,
            config_text,
        }
    }

    /// 変更検知 1 回ぶん。設定の変更を取り込んでから再ビルドする
    pub(crate) fn rebuild(&mut self) -> anyhow::Result<()> {
        self.reload_config();
        build_once(&self.rc, self.live_reload, &mut self.session, self.drafts)
    }

    /// `yuzu.jsonc` の変更を取り込む。読めない・不正なときは**前回の設定で続行**する
    /// （編集途中の壊れた JSONC でプロセスを落とさない。診断は load 側が警告する）
    fn reload_config(&mut self) {
        let path = self.rc.root.join(yuzu_config::CONFIG_FILE_NAME);
        let Ok(text) = fs::read_to_string(&path) else {
            return; // 一時的に消えた（エディタの保存方式）等
        };
        if text == self.config_text {
            return;
        }
        self.config_text = text;

        let mut next = match yuzu_config::load(&self.rc.root) {
            Ok(next) => next,
            Err(e) => {
                tracing::error!("yuzu.jsonc を読み込めません（前回の設定で続行します）: {e}");
                return;
            }
        };
        self.overrides.apply(&mut next);
        pin_restart_only(&mut next, &self.rc);

        // envKey が変わるのでセッションごと作り直す（キャッシュはディスクから
        // 読み直し、envKey 一致なら中身を引き継ぐので無駄な全再計算にはならない）。
        // force は渡さない（設定変更でユーザのキャッシュを消す理由はない）
        match BuildSession::new(&next, false) {
            Ok(session) => {
                self.session = session;
                self.rc = next;
                if let Err(e) = yuzu_config::write_resolved(&self.rc) {
                    tracing::warn!(".yuzu/settings.json を更新できません: {e}");
                }
                tracing::info!("yuzu.jsonc の変更を反映しました");
            }
            Err(e) => tracing::error!("設定の変更を反映できません（前回の設定で続行します）: {e}"),
        }
    }
}

/// 監視・配信の前提に焼き付いている設定を起動時の値へ戻し、
/// 再起動が必要だと警告する。
///
/// これらを watch 中に差し替えると壊れる:
/// - `output.dir` — 新しい出力先が**監視除外に入らない**（再ビルド → 変更検知の無限ループ）。
///   配信中のディレクトリでもある
/// - `baseUrl` / `dev.host` / `dev.port` — 起動済みサーバの bind と URL 接頭辞
/// - `dev.liveReload` — 注入済みの JS と WS 通知の有無
/// - `build.watchIgnore` — 監視除外の glob（起動時に監視スレッドへ渡している）
fn pin_restart_only(next: &mut ResolvedConfig, current: &ResolvedConfig) {
    let mut pinned: Vec<&str> = Vec::new();
    if next.config.output.dir != current.config.output.dir {
        pinned.push("output.dir");
        next.config.output.dir = current.config.output.dir.clone();
        next.output_dir = current.output_dir.clone();
    }
    if next.base_url != current.base_url {
        pinned.push("baseUrl");
        next.base_url = current.base_url.clone();
        next.config.site.base_url = current.config.site.base_url.clone();
        next.config.build.base_url = current.config.build.base_url.clone();
    }
    if next.config.build.watch_ignore != current.config.build.watch_ignore {
        pinned.push("build.watchIgnore");
        next.config.build.watch_ignore = current.config.build.watch_ignore.clone();
    }
    if next.config.dev.host != current.config.dev.host {
        pinned.push("dev.host");
        next.config.dev.host = current.config.dev.host.clone();
    }
    if next.config.dev.port != current.config.dev.port {
        pinned.push("dev.port");
        next.config.dev.port = current.config.dev.port;
    }
    if next.config.dev.live_reload != current.config.dev.live_reload {
        pinned.push("dev.liveReload");
        next.config.dev.live_reload = current.config.dev.live_reload;
    }
    if !pinned.is_empty() {
        tracing::warn!(
            "{} の変更は再起動しないと反映されません（起動時の値のままビルドします）",
            pinned.join(" / ")
        );
    }
}

/// ビルド間で再利用する状態一式。単発 build では 1 回だけ、
/// watch / dev では全再ビルドを通して使い回す
pub(crate) struct BuildSession {
    cache: BuildCache,
    shared: RenderShared,
    index_session: yuzu_index::IndexSession,
    manifest_path: PathBuf,
}

impl BuildSession {
    /// `.yuzu/cache/` を読み込む。force なら先に破棄する（＝全再計算＋dist 再クリーン）
    pub(crate) fn new(rc: &ResolvedConfig, force: bool) -> anyhow::Result<Self> {
        let cache_dir = rc.root.join(".yuzu/cache");
        if force {
            // 経路にシンボリックリンクがあればリンク先を消さずに中断する
            output::remove_dir_all_under(&rc.root, &cache_dir)
                .with_context(|| format!("キャッシュを削除できません: {}", cache_dir.display()))?;
        }
        Ok(Self {
            cache: BuildCache::load(&cache_dir, &env_key(rc)?),
            shared: RenderShared::new(rc)?,
            index_session: yuzu_index::IndexSession::default(),
            manifest_path: cache_dir.join("output-manifest.json"),
        })
    }
}

/// envKey: キャッシュ済みページ派生物に影響しうる全入力のハッシュ。
/// 不一致は全キャッシュ破棄（フルビルド）に縮退するだけなので、
/// 迷ったら含めて安全側に倒す
fn env_key(rc: &ResolvedConfig) -> anyhow::Result<String> {
    let config_json =
        serde_json::to_string(&rc.config).context("設定のシリアライズに失敗しました")?;
    // 辞書ファイルは設定（パス）が同じでも中身が変わりうるため内容ハッシュを採る
    let model = if rc.config.search.enabled {
        let dictionary = rc
            .config
            .search
            .dictionary
            .as_ref()
            .map(|p| rc.root.join(p));
        yuzu_index::model_fingerprint(dictionary.as_deref())?
    } else {
        String::new()
    };
    Ok(BuildCache::sha256_hex_parts(&[
        env!("CARGO_PKG_VERSION").as_bytes(),
        config_json.as_bytes(),
        rc.base_url.as_bytes(),
        model.as_bytes(),
    ]))
}

pub(crate) fn build_once(
    rc: &ResolvedConfig,
    live_reload: LiveReloadMode,
    session: &mut BuildSession,
    include_drafts: bool,
) -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    session.cache.begin_build();
    // watch 中のテーマ編集を拾うため、theme/ があれば毎回 Env だけ再構築する
    //（テンプレート解析は軽い。重い syntect 側はセッション共有のまま）
    if rc.theme_dir.is_some() {
        session.shared.reload_templates(rc.theme_dir.as_deref())?;
    }

    let md_opts = MarkdownOptions {
        gfm: rc.config.markdown.gfm,
        math: rc.config.markdown.math.enabled,
        mermaid: rc.config.markdown.mermaid.enabled,
        crossref_site_numbering: matches!(
            rc.config.markdown.crossref.numbering,
            yuzu_config::CrossrefNumbering::Site
        ),
    };
    let site = yuzu_core::build_site_model_cached(
        &rc.content_dir,
        &rc.config.input.ignore,
        &md_opts,
        Some(&session.cache),
        include_drafts,
    )?;

    // routesKey: 非 draft ページの rel→route 集合（`.md` リンク解決の入力）。
    // 変化時はキャッシュ層が本文 HTML だけを安全側で全破棄する。
    // サイト通し番号（crossref）では**先行ページの図表個数**も本文 HTML に効くので、
    // ラベル数もキーへ含める（あるページの図の増減で後続ページの番号がずれるため）
    let routes: Vec<String> = site
        .pages
        .iter()
        .map(|p| {
            if md_opts.crossref_site_numbering {
                format!("{}\t{}\t{}", p.rel.display(), p.route, p.labels.len())
            } else {
                format!("{}\t{}", p.rel.display(), p.route)
            }
        })
        .collect();
    session
        .cache
        .set_routes_key(BuildCache::sha256_hex_parts(&[routes
            .join("\n")
            .as_bytes()]));

    // ⚠️ ページの検証は**破壊的な clean より前**に行う。render_site の中でも
    // 検証するが、そこへ到達する前に dist を消してしまうと「不正なページのせいで
    // 既存の正常な成果物を失う」ことになる
    yuzu_render::validate_pages(&site, rc)?;

    // 前回の出力マニフェスト。無い（初回・--force 後・破損）なら既知状態がないので、
    // output.clean に従い dist を作り直してから全書き出しする
    let previous = output::load_manifest(&session.manifest_path);
    if previous.is_none() && rc.config.output.clean {
        output::remove_dir_all_under(&rc.root, &rc.output_dir)
            .with_context(|| format!("dist を削除できません: {}", rc.output_dir.display()))?;
    }

    // git 連携メタ（有効時のみ。git 不在・リポジトリ外は None に縮退）
    let git_dates = rc
        .config
        .git
        .last_updated
        .then(|| collect_git_dates(rc))
        .flatten();

    let tracker = OutputTracker::new(&rc.output_dir)
        .with_context(|| format!("出力先を使えません: {}", rc.output_dir.display()))?;
    yuzu_render::render_site(&RenderParams {
        config: rc,
        site: &site,
        live_reload,
        ctx: RenderCtx {
            cache: Some(&session.cache),
            outputs: Some(&tracker),
            shared: Some(&session.shared),
        },
        git_dates: git_dates.as_ref(),
    })?;

    // 検索インデックスは render の後（描画結果とは独立だが、ログ順を保つ）
    if rc.config.search.enabled {
        let search = &rc.config.search;
        // 同義語グループ = lint.terms（正表記＋ゆれ表記で 1 グループ）＋ search.synonyms。
        // lint が本文を正表記へ寄せ、検索がゆれ側を吸収する対の設計（Phase 20）
        let synonyms: Vec<Vec<String>> = rc
            .config
            .lint
            .terms
            .iter()
            .map(|(canonical, variants)| {
                let mut group = vec![canonical.clone()];
                group.extend(variants.iter().cloned());
                group
            })
            .chain(search.synonyms.iter().cloned())
            .collect();
        yuzu_index::build_search_index_with(
            &site,
            &md_opts,
            &yuzu_index::IndexParams {
                // 相対パスはプロジェクトルート基準
                dictionary: search.dictionary.as_ref().map(|p| rc.root.join(p)),
                typo_enabled: search.typo_tolerance.enabled,
                max_edits: search.typo_tolerance.max_edits.min(1),
                max_terms_per_shard: search.shard.max_terms_per_shard.max(1),
                synonyms,
                index_code: search.index_code,
                // コンテンツインクルード（file=）を索引へ展開するための基準
                project_root: Some(rc.root.clone()),
            },
            &rc.output_dir,
            &yuzu_index::IndexCtx {
                cache: Some(&session.cache),
                outputs: Some(&tracker),
                session: Some(&session.index_session),
            },
        )?;
    }

    // ここから下はビルド成功時のみ: 孤児掃除 → マニフェスト・キャッシュ保存
    let written = tracker.into_written();
    let removed = match &previous {
        Some(prev) => output::remove_orphans(&rc.output_dir, prev, &written)
            .context("孤児出力の削除に失敗しました")?,
        None => 0,
    };
    output::save_manifest(&session.manifest_path, &written)
        .context("出力マニフェストを保存できません")?;
    session
        .cache
        .save()
        .context("ビルドキャッシュを保存できません")?;

    let stats = session.cache.stats();
    tracing::info!(
        body_hits = stats.body_hits,
        body_misses = stats.body_misses,
        search_hits = stats.search_hits,
        search_misses = stats.search_misses,
        orphans_removed = removed,
        elapsed = %format!("{:.2}s", started.elapsed().as_secs_f64()),
        "インクリメンタルビルド"
    );
    Ok(())
}

/// content 配下ファイルの最終コミット日（YYYY-MM-DD）を 1 回の `git log` で収集する。
/// キーは content 相対の `/` 区切りパス。git 不在・リポジトリ外・失敗時は None（表示なしに縮退）
fn collect_git_dates(rc: &ResolvedConfig) -> Option<std::collections::HashMap<String, String>> {
    // core.quotepath=false: 日本語ファイル名がオクタルエスケープされるのを防ぐ。
    // --relative: --name-only のパスを rc.root 相対にする。これが無いと
    // パスは git リポジトリルート相対になり、yuzu プロジェクトがリポジトリの
    // サブディレクトリにある場合（例: monorepo 内の docs/）に
    // content_prefix の除去が全ファイルで失敗して日付が空になる
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(&rc.root)
        .args([
            "-c",
            "core.quotepath=false",
            "log",
            "--relative",
            "--format=\u{1}%cs",
            "--name-only",
            "--",
        ])
        .arg(&rc.content_dir)
        .output()
        .ok()?;
    if !output.status.success() {
        tracing::debug!("git log が失敗したため最終更新日は表示しません");
        return None;
    }

    // repo ルート相対 → content 相対への変換用プレフィクス（例: "content/"）
    let content_prefix = rc
        .content_dir
        .strip_prefix(&rc.root)
        .ok()?
        .iter()
        .map(|c| c.to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
        + "/";

    // 出力は新しいコミット順なので、最初に現れた日付がそのファイルの最終コミット日
    let mut dates = std::collections::HashMap::new();
    let mut current_date = String::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(date) = line.strip_prefix('\u{1}') {
            current_date = date.to_string();
        } else if let Some(rel) = line.strip_prefix(&content_prefix) {
            if !rel.is_empty() && !current_date.is_empty() {
                dates
                    .entry(rel.to_string())
                    .or_insert_with(|| current_date.clone());
            }
        }
    }
    Some(dates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 既定設定のプロジェクト（`/proj`）
    fn resolved(root: &Path) -> ResolvedConfig {
        let config = yuzu_config::Config::default();
        ResolvedConfig {
            content_dir: root.join(&config.input.dir),
            output_dir: root.join(&config.output.dir),
            theme_dir: None,
            public_dir: None,
            base_url: "/".to_string(),
            root: root.to_path_buf(),
            config,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn 監視除外は出力ディレクトリと_watch_ignore_の_glob() {
        let rc = resolved(Path::new("/proj"));
        let ignore = watch_ignore(&rc).unwrap();
        // 既定の watchIgnore（ビルド生成物）
        assert!(ignore.is_ignored(Path::new("/proj/target/debug/yuzu")));
        assert!(ignore.is_ignored(Path::new("/proj/web/node_modules/x/index.js")));
        // ディレクトリ作成イベント自体も除外する（これを取りこぼすと
        // `target/` が作られた瞬間に 1 回だけ再ビルドが走る）
        assert!(ignore.is_ignored(Path::new("/proj/target")));
        assert!(ignore.is_ignored(Path::new("/proj/target/debug")));
        // 出力ディレクトリ（除外必須。外すと再ビルドの無限ループ）
        assert!(ignore.is_ignored(Path::new("/proj/dist/index.html")));
        // 監視対象（インクルード参照先が content 外にもあるため src も対象）
        assert!(!ignore.is_ignored(Path::new("/proj/content/guide.md")));
        assert!(!ignore.is_ignored(Path::new("/proj/src/api.rs")));
        // パターンは「パス要素」に当たる必要がある（部分一致で消さない）
        assert!(!ignore.is_ignored(Path::new("/proj/content/target.md")));
        // ルート外は相対化できないので glob の対象外
        assert!(!ignore.is_ignored(Path::new("/other/target/x")));
    }

    #[test]
    fn watch_ignore_を空にすれば除外しない() {
        let mut rc = resolved(Path::new("/proj"));
        rc.config.build.watch_ignore = Vec::new();
        let ignore = watch_ignore(&rc).unwrap();
        assert!(!ignore.is_ignored(Path::new("/proj/target/debug/yuzu")));
        // 出力ディレクトリは設定に関係なく常に除外
        assert!(ignore.is_ignored(Path::new("/proj/dist/index.html")));
    }

    #[test]
    fn 不正な_glob_はエラーになる() {
        let mut rc = resolved(Path::new("/proj"));
        rc.config.build.watch_ignore = vec!["[".to_string()];
        assert!(watch_ignore(&rc).is_err());
    }

    #[test]
    fn 再起動が必要な設定は起動時の値へ戻す() {
        let current = resolved(Path::new("/proj"));
        let mut next = resolved(Path::new("/proj"));
        next.config.output.dir = "public_html".to_string();
        next.output_dir = Path::new("/proj/public_html").to_path_buf();
        next.base_url = "/docs/".to_string();
        next.config.site.base_url = Some("/docs/".to_string());
        next.config.dev.port = 9999;
        next.config.dev.live_reload = !current.config.dev.live_reload;
        next.config.build.watch_ignore = Vec::new();

        pin_restart_only(&mut next, &current);

        assert_eq!(next.output_dir, current.output_dir);
        assert_eq!(next.config.output.dir, current.config.output.dir);
        assert_eq!(next.base_url, current.base_url);
        assert_eq!(next.config.site.base_url, None);
        assert_eq!(next.config.dev.port, current.config.dev.port);
        assert_eq!(
            next.config.dev.live_reload, current.config.dev.live_reload,
            "注入済みの JS と WS 通知の有無は途中で変えられない"
        );
        assert_eq!(
            next.config.build.watch_ignore,
            current.config.build.watch_ignore
        );
    }

    #[test]
    fn 反映できる設定はそのまま通す() {
        let current = resolved(Path::new("/proj"));
        let mut next = resolved(Path::new("/proj"));
        next.config.site.title = "新しいタイトル".to_string();
        next.config.markdown.mermaid.enabled = !current.config.markdown.mermaid.enabled;
        next.config.input.ignore = vec!["**/_wip/**".to_string()];
        next.config.search.index_code = !current.config.search.index_code;

        pin_restart_only(&mut next, &current);

        assert_eq!(next.config.site.title, "新しいタイトル");
        assert_ne!(
            next.config.markdown.mermaid.enabled,
            current.config.markdown.mermaid.enabled
        );
        assert_eq!(next.config.input.ignore, ["**/_wip/**"]);
        assert_ne!(
            next.config.search.index_code,
            current.config.search.index_code
        );
    }

    #[test]
    fn cli_の上書きは設定リロード後も優先する() {
        let overrides = Overrides {
            base_url: Some("docs".to_string()),
            host: Some("0.0.0.0".to_string()),
        };
        let mut rc = resolved(Path::new("/proj"));
        rc.config.site.base_url = Some("/from-file/".to_string());
        overrides.apply(&mut rc);
        assert_eq!(rc.base_url, "/docs/");
        assert_eq!(rc.config.dev.host, "0.0.0.0");
    }
}
