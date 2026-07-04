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
//!              (varint doc_id_delta, varint tf)
//! ```
//! グローバル term_id は `manifest.shards[i].term_start + ローカル添字`。
//! 位置情報は v1 では持たない（BM25 に不要。フレーズ検索は将来のバージョンで）

use crate::FORMAT_VERSION;
use crate::error::FormatError;
use crate::varint;

const MAGIC: &[u8; 4] = b"YZSH";
const HEADER_LEN: usize = 12;

/// シャードを直列化する。`postings[ローカル添字]` = 昇順の `(doc_id, tf)` 列
pub fn encode_shard(postings: &[Vec<(u32, u32)>]) -> Vec<u8> {
    let mut blob = Vec::new();
    let mut offsets = Vec::with_capacity(postings.len() + 1);
    for term_postings in postings {
        offsets.push(blob.len() as u32);
        varint::write_u32(&mut blob, term_postings.len() as u32);
        let mut prev_doc = 0u32;
        for &(doc_id, tf) in term_postings {
            varint::write_u32(&mut blob, doc_id - prev_doc);
            varint::write_u32(&mut blob, tf);
            prev_doc = doc_id;
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

    /// ローカル添字の term の postings（`(doc_id, tf)` 列）をデコードする
    pub fn postings(&self, local_index: u32) -> Result<Vec<(u32, u32)>, FormatError> {
        if local_index >= self.term_count {
            return Err(FormatError::TermOutOfRange(local_index));
        }
        let start = self.offsets[local_index as usize] as usize;
        let end = self.offsets[local_index as usize + 1] as usize;
        let slice = &self.blob[start..end];

        let mut pos = 0;
        let doc_freq = varint::read_u32(slice, &mut pos)?;
        let mut postings = Vec::with_capacity(doc_freq as usize);
        let mut doc_id = 0u32;
        for _ in 0..doc_freq {
            doc_id += varint::read_u32(slice, &mut pos)?;
            let tf = varint::read_u32(slice, &mut pos)?;
            postings.push((doc_id, tf));
        }
        Ok(postings)
    }
}

#[cfg(test)]
mod tests {
    use super::{Shard, encode_shard};

    #[test]
    fn 読み書きの_roundtrip() {
        let postings = vec![
            vec![(0, 3), (2, 1), (10, 7)],
            vec![],
            vec![(5, 1)],
            vec![(0, 1), (1, 1), (2, 1), (3, 1)],
        ];
        let bytes = encode_shard(&postings);
        let shard = Shard::parse(&bytes).unwrap();
        assert_eq!(shard.term_count(), 4);
        for (i, expected) in postings.iter().enumerate() {
            assert_eq!(&shard.postings(i as u32).unwrap(), expected, "term {i}");
        }
        assert!(shard.postings(4).is_err());
    }

    #[test]
    fn magic_不一致はエラー() {
        let mut bytes = encode_shard(&[vec![(0, 1)]]);
        bytes[0] = b'X';
        assert!(Shard::parse(&bytes).is_err());
    }

    #[test]
    fn バージョン不一致はエラー() {
        let mut bytes = encode_shard(&[vec![(0, 1)]]);
        bytes[4] = 0xff;
        assert!(Shard::parse(&bytes).is_err());
    }

    #[test]
    fn 途中で切れたデータはエラー() {
        let bytes = encode_shard(&[vec![(0, 1), (5, 2)]]);
        assert!(Shard::parse(&bytes[..bytes.len() - 3]).is_err());
    }
}
