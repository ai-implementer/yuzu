//! セクション化されたドキュメント → postings/fst/シャード/manifest の集約。
//!
//! Markdown からのセクション抽出（tf・出現位置の計算）は呼び出し側（yuzu-index）の
//! 責務。ここでは計算済みの tf・位置を受け取り、doc_id 採番からバイナリ生成までを行う
//! （I/O は持たない。書き出しは呼び出し側の責務）。

use std::collections::BTreeMap;

use crate::FORMAT_VERSION;
use crate::error::FormatError;
use crate::manifest::{Bm25Params, Fragment, Manifest, ShardMeta, TokenizerMeta, TypoParams};
use crate::shard::{Posting, encode_shard};

/// 1 セクション（= 1 doc）ぶんの入力。tf・出現位置は呼び出し側が計算済み
pub struct SectionInput {
    pub anchor: Option<String>,
    pub heading: Option<String>,
    /// 動的抜粋用のセクション全文
    pub text: String,
    /// 重み付き文書長
    pub doc_len: u32,
    /// (term, 重み付き出現数, 出現位置の昇順列)
    pub tf: Vec<(String, u32, Vec<u32>)>,
}

/// 1 ドキュメント（yuzu では 1 ページ）ぶんの入力。1 ページは複数セクション（doc）を持つ
pub struct DocumentInput {
    pub title: String,
    /// サイト相対 URL（route）
    pub url: String,
    /// 結果の絞り込み単位（yuzu ではナビ第 1 階層）の表示名。`None` なら未分類。
    /// ⚠️ 下の `sections`（h2/h3 = doc）とは**別概念**
    pub group: Option<String>,
    pub sections: Vec<SectionInput>,
}

/// インデックス生成のオプション
pub struct BuildOptions {
    pub tokenizer: TokenizerMeta,
    pub bm25: Bm25Params,
    pub typo: TypoParams,
    pub max_terms_per_shard: u32,
    /// 同義語グループ（正規化前）。クエリ拡張に使われる
    pub synonyms: Vec<Vec<String>>,
    /// グループの表示名（この順で採番する）。ここに無い名前は初出順で末尾へ追加される。
    /// 空なら全部初出順（＝ 呼び出し側が順序を気にしない場合の既定）
    pub groups: Vec<String>,
}

/// [`build`] の出力。書き出しは呼び出し側の責務（I/O を持たない）
pub struct BuiltIndex {
    /// `content_hash` は空文字のまま返す。シャードバイト列にモデルバイト等を
    /// 加えたハッシュ計算は呼び出し側の責務（このクレートに sha2 依存を持ち込まないため）
    pub manifest: Manifest,
    pub terms_fst: Vec<u8>,
    /// (`index/NNNN.bin` 形式のファイル名, バイト列)。manifest.shards と同じ並び順
    pub shards: Vec<(String, Vec<u8>)>,
    /// doc_id 昇順（fragment/<doc_id>.json に対応）
    pub fragments: Vec<Fragment>,
}

/// ドキュメント列から postings/fst/シャード/manifest を構築する。
/// 決定性を保つため doc_id は `docs` の順序（ページ順→セクション順）どおりに採番する
pub fn build(docs: &[DocumentInput], opts: &BuildOptions) -> Result<BuiltIndex, FormatError> {
    let mut doc_lens: Vec<u32> = Vec::new();
    let mut terms: BTreeMap<String, Vec<Posting>> = BTreeMap::new();
    let mut fragments: Vec<Fragment> = Vec::new();
    let mut doc_id: u32 = 0;
    // グループは opts.groups の順で先に採番し、未宣言の名前は初出順で末尾へ足す
    // （同じ入力なら同じ manifest バイト列になる = 決定性を保つ）
    let mut groups: Vec<String> = opts.groups.clone();
    let mut doc_groups: Vec<u16> = Vec::new();

    for doc in docs {
        let group_id = doc.group.as_ref().map_or(crate::UNGROUPED, |name| {
            match groups.iter().position(|g| g == name) {
                Some(i) => i as u16,
                None => {
                    groups.push(name.clone());
                    (groups.len() - 1) as u16
                }
            }
        });
        for section in &doc.sections {
            doc_lens.push(section.doc_len);
            doc_groups.push(group_id);
            // doc_id 昇順で処理しているので postings は自然に昇順になる
            for (term, count, positions) in &section.tf {
                terms.entry(term.clone()).or_default().push(Posting {
                    doc_id,
                    tf: *count,
                    positions: positions.clone(),
                });
            }
            fragments.push(Fragment {
                title: doc.title.clone(),
                heading: section.heading.clone(),
                url: doc.url.clone(),
                anchor: section.anchor.clone(),
                text: section.text.clone(),
            });
            doc_id += 1;
        }
    }

    // term 辞書（fst は辞書順挿入が必須。BTreeMap の走査順で満たす）
    let mut fst_builder = fst::MapBuilder::memory();
    for (term_id, term) in terms.keys().enumerate() {
        fst_builder.insert(term, term_id as u64)?;
    }
    let terms_fst = fst_builder.into_inner()?;

    // postings の doc_id 昇順を保証（HashMap 経由でも上の理由で保たれるが、明示的に）
    let mut postings: Vec<Vec<Posting>> = terms.into_values().collect();
    for p in &mut postings {
        p.sort_unstable_by_key(|posting| posting.doc_id);
    }

    // シャード分割（term_id の連続範囲）
    let chunk = opts.max_terms_per_shard.max(1) as usize;
    let mut shards: Vec<(String, Vec<u8>)> = Vec::new();
    let mut shards_meta: Vec<ShardMeta> = Vec::new();
    for (i, chunk_postings) in postings.chunks(chunk).enumerate() {
        let file = format!("index/{i:04}.bin");
        shards.push((file.clone(), encode_shard(chunk_postings)));
        let start = (i * chunk) as u32;
        shards_meta.push(ShardMeta {
            file,
            term_start: start,
            term_end: start + chunk_postings.len() as u32,
        });
    }

    let avg_doc_len = if doc_lens.is_empty() {
        0.0
    } else {
        doc_lens.iter().map(|&l| l as f64).sum::<f64>() as f32 / doc_lens.len() as f32
    };

    let manifest = Manifest {
        version: FORMAT_VERSION,
        tokenizer: opts.tokenizer.clone(),
        bm25: opts.bm25,
        typo: opts.typo,
        doc_count: fragments.len() as u32,
        avg_doc_len,
        doc_lens,
        term_count: postings.len() as u32,
        terms_file: "terms.fst".to_string(),
        shards: shards_meta,
        synonyms: normalized_synonyms(&opts.synonyms),
        content_hash: String::new(),
        doc_groups,
        groups,
    };

    Ok(BuiltIndex {
        manifest,
        terms_fst,
        shards,
        fragments,
    })
}

/// 同義語グループを正規化する（決定的な manifest のため）:
/// グループ内の重複・空文字列を除去してソートし、1 語以下のグループを捨て、
/// グループ列自体もソートする
fn normalized_synonyms(groups: &[Vec<String>]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = groups
        .iter()
        .map(|group| {
            let mut g: Vec<String> = group.iter().filter(|m| !m.is_empty()).cloned().collect();
            g.sort();
            g.dedup();
            g
        })
        .filter(|g| g.len() >= 2)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> BuildOptions {
        BuildOptions {
            tokenizer: TokenizerMeta {
                kind: "vaporetto".to_string(),
                model_file: "model.zst".to_string(),
                model_sha256: "abc".to_string(),
            },
            bm25: Bm25Params::default(),
            typo: TypoParams {
                enabled: true,
                max_edits: 1,
            },
            max_terms_per_shard: 16384,
            synonyms: Vec::new(),
            groups: Vec::new(),
        }
    }

    /// グループ指定つきの入力を作るヘルパ
    fn doc_in(
        title: &str,
        url: &str,
        group: Option<&str>,
        sections: Vec<SectionInput>,
    ) -> DocumentInput {
        DocumentInput {
            title: title.to_string(),
            url: url.to_string(),
            group: group.map(str::to_string),
            sections,
        }
    }

    fn section(text: &str, tf: Vec<(&str, u32, Vec<u32>)>) -> SectionInput {
        SectionInput {
            anchor: None,
            heading: None,
            text: text.to_string(),
            doc_len: tf.iter().map(|(_, c, _)| c).sum(),
            tf: tf
                .into_iter()
                .map(|(t, c, p)| (t.to_string(), c, p))
                .collect(),
        }
    }

    #[test]
    fn doc_id_はページ順セクション順で採番される() {
        let docs = vec![
            DocumentInput {
                title: "A".to_string(),
                url: "a/".to_string(),
                group: None,
                sections: vec![section("alpha", vec![("alpha", 1, vec![0])])],
            },
            DocumentInput {
                title: "B".to_string(),
                url: "b/".to_string(),
                group: None,
                sections: vec![
                    section("beta", vec![("beta", 1, vec![0])]),
                    section("gamma", vec![("gamma", 1, vec![0])]),
                ],
            },
        ];
        let built = build(&docs, &opts()).unwrap();
        assert_eq!(built.fragments.len(), 3);
        assert_eq!(built.fragments[0].url, "a/");
        assert_eq!(built.fragments[1].url, "b/");
        assert_eq!(built.fragments[2].url, "b/");
        assert_eq!(built.manifest.doc_count, 3);
        assert_eq!(built.manifest.doc_lens, vec![1, 1, 1]);
    }

    #[test]
    fn シャードは_term_id_の連続範囲で分割される() {
        let docs = vec![DocumentInput {
            title: "A".to_string(),
            url: "a/".to_string(),
            group: None,
            sections: vec![section(
                "alpha beta",
                vec![("alpha", 1, vec![0]), ("beta", 1, vec![1])],
            )],
        }];
        let mut o = opts();
        o.max_terms_per_shard = 1;
        let built = build(&docs, &o).unwrap();
        assert_eq!(
            built.shards.len(),
            2,
            "term 2 個・1 term/shard で 2 シャード"
        );
        assert_eq!(built.manifest.shards.len(), 2);
        assert_eq!(built.manifest.shards[0].file, "index/0000.bin");
        assert_eq!(built.manifest.shards[1].file, "index/0001.bin");
    }

    #[test]
    fn 同義語は正規化されて_manifest_に入る() {
        let docs = vec![DocumentInput {
            title: "A".to_string(),
            url: "a/".to_string(),
            group: None,
            sections: vec![section("x", vec![("x", 1, vec![0])])],
        }];
        let mut o = opts();
        o.synonyms = vec![
            vec!["b".to_string(), "a".to_string(), "a".to_string()],
            vec!["単独".to_string()],
        ];
        let built = build(&docs, &o).unwrap();
        assert_eq!(built.manifest.synonyms, vec![vec!["a", "b"]]);
    }

    #[test]
    fn content_hash_は空文字で返す() {
        let docs = vec![DocumentInput {
            title: "A".to_string(),
            url: "a/".to_string(),
            group: None,
            sections: vec![section("x", vec![("x", 1, vec![0])])],
        }];
        let built = build(&docs, &opts()).unwrap();
        assert_eq!(built.manifest.content_hash, "");
    }

    #[test]
    fn グループは_opts_の順で採番され未宣言は末尾へ足される() {
        let docs = vec![
            doc_in(
                "A",
                "guide/a/",
                Some("ガイド"),
                vec![section("alpha", vec![("alpha", 1, vec![0])])],
            ),
            doc_in(
                "B",
                "x/b/",
                Some("未宣言"),
                vec![section("beta", vec![("beta", 1, vec![0])])],
            ),
            doc_in(
                "C",
                "ref/c/",
                Some("リファレンス"),
                vec![section("gamma", vec![("gamma", 1, vec![0])])],
            ),
        ];
        let mut opts = opts();
        opts.groups = vec!["ガイド".to_string(), "リファレンス".to_string()];
        let built = build(&docs, &opts).unwrap();

        // 宣言順が保たれ、未宣言は初出順で末尾
        assert_eq!(built.manifest.groups, ["ガイド", "リファレンス", "未宣言"]);
        assert_eq!(built.manifest.doc_groups, [0, 2, 1]);
    }

    #[test]
    fn グループ未指定の_doc_は未分類になる() {
        let docs = vec![
            doc_in(
                "A",
                "a/",
                None,
                vec![section("alpha", vec![("alpha", 1, vec![0])])],
            ),
            doc_in(
                "B",
                "b/",
                Some("ガイド"),
                vec![section("beta", vec![("beta", 1, vec![0])])],
            ),
        ];
        let built = build(&docs, &opts()).unwrap();
        assert_eq!(built.manifest.doc_groups, [crate::UNGROUPED, 0]);
        assert_eq!(built.manifest.groups, ["ガイド"]);
    }

    #[test]
    fn doc_groups_は_doc_count_と同じ長さになる() {
        // 1 ページ = 複数セクション（doc）なので、ページ単位のグループが
        // セクション数ぶん展開されることの回帰
        let docs = vec![doc_in(
            "A",
            "a/",
            Some("ガイド"),
            vec![
                section("alpha", vec![("alpha", 1, vec![0])]),
                section("beta", vec![("beta", 1, vec![0])]),
                section("gamma", vec![("gamma", 1, vec![0])]),
            ],
        )];
        let built = build(&docs, &opts()).unwrap();
        assert_eq!(
            built.manifest.doc_groups.len(),
            built.manifest.doc_count as usize
        );
        assert_eq!(built.manifest.doc_groups, [0, 0, 0]);
    }

    #[test]
    fn グループつきでも_manifest_は決定的() {
        let make = || {
            vec![
                doc_in(
                    "A",
                    "a/",
                    Some("ガイド"),
                    vec![section("alpha", vec![("alpha", 1, vec![0])])],
                ),
                doc_in(
                    "B",
                    "b/",
                    Some("開発"),
                    vec![section("beta", vec![("beta", 1, vec![0])])],
                ),
            ]
        };
        let a = build(&make(), &opts()).unwrap();
        let b = build(&make(), &opts()).unwrap();
        assert_eq!(
            serde_json::to_string(&a.manifest).unwrap(),
            serde_json::to_string(&b.manifest).unwrap()
        );
    }

    #[test]
    fn 古い_manifest_json_も読める() {
        // doc_groups / groups を持たない JSON（v0.11 以前の dist）が
        // serde(default) で通ること。`synonyms` / `content_hash` と同じ後方互換
        let built = build(
            &[doc_in(
                "A",
                "a/",
                Some("ガイド"),
                vec![section("alpha", vec![("alpha", 1, vec![0])])],
            )],
            &opts(),
        )
        .unwrap();
        let mut value: serde_json::Value = serde_json::to_value(&built.manifest).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove("docGroups");
        obj.remove("groups");
        let back: Manifest = serde_json::from_value(value).unwrap();
        assert!(back.doc_groups.is_empty());
        assert!(back.groups.is_empty());
    }
}
