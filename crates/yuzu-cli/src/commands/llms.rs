//! `yuzu llms [--full]`: llms.txt / llms-full.txt をその場で生成して標準出力へ。
//! dist/ 不要のドライラン兼エクスポート（`yuzu llms --full | pbcopy` で LLM に直接渡せる）。
//! 明示実行なので `llms.enabled` に関わらず生成する（`yuzu search` と同じ思想）

pub fn run(full: bool) -> anyhow::Result<()> {
    let (_, rc) = super::load_project()?;

    let site = yuzu_core::build_site_model(
        &rc.content_dir,
        &rc.config.input.ignore,
        &yuzu_render::markdown_options(&rc.config),
    )?;

    let text = if full {
        yuzu_render::generate_llms_full_txt(&rc, &site, None)?
    } else {
        yuzu_render::generate_llms_txt(&rc, &site)?
    };
    // 全文を 1 回で書く（`| head` で下流が閉じても panic しない。out.rs 参照）
    crate::out::str(&text);
    Ok(())
}
