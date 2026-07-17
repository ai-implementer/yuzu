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
use yuzu_core::{BuildCache, MarkdownOptions, OutputTracker, output};
use yuzu_render::{LiveReloadMode, RenderCtx, RenderParams, RenderShared};

use crate::commands::preview;

/// エディタの連続保存をまとめる debounce 幅（build --watch / dev 共通）
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(300);

pub fn run(watch: bool, base_url: Option<String>, force: bool, drafts: bool) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().context("カレントディレクトリを取得できません")?;
    let root = yuzu_config::find_project_root(&cwd)?;
    let mut rc = yuzu_config::load(&root)?;
    // --base-url は site/build の設定より優先（CI から配信パスを注入する用途）。
    // write_resolved より前に上書きし、.yuzu/settings.json にも反映する
    if let Some(raw) = base_url {
        rc.base_url = yuzu_config::normalize_base_url(&raw);
    }
    yuzu_config::write_resolved(&rc)?;

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

    // 監視対象は content/ と theme/ のみ（dist/ を見ると無限ループ）。
    // 設定は起動時のもので固定（yuzu.jsonc の変更は再起動で反映）
    let mut paths = vec![rc.content_dir.clone()];
    if let Some(theme_dir) = &rc.theme_dir {
        paths.push(theme_dir.clone());
    }
    let rc_for_watch = rc.clone();
    // session はクロージャへ move してセッション全体で再利用する
    //（キャッシュ・テンプレート Env・ハイライタ・トークナイザ）
    let _watch_handle = yuzu_server::watch(&paths, DEBOUNCE, move || {
        tracing::info!("変更を検知 → 再ビルド");
        if let Err(e) = build_once(&rc_for_watch, LiveReloadMode::Poll, &mut session, drafts) {
            // 執筆中の一時的な構文エラー等でプロセスは落とさない
            tracing::error!("再ビルドに失敗しました: {e:#}");
        }
    })?;

    // 受け入れ条件「編集 → ブラウザ自動更新」を 1 コマンドで満たすため、
    // preview と同じ静的サーバも起動する（ブロッキング）
    preview::serve_dist(&rc, None)
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
        if force && cache_dir.exists() {
            fs::remove_dir_all(&cache_dir)
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
    };
    let site = yuzu_core::build_site_model_cached(
        &rc.content_dir,
        &rc.config.input.ignore,
        &md_opts,
        Some(&session.cache),
        include_drafts,
    )?;

    // routesKey: 非 draft ページの rel→route 集合（`.md` リンク解決の入力）。
    // 変化時はキャッシュ層が本文 HTML だけを安全側で全破棄する
    let routes: Vec<String> = site
        .pages
        .iter()
        .map(|p| format!("{}\t{}", p.rel.display(), p.route))
        .collect();
    session
        .cache
        .set_routes_key(BuildCache::sha256_hex_parts(&[routes
            .join("\n")
            .as_bytes()]));

    // 前回の出力マニフェスト。無い（初回・--force 後・破損）なら既知状態がないので、
    // output.clean に従い dist を作り直してから全書き出しする
    let previous = output::load_manifest(&session.manifest_path);
    if previous.is_none() && rc.config.output.clean && rc.output_dir.exists() {
        fs::remove_dir_all(&rc.output_dir)
            .with_context(|| format!("dist を削除できません: {}", rc.output_dir.display()))?;
    }

    // git 連携メタ（有効時のみ。git 不在・リポジトリ外は None に縮退）
    let git_dates = rc
        .config
        .git
        .last_updated
        .then(|| collect_git_dates(rc))
        .flatten();

    let tracker = OutputTracker::new(&rc.output_dir);
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
        orphans_removed = removed,
        "インクリメンタルビルド"
    );
    Ok(())
}

/// content 配下ファイルの最終コミット日（YYYY-MM-DD）を 1 回の `git log` で収集する。
/// キーは content 相対の `/` 区切りパス。git 不在・リポジトリ外・失敗時は None（表示なしに縮退）
fn collect_git_dates(rc: &ResolvedConfig) -> Option<std::collections::HashMap<String, String>> {
    // core.quotepath=false: 日本語ファイル名がオクタルエスケープされるのを防ぐ
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(&rc.root)
        .args([
            "-c",
            "core.quotepath=false",
            "log",
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
