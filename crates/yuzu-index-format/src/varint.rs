//! LEB128 varint と delta エンコード（postings 用）

use crate::error::FormatError;

/// u32 を LEB128 で書き込む
pub fn write_u32(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// LEB128 の u32 を読み出す（`pos` を進める）
pub fn read_u32(buf: &[u8], pos: &mut usize) -> Result<u32, FormatError> {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = *buf.get(*pos).ok_or(FormatError::UnexpectedEof)?;
        *pos += 1;
        // 5 バイト目（shift=28）は下位 4 bit しか使えない
        if shift == 28 && (byte & 0x70) != 0 {
            return Err(FormatError::VarintOverflow);
        }
        result |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift > 28 {
            return Err(FormatError::VarintOverflow);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{read_u32, write_u32};

    #[test]
    fn 境界値の_roundtrip() {
        for v in [0u32, 1, 127, 128, 255, 16383, 16384, u32::MAX - 1, u32::MAX] {
            let mut buf = Vec::new();
            write_u32(&mut buf, v);
            let mut pos = 0;
            assert_eq!(read_u32(&buf, &mut pos).unwrap(), v, "v={v}");
            assert_eq!(pos, buf.len());
        }
    }

    #[test]
    fn 連続書き込みの_roundtrip() {
        let values: Vec<u32> = (0u32..1000).map(|i| i.wrapping_mul(2654435761)).collect();
        let mut buf = Vec::new();
        for &v in &values {
            write_u32(&mut buf, v);
        }
        let mut pos = 0;
        for &v in &values {
            assert_eq!(read_u32(&buf, &mut pos).unwrap(), v);
        }
        assert_eq!(pos, buf.len());
    }

    #[test]
    fn 途中で切れたデータはエラー() {
        let mut buf = Vec::new();
        write_u32(&mut buf, u32::MAX);
        buf.pop();
        let mut pos = 0;
        assert!(read_u32(&buf, &mut pos).is_err());
    }

    #[test]
    fn 過長な_varint_はエラー() {
        // 6 バイト継続は u32 に収まらない
        let buf = [0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
        let mut pos = 0;
        assert!(read_u32(&buf, &mut pos).is_err());
    }
}
