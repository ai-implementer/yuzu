//! シャードバイナリ（`dist/_search/index/NNNN.bin`）の読み書き。
//!
//! レイアウト（リトルエンディアン）:
//! ```text
//! 0x00  magic  "YZSH" (4B)
//! 0x04  u16    format_version
//! 0x06  u16    reserved (= 0)
//! 0x08  u32    term_count（このシャードに含まれる term 数）
//! 0x0C  u32 × (term_count + 1)   blob 内オフセット表（term ローカル添字順）
//! ...   blob:  term ごとに varint doc_freq、続けて doc_freq 組の
//!              (varint doc_id_delta, varint tf,
//!               varint pos_count, pos_count × varint pos_delta)
//! ```
//! グローバル term_id は `manifest.shards[i].term_start + ローカル添字`。
//!
//! 位置情報（v3 で追加）: doc 内の出現位置をトークン添字の delta 列で持つ
//! （先頭は絶対値、以降は直前との差。昇順保証）。tf は見出し・タイトルの
//! 重み付きで出現数と一致しないため、位置の件数は pos_count で明示する。
//! フォーマットとしては pos_count = 0 も受理する（インデクサは実出現から
//! 作るため常に 1 以上を書く）。BM25 だけの読み手（[`Shard::postings`]）は
//! 位置ブロックを読み飛ばす

use crate::FORMAT_VERSION;
use crate::error::FormatError;
use crate::varint;

const MAGIC: &[u8; 4] = b"YZSH";
const HEADER_LEN: usize = 12;
/// 1 つの posting が blob で占める最小のバイト数（doc_id_delta・tf・pos_count の varint が 1 バイトずつ）
const MIN_POSTING_BYTES: usize = 3;

/// オフセット表の終わり（= blob の始まり）のバイト位置。usize の幅に依らないよう u64 で計算する
/// （32 bit の usize（wasm32）では `(term_count + 1) * 4` が桁あふれし、長さの検査を通った後に
/// 範囲外の添字で panic していた。Phase 92）
fn blob_start(term_count: u32) -> u64 {
    HEADER_LEN as u64 + (u64::from(term_count) + 1) * 4
}

/// 入力が申告する件数で確保する量を、残りのバイト数で読める件数までに抑える
/// （件数は 1 件あたり `min_bytes` 以上のバイトを使うので、それ以上は読めない。
/// 申告どおりに確保すると `u32::MAX` 件で数十 GB を確保しようとする）
fn bounded_capacity(count: u32, remaining: usize, min_bytes: usize) -> usize {
    (count as usize).min(remaining / min_bytes)
}

/// delta 列を足し合わせる（壊れたデータで u32 を超えたらエラー）
fn add_delta(base: u32, delta: u32) -> Result<u32, FormatError> {
    base.checked_add(delta).ok_or(FormatError::PostingOverflow)
}

/// 1 doc ぶんの posting（v3: 出現位置付き）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posting {
    pub doc_id: u32,
    /// 重み付き出現数（見出し・タイトルは加重されるため positions.len() と一致しない）
    pub tf: u32,
    /// doc 内の出現位置（トークン添字・昇順）
    pub positions: Vec<u32>,
}

/// シャードを直列化する。`postings[ローカル添字]` = doc_id 昇順の [`Posting`] 列
pub fn encode_shard(postings: &[Vec<Posting>]) -> Vec<u8> {
    let mut blob = Vec::new();
    let mut offsets = Vec::with_capacity(postings.len() + 1);
    for term_postings in postings {
        offsets.push(blob.len() as u32);
        varint::write_u32(&mut blob, term_postings.len() as u32);
        let mut prev_doc = 0u32;
        for posting in term_postings {
            varint::write_u32(&mut blob, posting.doc_id - prev_doc);
            varint::write_u32(&mut blob, posting.tf);
            varint::write_u32(&mut blob, posting.positions.len() as u32);
            let mut prev_pos = 0u32;
            for &p in &posting.positions {
                varint::write_u32(&mut blob, p - prev_pos);
                prev_pos = p;
            }
            prev_doc = posting.doc_id;
        }
    }
    offsets.push(blob.len() as u32);

    let mut out = Vec::with_capacity(HEADER_LEN + offsets.len() * 4 + blob.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(postings.len() as u32).to_le_bytes());
    for offset in &offsets {
        out.extend_from_slice(&offset.to_le_bytes());
    }
    out.extend_from_slice(&blob);
    out
}

/// パース済みシャード
pub struct Shard {
    term_count: u32,
    /// term_count + 1 個の blob 内オフセット
    offsets: Vec<u32>,
    blob: Vec<u8>,
}

impl Shard {
    pub fn parse(bytes: &[u8]) -> Result<Self, FormatError> {
        if bytes.len() < HEADER_LEN || &bytes[0..4] != MAGIC {
            return Err(FormatError::BadMagic);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != FORMAT_VERSION {
            return Err(FormatError::VersionMismatch {
                expected: FORMAT_VERSION,
                actual: version,
            });
        }
        let term_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        let blob_start = usize::try_from(blob_start(term_count))
            .ok()
            .filter(|&start| start <= bytes.len())
            .ok_or(FormatError::UnexpectedEof)?;
        let offsets: Vec<u32> = bytes[HEADER_LEN..blob_start]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        let blob = bytes[blob_start..].to_vec();
        if offsets.last().copied().unwrap_or(0) as usize > blob.len() {
            return Err(FormatError::UnexpectedEof);
        }

        Ok(Self {
            term_count,
            offsets,
            blob,
        })
    }

    pub fn term_count(&self) -> u32 {
        self.term_count
    }

    /// ローカル添字の term に対応する blob スライスを取り出す（境界チェック込み）
    fn term_slice(&self, local_index: u32) -> Result<&[u8], FormatError> {
        if local_index >= self.term_count {
            return Err(FormatError::TermOutOfRange(local_index));
        }
        let start = self.offsets[local_index as usize] as usize;
        let end = self.offsets[local_index as usize + 1] as usize;
        self.blob.get(start..end).ok_or(FormatError::UnexpectedEof)
    }

    /// ローカル添字の term の postings（`(doc_id, tf)` 列）をデコードする。
    /// 位置ブロックは読み飛ばす（BM25 のホットパス）
    pub fn postings(&self, local_index: u32) -> Result<Vec<(u32, u32)>, FormatError> {
        let slice = self.term_slice(local_index)?;
        let mut pos = 0;
        let doc_freq = varint::read_u32(slice, &mut pos)?;
        let mut postings = Vec::with_capacity(bounded_capacity(
            doc_freq,
            slice.len() - pos,
            MIN_POSTING_BYTES,
        ));
        let mut doc_id = 0u32;
        for _ in 0..doc_freq {
            doc_id = add_delta(doc_id, varint::read_u32(slice, &mut pos)?)?;
            let tf = varint::read_u32(slice, &mut pos)?;
            let pos_count = varint::read_u32(slice, &mut pos)?;
            for _ in 0..pos_count {
                varint::read_u32(slice, &mut pos)?; // 読み捨て（EOF ガードが破損検出を兼ねる）
            }
            postings.push((doc_id, tf));
        }
        Ok(postings)
    }

    /// ローカル添字の term の postings を出現位置込みでデコードする
    /// （フレーズ照合用。BM25 だけなら [`Shard::postings`] を使う）
    pub fn postings_with_positions(&self, local_index: u32) -> Result<Vec<Posting>, FormatError> {
        let slice = self.term_slice(local_index)?;
        let mut pos = 0;
        let doc_freq = varint::read_u32(slice, &mut pos)?;
        let mut postings = Vec::with_capacity(bounded_capacity(
            doc_freq,
            slice.len() - pos,
            MIN_POSTING_BYTES,
        ));
        let mut doc_id = 0u32;
        for _ in 0..doc_freq {
            doc_id = add_delta(doc_id, varint::read_u32(slice, &mut pos)?)?;
            let tf = varint::read_u32(slice, &mut pos)?;
            let pos_count = varint::read_u32(slice, &mut pos)?;
            // 位置の delta は 1 件 1 バイト以上
            let mut positions =
                Vec::with_capacity(bounded_capacity(pos_count, slice.len() - pos, 1));
            let mut token_pos = 0u32;
            for _ in 0..pos_count {
                token_pos = add_delta(token_pos, varint::read_u32(slice, &mut pos)?)?;
                positions.push(token_pos);
            }
            postings.push(Posting {
                doc_id,
                tf,
                positions,
            });
        }
        Ok(postings)
    }
}

#[cfg(test)]
mod tests {
    use super::{MIN_POSTING_BYTES, Posting, Shard, blob_start, bounded_capacity, encode_shard};
    use crate::FORMAT_VERSION;
    use crate::error::FormatError;
    use crate::varint;

    /// term 1 つのシャードを、blob の生バイトから組み立てる（エンコーダが書かない壊れた値を作る）
    fn raw_shard(blob: &[u8]) -> Vec<u8> {
        let mut out = b"YZSH".to_vec();
        out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        out.extend_from_slice(blob);
        out
    }

    fn varints(values: &[u32]) -> Vec<u8> {
        let mut out = Vec::new();
        for &v in values {
            varint::write_u32(&mut out, v);
        }
        out
    }

    /// 32 bit の usize で桁あふれしていた計算を、usize の幅に依らない値で確かめる
    /// （64 bit では修正前も parse がエラーを返すので、parse だけのテストでは差が出ない）
    #[test]
    fn blob_の開始位置は_usize_の幅に依らず計算する() {
        assert_eq!(blob_start(0), 16);
        assert_eq!(blob_start(3), 12 + 4 * 4);
        assert_eq!(blob_start(u32::MAX), 12 + 4 * (1u64 << 32));
        let mut header = b"YZSH".to_vec();
        header.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&0u16.to_le_bytes());
        header.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Shard::parse(&header),
            Err(FormatError::UnexpectedEof)
        ));
    }

    #[test]
    fn 確保する量は残りのバイト数で読める件数まで() {
        assert_eq!(bounded_capacity(u32::MAX, 30, MIN_POSTING_BYTES), 10);
        assert_eq!(bounded_capacity(2, 30, MIN_POSTING_BYTES), 2);
        assert_eq!(bounded_capacity(u32::MAX, 0, 1), 0);
    }

    /// 申告された件数（u32::MAX）の posting でも、データの終わりでエラーを返す。
    /// 修正前は申告どおりに数十 GB を確保しようとした。64 bit の Linux では overcommit で確保が
    /// 通るのでこのテストでは差が出ない（wasm32 では容量の桁あふれで panic し、fuzz では malloc の
    /// 上限で止まる）。確保量の上限は [`bounded_capacity`] のテストで確かめる
    #[test]
    fn 件数が大きすぎる_posting_はエラー() {
        let shard = Shard::parse(&raw_shard(&varints(&[u32::MAX]))).unwrap();
        assert!(matches!(shard.postings(0), Err(FormatError::UnexpectedEof)));
        assert!(matches!(
            shard.postings_with_positions(0),
            Err(FormatError::UnexpectedEof)
        ));
        // 位置の件数（doc_freq 1・doc_id 0・tf 1・pos_count u32::MAX）
        let shard = Shard::parse(&raw_shard(&varints(&[1, 0, 1, u32::MAX]))).unwrap();
        assert!(matches!(
            shard.postings_with_positions(0),
            Err(FormatError::UnexpectedEof)
        ));
    }

    /// delta の合計が u32 を超えたらエラー（修正前は debug で panic・release で値が巻き戻っていた）
    #[test]
    fn doc_id_と出現位置の桁あふれはエラー() {
        // doc_freq 2: (delta u32::MAX, tf 1, pos 0) → (delta 1, …) で doc_id が u32 を超える
        let shard = Shard::parse(&raw_shard(&varints(&[2, u32::MAX, 1, 0, 1, 1, 0]))).unwrap();
        assert!(matches!(
            shard.postings(0),
            Err(FormatError::PostingOverflow)
        ));
        assert!(matches!(
            shard.postings_with_positions(0),
            Err(FormatError::PostingOverflow)
        ));
        // 位置の delta: u32::MAX の次に 1
        let shard = Shard::parse(&raw_shard(&varints(&[1, 0, 1, 2, u32::MAX, 1]))).unwrap();
        assert!(matches!(
            shard.postings_with_positions(0),
            Err(FormatError::PostingOverflow)
        ));
        // 合計がちょうど u32::MAX なら読める
        let shard = Shard::parse(&raw_shard(&varints(&[1, u32::MAX, 1, 1, u32::MAX]))).unwrap();
        assert_eq!(shard.postings(0).unwrap(), vec![(u32::MAX, 1)]);
        assert_eq!(
            shard.postings_with_positions(0).unwrap()[0].positions,
            vec![u32::MAX]
        );
    }

    /// テスト用の Posting 生成ヘルパ
    fn p(doc_id: u32, tf: u32, positions: &[u32]) -> Posting {
        Posting {
            doc_id,
            tf,
            positions: positions.to_vec(),
        }
    }

    /// fuzz の種（`fuzz/seeds/shard/<名前>.bin`）にするシャード
    fn fuzz_seeds() -> Vec<(&'static str, Vec<Vec<Posting>>)> {
        vec![
            ("empty", vec![]),
            ("one-term", vec![vec![p(0, 1, &[0])]]),
            (
                "several-terms",
                vec![
                    vec![p(0, 3, &[0, 4, 210]), p(2, 1, &[7]), p(10, 7, &[1, 2, 3])],
                    vec![],
                    vec![p(5, 1, &[102])],
                    vec![p(0, 1, &[0]), p(1, 1, &[9]), p(2, 1, &[5]), p(3, 1, &[1])],
                ],
            ),
            ("no-positions", vec![vec![p(3, 2, &[])]]),
            (
                "large-values",
                vec![vec![
                    p(127, 128, &[16_383, 16_384]),
                    p(u32::MAX - 1, 1, &[u32::MAX - 1]),
                ]],
            ),
        ]
    }

    /// fuzz の種が今のエンコーダの出力と同じであること（Phase 92）。種は magic とバージョンを
    /// 通らないと postings の読み込みまで届かないので、`FORMAT_VERSION` を上げたら
    /// `MIKAN_UPDATE_SEEDS=1 cargo test -p mikan fuzz_の種` で書き直す
    #[test]
    fn fuzz_の種は今のエンコーダの出力と同じ() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/seeds/shard");
        if std::env::var_os("MIKAN_UPDATE_SEEDS").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            for (name, postings) in fuzz_seeds() {
                std::fs::write(dir.join(format!("{name}.bin")), encode_shard(&postings)).unwrap();
            }
        }
        // crates.io の配布物には fuzz/ が入らない（入れ子のパッケージは cargo package が外す）
        if !dir.is_dir() {
            return;
        }
        for (name, postings) in fuzz_seeds() {
            let path = dir.join(format!("{name}.bin"));
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                bytes,
                encode_shard(&postings),
                "{name}.bin が古い（MIKAN_UPDATE_SEEDS=1 cargo test -p mikan fuzz_の種 で書き直す）"
            );
            let shard = Shard::parse(&bytes).unwrap();
            for i in 0..shard.term_count() {
                shard.postings_with_positions(i).unwrap();
            }
        }
    }

    #[test]
    fn 読み書きの_roundtrip() {
        let postings = vec![
            vec![p(0, 3, &[0, 4, 210]), p(2, 1, &[7]), p(10, 7, &[1, 2, 3])],
            vec![],
            vec![p(5, 1, &[102])],
            vec![p(0, 1, &[0]), p(1, 1, &[9]), p(2, 1, &[5]), p(3, 1, &[1])],
        ];
        let bytes = encode_shard(&postings);
        let shard = Shard::parse(&bytes).unwrap();
        assert_eq!(shard.term_count(), 4);
        for (i, expected) in postings.iter().enumerate() {
            // BM25 パス: 位置を読み飛ばして (doc_id, tf) を返す
            let flat: Vec<(u32, u32)> = expected.iter().map(|p| (p.doc_id, p.tf)).collect();
            assert_eq!(
                shard.postings(i as u32).unwrap(),
                flat,
                "term {i} の平坦読み"
            );
            // フレーズパス: 位置込みで完全一致
            assert_eq!(
                &shard.postings_with_positions(i as u32).unwrap(),
                expected,
                "term {i} の位置込み読み"
            );
        }
        assert!(shard.postings(4).is_err());
        assert!(shard.postings_with_positions(4).is_err());
    }

    #[test]
    fn 位置なし_posting_も扱える() {
        // フォーマットとしては pos_count = 0 を受理する（インデクサは書かない）
        let postings = vec![vec![p(3, 2, &[])]];
        let bytes = encode_shard(&postings);
        let shard = Shard::parse(&bytes).unwrap();
        assert_eq!(shard.postings(0).unwrap(), vec![(3, 2)]);
        assert_eq!(shard.postings_with_positions(0).unwrap(), postings[0]);
    }

    #[test]
    fn magic_不一致はエラー() {
        let mut bytes = encode_shard(&[vec![p(0, 1, &[0])]]);
        bytes[0] = b'X';
        assert!(Shard::parse(&bytes).is_err());
    }

    #[test]
    fn バージョン不一致はエラー() {
        let mut bytes = encode_shard(&[vec![p(0, 1, &[0])]]);
        bytes[4] = 0xff;
        assert!(Shard::parse(&bytes).is_err());
    }

    #[test]
    fn 途中で切れたデータはエラー() {
        let bytes = encode_shard(&[vec![p(0, 1, &[0]), p(5, 2, &[1, 8])]]);
        assert!(Shard::parse(&bytes[..bytes.len() - 3]).is_err());
    }

    #[test]
    fn 位置ブロックの途中で切れたデータは両_api_ともエラー() {
        // オフセット表は末尾を指したまま blob の位置バイトだけ欠けた状態を作る:
        // parse は通るが、デコードが位置ブロックの EOF で失敗する
        let postings = vec![vec![p(0, 1, &[0, 3, 9])]];
        let full = encode_shard(&postings);
        let mut cut = full.clone();
        let removed = 2; // 末尾の pos_delta 2 個ぶん（各 1B）を削る
        cut.truncate(full.len() - removed);
        // オフセット表の末尾（blob 長）を切り詰め後の長さに合わせて偽装する
        let blob_len_offset = 12 + 4; // header + offsets[0]
        let new_blob_len = (cut.len() - (12 + 8)) as u32; // offsets は 2 エントリ
        cut[blob_len_offset..blob_len_offset + 4].copy_from_slice(&new_blob_len.to_le_bytes());
        let shard = Shard::parse(&cut).unwrap();
        assert!(shard.postings(0).is_err(), "スキップ読みも破損を検出する");
        assert!(shard.postings_with_positions(0).is_err());
    }
}
