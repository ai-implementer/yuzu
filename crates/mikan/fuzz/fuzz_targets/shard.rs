//! 検索の索引のシャードの読み込みが、任意の入力で panic・ハング・巨大な確保をしないこと。
//! parse だけでは届かない postings の読み込み（位置情報付きを含む）まで、全 term について読む
//! （Phase 92。種は seeds/shard/ のエンコーダが書いたシャード）

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(shard) = mikan::Shard::parse(data) {
        // term_count はオフセット表の長さを入力が満たしたときだけ通るので、入力の大きさで抑えられる
        for i in 0..shard.term_count() {
            let _ = shard.postings(i);
            let _ = shard.postings_with_positions(i);
        }
    }
});
