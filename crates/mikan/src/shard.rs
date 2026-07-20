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

        let offsets_len = (term_count as usize + 1) * 4;
        let blob_start = HEADER_LEN + offsets_len;
        if bytes.len() < blob_start {
            return Err(FormatError::UnexpectedEof);
        }
        let offsets: Vec<u32> = bytes[HEADER_LEN..blob_start]
            .chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
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
        let mut postings = Vec::with_capacity(doc_freq as usize);
        let mut doc_id = 0u32;
        for _ in 0..doc_freq {
            doc_id += varint::read_u32(slice, &mut pos)?;
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
        let mut postings = Vec::with_capacity(doc_freq as usize);
        let mut doc_id = 0u32;
        for _ in 0..doc_freq {
            doc_id += varint::read_u32(slice, &mut pos)?;
            let tf = varint::read_u32(slice, &mut pos)?;
            let pos_count = varint::read_u32(slice, &mut pos)?;
            let mut positions = Vec::with_capacity(pos_count as usize);
            let mut token_pos = 0u32;
            for _ in 0..pos_count {
                token_pos += varint::read_u32(slice, &mut pos)?;
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
    use super::{Posting, Shard, encode_shard};

    /// テスト用の Posting 生成ヘルパ
    fn p(doc_id: u32, tf: u32, positions: &[u32]) -> Posting {
        Posting {
            doc_id,
            tf,
            positions: positions.to_vec(),
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
