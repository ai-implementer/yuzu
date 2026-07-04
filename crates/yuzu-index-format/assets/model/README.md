# vendor 物の記録: vaporetto 学習済みモデル

## bccwj-suw_c1.0.model.zst

- 取得元: <https://github.com/daac-tools/vaporetto-models/releases/tag/v0.5.0>
  （`bccwj-suw_c1.0.tar.xz` に同梱の `.model.zst`）
- ライセンス: **MIT OR Apache-2.0**（アーカイブ同梱の LICENSE-MIT / LICENSE-APACHE を確認済み。
  BCCWJ 由来だが NINJAL との共同研究成果としてこのライセンスで配布されている）
- 取得日: 2026-07-04
- sha256: `95efbc00d833d0f979044fb74d5083380d7c93163763a166bc0e7aeb58abe143`
- 圧縮サイズ: 372KB（zstd。**伸長せずこのまま** dist の `_search/model.zst` として配信し、
  ネイティブ／wasm の両側で同一バイトから読み込む＝トークナイザ整合の保証）
- 更新手順: リポジトリルートで `scripts/vendor-vaporetto-model.sh` を実行し、本ファイルを更新する

> 辞書なし SUW（短単位）モデル。UniDic 入りモデル（約 6MB、BSD-3-Clause）より
> 精度は下がるが、クライアント配布サイズを優先して採用（設計ノートの方針どおり）。
> `yuzu.jsonc` の `search.dictionary` に `.model.zst` のパスを指定すれば差し替え可能。
