use std::collections::{BTreeSet, HashMap, HashSet};

use crate::cache::SyncCache;
use crate::debug::child_extractor::DataWithRoot;
use crate::debug::{DebugPrintExt, EntriesHex, InnerEntriesHex};
use crate::gc::tests::{
    FixedKey, MixedNonUniqueValue, NodesGenerator, UniqueValue, VariableKey, RNG_DATA_SIZE,
};
use crate::mutable::TrieMut;
use crate::ops::diff::verify::VerificationError;

use crate::{debug, diff, empty_trie_hash, verify_diff, Database, DiffChange as Change};
use hex_literal::hex;
use primitive_types::H256;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Keccak256};

use crate::gc::{DbCounter, MapWithCounterCachedParam, ReachableHashes, RootGuard, TrieCollection};
use crate::ops::debug::tests::*;

use super::VerifiedPatch;

fn check_changes(
    changes: VerifiedPatch,
    initial_trie_data: &debug::EntriesHex,
    expected_trie_root: H256,
    expected_trie_data: debug::EntriesHex,
) {
    let collection = TrieCollection::new(MapWithCounterCachedParam::<SyncCache>::default());
    let mut trie = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &initial_trie_data.data {
        trie.insert(key, value.as_ref().unwrap());
    }

    let patch = trie.into_patch();
    let _initial_root = collection.apply_increase(patch, crate::debug::no_childs);

    let apply_result = collection.apply_diff_patch(changes, no_childs);
    assert!(apply_result.is_ok());
    let expected_root_root_guard = apply_result.unwrap();
    assert_eq!(expected_root_root_guard.root, expected_trie_root);

    let new_trie = collection.trie_for(expected_trie_root);

    for (key, value) in expected_trie_data.data {
        assert_eq!(TrieMut::get(&new_trie, &key), value);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
struct VerifiedPatchHexStr {
    patch_dependencies: Option<BTreeSet<H256>>,
    sorted_changes: Vec<(H256, String)>,
}

impl From<VerifiedPatch> for VerifiedPatchHexStr {
    fn from(value: VerifiedPatch) -> Self {
        let mut result = Self {
            patch_dependencies: value.patch_dependencies,
            sorted_changes: Vec::new(),
        };
        for (hash, _is_direct, data) in value.sorted_changes.into_iter() {
            let ser = hexutil::to_hex(&data);
            result.sorted_changes.push((hash, ser));
        }

        log::info!("{}", serde_json::to_string_pretty(&result).unwrap());
        result
    }
}

fn no_childs(_: &[u8]) -> Vec<H256> {
    vec![]
}

type SyncDashMap = MapWithCounterCachedParam<SyncCache>;

#[test]
fn test_two_different_leaf_nodes() {
    tracing_sub_init();

    let j = json!([[
        "0xaaab",
        "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
    ]]);
    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();

    // make data too long for inline
    let j = json!([[
        "0xaaac",
        "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
    ]]);
    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();

    let verify_result = verify_diff(
        &collection.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(verify_result.is_ok());

    let j = json!([
        ["0xaaab", null],
        [
            "0xaaac",
            "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ]
    ]);
    let expected_trie_data: debug::EntriesHex = serde_json::from_value(j).unwrap();
    check_changes(
        verify_result.unwrap(),
        &entries1,
        second_root.root,
        expected_trie_data,
    );

    drop(second_root);
    log::info!("second trie dropped")
}

#[test]
fn test_1() {
    tracing_sub_init();

    let j = json!([
        [
            "0x0000000000000c19",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x00000000000010f6",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f33"
        ]
    ]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let j = json!([
        [
            "0x0000000000000d34",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f34"
        ],
        [
            "0x0000000000000f37",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f35"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f36"
        ]
    ]);
    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();
    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();
    let verify_result = verify_diff(
        &collection.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(verify_result.is_ok());

    let j = json!([
        ["0x0000000000000c19", null],
        ["0x00000000000010f6", null],
        [
            "0x0000000000000d34",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f34"
        ],
        [
            "0x0000000000000f37",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f35"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f36"
        ]
    ]);

    let expected_trie_data: debug::EntriesHex = serde_json::from_value(j).unwrap();
    check_changes(
        verify_result.unwrap(),
        &entries1,
        second_root.root,
        expected_trie_data,
    );

    drop(second_root);
    log::info!("second trie dropped")
}

#[test]
fn test_2() {
    tracing_sub_init();

    let j = json!([
        [
            "0x0000000000000c19",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x00000000000010f6",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f33"
        ]
    ]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();
    let j = json!([
        [
            "0x0000000000000d34",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f34"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f35"
        ]
    ]);
    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();
    let verify_result = verify_diff(
        &collection.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(verify_result.is_ok());

    let j = json!([
        ["0x0000000000000c19", null],
        ["0x00000000000010f6", null],
        [
            "0x0000000000000d34",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f34"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f35"
        ]
    ]);
    let expected_trie_data: debug::EntriesHex = serde_json::from_value(j).unwrap();
    check_changes(
        verify_result.unwrap(),
        &entries1,
        second_root.root,
        expected_trie_data,
    );
    drop(second_root);
    log::info!("second trie dropped")
}

#[test]
fn test_3() {
    tracing_sub_init();

    let j = json!([
        [
            "0x0000000000000c19",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x00000000000010f6",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f33"
        ]
    ]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();

    // One entry removed which eliminates first branch node
    let j = json!([
        [
            "0x0000000000000c19",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ]
    ]);
    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();

    let verify_result = verify_diff(
        &collection.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(verify_result.is_ok());

    let diff_patch: VerifiedPatch = verify_result.unwrap();
    let diff_patch_ser: VerifiedPatchHexStr = diff_patch.clone().into();

    let j = json!({
        "patch_dependencies": [
            "0xcfb83f6df401062bbc6ec0e083bfdb1331c83162cb863272bea7c5d78805e25e"
        ],
        "sorted_changes": [
            [
                format!("{:?}", second_root.root),
                "0xe98710000000000000a0cfb83f6df401062bbc6ec0e083bfdb1331c83162cb863272bea7c5d78805e25e"
            ]
        ]
    });
    let exp_patch: VerifiedPatchHexStr = serde_json::from_value(j).unwrap();

    assert_eq!(diff_patch_ser, exp_patch);
    let j = json!([
        ["0x00000000000010f6", null],
        [
            "0x0000000000000c19",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x0000000000000fcb",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ]
    ]);

    let expected_trie_data: debug::EntriesHex = serde_json::from_value(j).unwrap();
    check_changes(diff_patch, &entries1, second_root.root, expected_trie_data);
    drop(second_root);
    log::info!("second trie dropped")
}

#[test]
fn test_4() {
    tracing_sub_init();

    let j = json!([
        [
            "0xb0033333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x33333000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x03333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333b33",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f33"
        ]
    ]);

    // One entry removed which eliminates first branch node

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();
    let j = json!([
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333b30",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0xf3333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x3333333b",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ]
    ]);

    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();
    let verify_result = verify_diff(
        &collection.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(verify_result.is_ok());

    let j = json!([
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x33333b30",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0xf3333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x3333333b",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        ["0xb0033333", null]
    ]);

    let expected_trie_data: debug::EntriesHex = serde_json::from_value(j).unwrap();
    check_changes(
        verify_result.unwrap(),
        &entries1,
        second_root.root,
        expected_trie_data,
    );
    drop(second_root);
    log::info!("second trie dropped")
}

#[test]
fn test_5() {
    tracing_sub_init();

    let j = json!([
        [
            "0xb0033333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x33333000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x03333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333b33",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f33"
        ]
    ]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();

    // One entry removed which eliminates first branch node
    let j = json!([
        [
            "0xb0033333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333b30",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0xf3333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x3333333b",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ]
    ]);
    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();

    let result = verify_diff(
        &collection.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(result.is_ok());
    let diff_patch: VerifiedPatchHexStr = result.unwrap().into();

    let j = json!({
        "patch_dependencies": [
            "0xc905cc1f8c7992a69e8deabc075e6daf72029efa76b86be5e71903c0848001fb"
        ],
        "sorted_changes": [
            [
                format!("{:?}", second_root.root),
                "0xf871808080a09917c55a4ff0aea28a59174e0bf71ded54e14d0cfb345b7c4ebd50801363426980808080808080a0c905cc1f8c7992a69e8deabc075e6daf72029efa76b86be5e71903c0848001fb808080a04cf15526cbfe7ed0093e6e28346d9ef3977541a6b56fdea74a914df6b451e3d780"
            ],
            [
                "0x9917c55a4ff0aea28a59174e0bf71ded54e14d0cfb345b7c4ebd508013634269",
                "0xe583003333a0485d6a6f685291273df84688f4f884b68568f6c35f79037da41f020f6434e2db"
            ],
            [
                "0x4cf15526cbfe7ed0093e6e28346d9ef3977541a6b56fdea74a914df6b451e3d7",
                "0xe78433333333a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
            ],
            [
                "0x485d6a6f685291273df84688f4f884b68568f6c35f79037da41f020f6434e2db",
                "0xf851808080a0a1a9208173e3a50541f6961ca0eaede00a862a7d2bdad367c6122b1d9ec2117280808080808080a05bc0a795dae749afa7c4354e0b5cbb33e16e52c0c1da42bf02b5349ec554e8938080808080"
            ],
            [
                "0xa1a9208173e3a50541f6961ca0eaede00a862a7d2bdad367c6122b1d9ec21172",
                "0xe213a097eb90da8920ff6d6740f0bb8a89719d789a1fe6a871861eca5caba98d6f847b"
            ],
            [
                "0x5bc0a795dae749afa7c4354e0b5cbb33e16e52c0c1da42bf02b5349ec554e893",
                "0xe5822030a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
            ],
            [
                "0x97eb90da8920ff6d6740f0bb8a89719d789a1fe6a871861eca5caba98d6f847b",
                "0xf851808080a0999403f7e9f45fb8ccdc81134c04590524ece5ca8edcd9f884cb27208c6825de80808080808080a0999403f7e9f45fb8ccdc81134c04590524ece5ca8edcd9f884cb27208c6825de8080808080"
            ],
            [
                "0x999403f7e9f45fb8ccdc81134c04590524ece5ca8edcd9f884cb27208c6825de",
                "0xe320a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
            ],
            [
                "0x999403f7e9f45fb8ccdc81134c04590524ece5ca8edcd9f884cb27208c6825de",
                "0xe320a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
            ]
        ]
    });

    let exp_patch: VerifiedPatchHexStr = serde_json::from_value(j).unwrap();

    assert_eq!(diff_patch, exp_patch);

    drop(second_root);
    log::info!("second trie dropped")
}

#[test]
fn test_get_changeset_trivial_tree() {
    tracing_sub_init();

    let j = json!([
        [
            "0x70000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0xb0000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x00000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x00000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ]
    ]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();
    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());

    debug::draw(
        &collection.database,
        debug::Child::Hash(empty_trie_hash()),
        vec![],
        no_childs,
    )
    .print();

    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        crate::empty_trie_hash(),
        first_root.root,
    )
    .unwrap();

    let result = verify_diff(
        &collection.database,
        first_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(result.is_ok());

    let diff_patch = result.unwrap();
    let diff_patch_ser: VerifiedPatchHexStr = diff_patch.clone().into();

    let j = json!({
        "patch_dependencies": [],
        "sorted_changes": [
            [
                format!("{:?}", first_root.root),
                "0xf871a0bbe6b76206a9cad7ef4b2c8a36b4c8e360c23363cd1db491126f9b245e2214e1808080808080a0bbe6b76206a9cad7ef4b2c8a36b4c8e360c23363cd1db491126f9b245e2214e1808080a0eda927899744a922998038fa648ddadb89500cee5938b4b533067c115e84fb3f8080808080"
            ],
            [
                "0xbbe6b76206a9cad7ef4b2c8a36b4c8e360c23363cd1db491126f9b245e2214e1",
                "0xe78430000000a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
            ],
            [
                "0xbbe6b76206a9cad7ef4b2c8a36b4c8e360c23363cd1db491126f9b245e2214e1",
                "0xe78430000000a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
            ],
            [
                "0xeda927899744a922998038fa648ddadb89500cee5938b4b533067c115e84fb3f",
                "0xe78430000000a15f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
            ]
        ]
    });

    let exp_patch: VerifiedPatchHexStr = serde_json::from_value(j).unwrap();

    assert_eq!(diff_patch_ser, exp_patch);

    for (hash, _is_direct, value) in diff_patch.sorted_changes {
        let actual_hash = H256(Keccak256::digest(&value).into());
        assert_eq!(hash, actual_hash);
    }
}

#[test]
fn test_leaf_node_and_extension_node() {
    tracing_sub_init();

    let j = json!([[
        "0xaaab",
        "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
    ]]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let j = json!([[
        "0xaaac",
        "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
    ]]);

    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());

    let mut trie = collection.trie_for(crate::empty_trie_hash());

    for (key, value) in &entries1.data {
        trie.insert(key, value.as_ref().unwrap());
    }

    let patch = trie.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);
    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie = collection.trie_for(first_root.root);

    for (key, value) in &entries2.data {
        trie.insert(key, value.as_ref().unwrap());
    }
    let patch = trie.into_patch();

    let last_root = collection.apply_increase(patch, crate::debug::no_childs);
    debug::draw(
        &collection.database,
        debug::Child::Hash(last_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changeset = diff(
        &collection.database,
        no_childs,
        first_root.root,
        last_root.root,
    )
    .unwrap();

    let result = verify_diff(
        &collection.database,
        last_root.root,
        changeset,
        no_childs,
        true,
    );
    assert!(result.is_ok());

    let diff_patch: VerifiedPatchHexStr = result.unwrap().into();
    let j = json!({
      "patch_dependencies": [],
      "sorted_changes": [
        [
          format!("{:?}", last_root.root),
          "0xe4821aaaa040e05de038a539e7e53d1a02b4d583c0cedb256dec95f0f24025aa72f22bc047"
        ],
        [
          "0x40e05de038a539e7e53d1a02b4d583c0cedb256dec95f0f24025aa72f22bc047",
          "0xf8518080808080808080808080a0d0a649de60d406c5604edfe459e502accc44101096219256e324098fb00bb28da0d0a649de60d406c5604edfe459e502accc44101096219256e324098fb00bb28d80808080"
        ],
        [
          "0xd0a649de60d406c5604edfe459e502accc44101096219256e324098fb00bb28d",
          "0xe320a173616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
        ],
        [
          "0xd0a649de60d406c5604edfe459e502accc44101096219256e324098fb00bb28d",
          "0xe320a173616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
        ]
      ]
    });

    let exp_patch: VerifiedPatchHexStr = serde_json::from_value(j).unwrap();

    assert_eq!(diff_patch, exp_patch);
    drop(last_root);
    log::info!("second trie dropped")
}

pub fn split_changes(input: Vec<Change>) -> (HashSet<H256>, HashSet<H256>) {
    let mut removes = HashSet::<H256>::new();
    let mut inserts = HashSet::<H256>::new();
    for element in input {
        match element {
            Change::Insert(hash, _) => {
                log::trace!(
                    "====================== INSERT: {} ======================",
                    hash
                );
                inserts.insert(hash)
            }
            Change::Removal(hash, _) => {
                log::trace!(
                    "====================== REMOVE: {} ======================",
                    hash
                );
                removes.insert(hash)
            }
        };
    }
    (removes, inserts)
}

#[test]
fn test_diff_with_child_extractor() {
    tracing_sub_init();

    let j = json!([
        [
            "0x00000000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0000000f",
            "0xee00000000000000000000000000000000000000000000000000000000000001"
        ],
        [
            "0x00000300",
            "0xff00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x00003000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0000f300",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0007f000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x000f0000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x03000000",
            "0x0100000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0f33ffff",
            "0x0100000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0xf0fff07f",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0xfffffff0",
            "0xee00000000000000000000000000000000000000000000000000000000000002"
        ],
        [
            "0xffffffff",
            "0xee00000000000000000000000000000000000000000000000000000000000003"
        ]
    ]);
    let entries1_1: debug::EntriesHex = serde_json::from_value(j).unwrap();
    let j = json!([
        [
            "0x00000000",
            "0xff00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0000000f",
            "0xee00000000000000000000000000000000000000000000000000000000000010"
        ],
        [
            "0x00000300",
            "0xff00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x00000f33",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x00003000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0000f300",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0007f000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x000f0000",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x03000000",
            "0x0100000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0x0f33ffff",
            "0x0100000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0xf0fff07f",
            "0xee00000000000000000000000000000000000000000000000000000000000000"
        ],
        [
            "0xfffffff0",
            "0xee00000000000000000000000000000000000000000000000000000000000002"
        ],
        [
            "0xffffffff",
            "0xee00000000000000000000000000000000000000000000000000000000000003"
        ]
    ]);
    let entries1_2: debug::EntriesHex = serde_json::from_value(j).unwrap();
    let j = json!([
        [
            "0x00000000",
            "0xee00000000000000000000000000000000000000000000000000000000000011"
        ],
        [
            "0x0007f000",
            "0xee00000000000000000000000000000000000000000000000000000000000012"
        ],
        [
            "0x03000000",
            "0x0100000000000000000000000000000000000000000000000000000000000111"
        ],
        [
            "0x00000fff",
            "0xff00000000000000000000000000000000000000000000000000000000000000"
        ]
    ]);
    let entries2_2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let keys1 = vec![(hexutil::read_hex("0x00000000").unwrap(), entries1_1)];
    let keys2 = vec![
        (hexutil::read_hex("0x00000000").unwrap(), entries1_2),
        (hexutil::read_hex("0x00000030").unwrap(), entries2_2),
    ];

    let collection1 = TrieCollection::new(SyncDashMap::default());
    let collection2 = TrieCollection::new(SyncDashMap::default());
    let mut collection1_trie1 = RootGuard::new(
        &collection1.database,
        crate::empty_trie_hash(),
        DataWithRoot::get_childs,
    );
    let mut collection1_trie2 = RootGuard::new(
        &collection1.database,
        crate::empty_trie_hash(),
        DataWithRoot::get_childs,
    );
    let mut collection2_trie1 = RootGuard::new(
        &collection2.database,
        crate::empty_trie_hash(),
        DataWithRoot::get_childs,
    );

    for (account_key, storage) in keys1.iter() {
        for (data_key, data) in &storage.data {
            {
                collection1_trie1 = debug::child_extractor::insert_element(
                    &collection1,
                    account_key,
                    data_key,
                    data.as_ref().unwrap(),
                    collection1_trie1.root,
                    DataWithRoot::get_childs,
                );
            }
            {
                collection2_trie1 = debug::child_extractor::insert_element(
                    &collection2,
                    account_key,
                    data_key,
                    data.as_ref().unwrap(),
                    collection2_trie1.root,
                    DataWithRoot::get_childs,
                );
            }
        }
    }

    debug::draw(
        &collection1.database,
        debug::Child::Hash(collection1_trie1.root),
        vec![],
        DataWithRoot::get_childs,
    )
    .print();

    let mut accounts_map: HashMap<Vec<u8>, HashMap<Vec<u8>, Vec<u8>>> = HashMap::new();
    for (account_key, storage) in keys2.iter() {
        let account_updates = accounts_map.entry(account_key.clone()).or_default();
        for (data_key, data) in &storage.data {
            account_updates.insert(data_key.clone(), data.as_ref().unwrap().clone());
        }
    }

    for (account_key, storage) in keys2.iter() {
        for (data_key, data) in &storage.data {
            {
                collection1_trie2 = debug::child_extractor::insert_element(
                    &collection1,
                    account_key,
                    data_key,
                    data.as_ref().unwrap(),
                    collection1_trie2.root,
                    DataWithRoot::get_childs,
                );
            }
        }
    }

    debug::draw(
        &collection1.database,
        debug::Child::Hash(collection1_trie2.root),
        vec![],
        DataWithRoot::get_childs,
    )
    .print();

    let changes = diff(
        &collection1.database,
        DataWithRoot::get_childs,
        collection1_trie1.root,
        collection1_trie2.root,
    )
    .unwrap();

    let verify_result = verify_diff(
        &collection2.database,
        collection1_trie2.root,
        changes.clone(),
        DataWithRoot::get_childs,
        true,
    );

    let diff_patch: VerifiedPatch = verify_result.unwrap();
    let _diff_patch_serialized: VerifiedPatchHexStr = diff_patch.clone().into();
    let (removes, inserts) = split_changes(changes);
    let _common: HashSet<H256> = removes.intersection(&inserts).copied().collect();
    // TODO: uncomment
    // ERROR:
    assert!(_common.is_empty());

    let apply_result = collection2.apply_diff_patch(diff_patch, DataWithRoot::get_childs);
    assert!(apply_result.is_ok());

    let accounts_storage = collection2.trie_for(collection1_trie2.root);
    for (k, storage) in accounts_map {
        let account: DataWithRoot =
            bincode::deserialize(&TrieMut::get(&accounts_storage, &k).unwrap()).unwrap();

        let account_storage_trie = collection2.trie_for(account.root);
        for data_key in storage.keys() {
            assert_eq!(
                &storage[data_key][..],
                &TrieMut::get(&account_storage_trie, data_key).unwrap()
            );
        }
    }
}

#[test]
fn test_try_verify_invalid_changes() {
    tracing_sub_init();
    let collection1 = TrieCollection::new(SyncDashMap::default());
    let collection2 = TrieCollection::new(SyncDashMap::default());
    let j = json!([
        [
            "0xbbaa",
            "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
        ],
        [
            "0xffaa",
            "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
        ],
        [
            "0xbbcc",
            "0x73616d6520646174615f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f"
        ]
    ]);
    let entries: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let mut trie = collection1.trie_for(crate::empty_trie_hash());

    for (key, value) in &entries.data {
        trie.insert(key, value.as_ref().unwrap());
    }
    let patch = trie.into_patch();
    let root_guard = collection1.apply_increase(patch, no_childs);

    debug::draw(
        &collection1.database,
        debug::Child::Hash(root_guard.root),
        vec![],
        no_childs,
    )
    .print();

    log::info!("the only insertion {:?}", root_guard.root);
    let node = collection1.database.get(root_guard.root);
    let changes = vec![Change::Insert(root_guard.root, node.into())];

    let result = verify_diff(
        &collection2.database,
        root_guard.root,
        changes,
        no_childs,
        true,
    );
    log::info!("{:?}", result);
    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        crate::error::Error::Decoder(..) | crate::error::Error::DiffPatchApply(..) => {
            unreachable!()
        }
        crate::error::Error::Verification(verification_error) => {
            match verification_error {
                VerificationError::MissDependencyDB(hash) => {
                    assert_eq!(hash, H256::from_slice(&hexutil::read_hex("0x0a3d3e6b136f84355d29dadc750935a2dac1ea026245dd329fece4ad305e6613").unwrap()))
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn test_try_apply_diff_with_deleted_db_dependency() {
    tracing_sub_init();

    let j = json!([
        [
            "0xb0033333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x33333000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x03333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333b33",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33000000",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f33"
        ]
    ]);

    let entries1: debug::EntriesHex = serde_json::from_value(j).unwrap();

    // One entry removed which eliminates first branch node
    let j = json!([
        [
            "0xb0033333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333b30",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0xf3333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f32"
        ],
        [
            "0x33333333",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ],
        [
            "0x3333333b",
            "0x5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f5f31"
        ]
    ]);
    let entries2: debug::EntriesHex = serde_json::from_value(j).unwrap();

    let collection = TrieCollection::new(SyncDashMap::default());
    let collection2 = TrieCollection::new(SyncDashMap::default());

    let mut trie1 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie1.insert(key, value.as_ref().unwrap());
    }
    let patch = trie1.into_patch();
    let first_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        no_childs,
    )
    .print();

    let mut trie2 = collection.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries2.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let second_root = collection.apply_increase(patch, crate::debug::no_childs);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        no_childs,
    )
    .print();

    let changes = diff(
        &collection.database,
        no_childs,
        first_root.root,
        second_root.root,
    )
    .unwrap();

    let mut trie2 = collection2.trie_for(crate::empty_trie_hash());
    for (key, value) in &entries1.data {
        trie2.insert(key, value.as_ref().unwrap());
    }
    let patch = trie2.into_patch();
    let first_root2 = collection2.apply_increase(patch, crate::debug::no_childs);

    let verify_result = verify_diff(
        &collection2.database,
        second_root.root,
        changes,
        no_childs,
        true,
    );
    assert!(verify_result.is_ok());

    // drop first root, that the patch is supposed to be based onto
    drop(first_root2);
    let apply_result = collection2.apply_diff_patch(verify_result.unwrap(), no_childs);
    assert!(apply_result.is_err());
    let err = unsafe {
        let err = apply_result.unwrap_err_unchecked();
        log::info!("{:?}", err);
        err
    };
    match err {
        crate::error::Error::Decoder(..) | crate::error::Error::Verification(..) => {
            unreachable!()
        }
        crate::error::Error::DiffPatchApply(hash) => {
            assert_eq!(
                hash,
                H256::from_slice(&hex!(
                    "c905cc1f8c7992a69e8deabc075e6daf72029efa76b86be5e71903c0848001fb"
                ))
            )
        }
    }

    drop(second_root);
    log::info!("second trie dropped")
}

use quickcheck::{Gen, QuickCheck, TestResult};

fn reverse_changes(changes: Vec<Change>) -> Vec<Change> {
    changes
        .into_iter()
        .map(|i| match i {
            Change::Insert(h, d) => Change::Removal(h, d),
            Change::Removal(h, d) => Change::Insert(h, d),
        })
        .collect()
}

trait ChildExtractor: Serialize {
    type Child: ChildExtractor;
    fn extract(data: &[u8]) -> Vec<H256> {
        crate::debug::no_childs(data)
    }
    // Change existing data, so it will refer to link root
    fn update_child_root(data: &[u8], _root: H256) -> Vec<u8> {
        data.to_vec()
    }
    fn for_each<F>(&self, f: F)
    where
        F: FnMut(&[u8], &[u8], &Self::Child);
    fn join(&self, other: &Self) -> Self;
}

impl ChildExtractor for () {
    type Child = ();
    fn for_each<F>(&self, _f: F)
    where
        F: FnMut(&[u8], &[u8], &Self::Child),
    {
    }
    fn join(&self, _other: &Self) -> Self {}
}

impl ChildExtractor for EntriesHex {
    type Child = ();

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&[u8], &[u8], &Self::Child),
    {
        for (key, value) in &self.data {
            f(key, value.as_deref().unwrap_or(&[]), &())
        }
    }
    fn join(&self, other: &Self) -> Self {
        EntriesHex::join(self, other)
    }
}

impl ChildExtractor for InnerEntriesHex {
    type Child = EntriesHex;

    fn extract(data: &[u8]) -> Vec<H256> {
        // On RootGuard drop, it handle all subtries as single "data" trie.
        // and execute child signe child extractor for it.
        // TODO: Make RootGuard drop bound to specific layer of trie.
        let result = if data.len() != 32 {
            empty_trie_hash()
        } else {
            H256::from_slice(data)
        };
        vec![result]
    }

    fn update_child_root(_data: &[u8], root: H256) -> Vec<u8> {
        root.as_bytes().to_vec()
    }

    fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&[u8], &[u8], &Self::Child),
    {
        for (key, value) in &self.data {
            // This struct is designed for tests and does not contain any old roots.
            // So during insert we always create new trie
            f(key, empty_trie_hash().as_bytes(), value)
        }
    }
    fn join(&self, other: &Self) -> Self {
        InnerEntriesHex::join(self, other)
    }
}

type ChildExtractorFn = fn(&[u8]) -> Vec<H256>;

fn insert_entries<'a, D, DB>(
    collection: &'a TrieCollection<DB>,
    root: H256,
    entries: &D,
) -> RootGuard<'a, DB, ChildExtractorFn>
where
    DB: DbCounter + Database,
    D: ChildExtractor,

    // Limit to max two layers.
    // Second layer should has Childs ().
    <D as ChildExtractor>::Child: ChildExtractor<Child = ()>,
{
    let mut trie1 = collection.trie_for(root);
    let mut root_guards = HashMap::new();
    entries.for_each(|key, value, child| {
        let old_data = TrieMut::get(&trie1, key);
        let old_roots = old_data
            .as_deref()
            .map(D::extract)
            .unwrap_or_else(|| vec![empty_trie_hash()]);
        match *old_roots {
            [] => trie1.insert(key, value),
            [old_root] => {
                let mut sub_trie1 = collection.trie_for(old_root);
                child.for_each(|k, v, _| {
                    sub_trie1.insert(k, v);
                });
                let patch = sub_trie1.into_patch();

                let root_guard =
                    // currently rootguard is limited to one layer of indirection
                    collection.apply_increase(patch.clone(), D::Child::extract as ChildExtractorFn);

                root_guards.remove(&old_root);
                root_guards.insert(root_guard.root, root_guard);
                let data = D::update_child_root(value, patch.root);
                trie1.insert(key, data.as_ref())
            }
            _ => panic!("Expecting only 0 or 1 child tries"),
        }
    });

    let patch = trie1.into_patch();

    // internall check: assert that all storage roots was written to the storage
    let mut roots_set: HashSet<_> = root_guards.keys().copied().collect();
    {
        for (_k, v) in &patch.change.changes {
            if let Some(n) = v {
                let node = crate::rlp::decode(n).unwrap();
                let childs = ReachableHashes::collect(&node, D::extract).childs();
                for n in childs.0.into_iter().chain(childs.1) {
                    roots_set.remove(&n);
                }
            }
        }
    }
    roots_set.remove(&empty_trie_hash());

    assert!(roots_set.is_empty());

    collection.apply_increase(patch, D::extract as ChildExtractorFn)
}

// Check that after inserting entries, data in trie correct.
// 1. Check that for low level trie db.get(k) == v from original entries;
// 2. Check that for high level trie db.get(key) == (value + update_child_root(new_root))
fn assert_contain<D, DB>(collection: &TrieCollection<DB>, root: H256, entries: &D)
where
    DB: DbCounter + Database,
    D: ChildExtractor,

    // Limit to max two layers.
    // Second layer should has Childs ().
    <D as ChildExtractor>::Child: ChildExtractor<Child = ()>,
{
    let trie1 = collection.trie_for(root);
    entries.for_each(|key, value, child| {
        let data_from_db = TrieMut::get(&trie1, key).unwrap_or_default();

        let roots = D::extract(&data_from_db);
        let data_with_root = match *roots {
            [] => data_from_db.clone(),
            [root] => {
                let sub_trie1 = collection.trie_for(root);
                child.for_each(|k, v, _| {
                    assert_eq!(
                        hexutil::to_hex(&TrieMut::get(&sub_trie1, k).unwrap_or_default()),
                        hexutil::to_hex(v)
                    );
                });
                D::update_child_root(value, root)
            }
            _ => {
                panic!("Expecting only 0 or 1 child tries")
            }
        };

        assert_eq!(
            hexutil::to_hex(&data_from_db),
            hexutil::to_hex(&data_with_root)
        );
    });
}

fn empty_keys_union_diff_intersection_test_body<D>(
    entries_1: D,
    entries_2: D,
    redundancy_checks: bool,
) where
    D: ChildExtractor,

    // Limit to max two layers.
    // Second layer should has no childs Child=().
    <D as ChildExtractor>::Child: ChildExtractor<Child = ()>,
{
    let collection = TrieCollection::new(SyncDashMap::default());
    let collection_2 = TrieCollection::new(SyncDashMap::default());

    let first_root = insert_entries(&collection, empty_trie_hash(), &entries_1);
    let first_root_2 = insert_entries(&collection_2, empty_trie_hash(), &entries_1);
    assert_eq!(first_root.root, first_root_2.root);

    debug::draw(
        &collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        D::extract,
    )
    .print();

    let second_root = insert_entries(&collection, first_root.root, &entries_2);

    debug::draw(
        &collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        D::extract,
    )
    .print();

    let changes = diff(
        &collection.database,
        D::extract,
        first_root.root,
        second_root.root,
    )
    .unwrap();

    if redundancy_checks {
        let (removes, inserts) = split_changes(changes.clone());

        let common: HashSet<H256> = removes.intersection(&inserts).copied().collect();
        assert!(common.is_empty());
    }

    let verify_result = verify_diff(
        &collection_2.database,
        second_root.root,
        changes,
        D::extract,
        true,
    );

    let verify_result = verify_result.unwrap();
    let apply_result = collection_2.apply_diff_patch(verify_result, D::extract);
    let _rg = apply_result.unwrap();

    let entries = entries_1.join(&entries_2);
    assert_contain(&collection_2, second_root.root, &entries);
}

fn empty_keys_distinct_diff_empty_intersection_and_reversal_test_body<D>(
    entries_1: D,
    entries_2: D,
    redundancy_checks: bool,
) where
    D: ChildExtractor,
    // Limit to max two layers.
    // Second layer should has no childs Child=().
    <D as ChildExtractor>::Child: ChildExtractor<Child = ()>,
{
    let full_collection = TrieCollection::new(SyncDashMap::default());
    let collection_reversal_target = TrieCollection::new(SyncDashMap::default());
    let collection_direct_target = TrieCollection::new(SyncDashMap::default());

    let first_root = insert_entries(&full_collection, empty_trie_hash(), &entries_1);
    let _first_root = insert_entries(&collection_direct_target, empty_trie_hash(), &entries_1);

    debug::draw(
        &full_collection.database,
        debug::Child::Hash(first_root.root),
        vec![],
        D::extract,
    )
    .print();

    let second_root = insert_entries(&full_collection, empty_trie_hash(), &entries_2);
    let _second_root = insert_entries(&collection_reversal_target, empty_trie_hash(), &entries_2);

    debug::draw(
        &full_collection.database,
        debug::Child::Hash(second_root.root),
        vec![],
        D::extract,
    )
    .print();

    let changes = diff(
        &full_collection.database,
        D::extract,
        first_root.root,
        second_root.root,
    )
    .unwrap();

    if redundancy_checks {
        let (removes, inserts) = split_changes(changes.clone());
        assert_eq!(removes.len() + inserts.len(), changes.len());

        let common: HashSet<H256> = removes.intersection(&inserts).copied().collect();
        assert!(common.is_empty());
    }
    let reversed = reverse_changes(changes.clone());

    for (changes, collection, target_root, tested_entries) in [
        (
            changes,
            &collection_direct_target,
            second_root.root,
            &entries_2,
        ),
        (
            reversed,
            &collection_reversal_target,
            first_root.root,
            &entries_1,
        ),
    ] {
        debug::draw(
            &full_collection.database,
            debug::Child::Hash(target_root),
            vec![],
            D::extract,
        )
        .print();

        let verify_result =
            verify_diff(&collection.database, target_root, changes, D::extract, true);
        let verify_result = verify_result.unwrap();

        let apply_result = collection.apply_diff_patch(verify_result, D::extract);
        assert!(apply_result.is_ok());
        // removing duplicates from tested_entries, checking for last value

        assert_contain(collection, target_root, tested_entries);
    }
}

type FixedKeyUniqueValues = NodesGenerator<debug::EntriesHex, FixedKey, UniqueValue>;
type VariableKeyUniqueValues = NodesGenerator<debug::EntriesHex, VariableKey, UniqueValue>;

type FixedKeyUniqueValuesInner = NodesGenerator<debug::InnerEntriesHex, FixedKey, UniqueValue>;
type VariableKeyUniqueValuesInner =
    NodesGenerator<debug::InnerEntriesHex, VariableKey, UniqueValue>;

type VariableKeyMixedValues = NodesGenerator<debug::EntriesHex, VariableKey, MixedNonUniqueValue>;

type VariableKeyMixedValuesInner =
    NodesGenerator<debug::InnerEntriesHex, VariableKey, MixedNonUniqueValue>;

macro_rules! generate_tests {
    ($name: ident => $type_name:ident, $redundancy_checks: expr) => {
        mod $name {
            use super::*;
            #[test]
            fn qc_unique_nodes_empty_diff_intersection1() {
                fn property(gen_1: $type_name, gen_2: $type_name) -> TestResult {
                    tracing_sub_init();
                    if gen_1.data.data.is_empty()
                        || gen_2.data.data.is_empty()
                        || gen_1.data.data == gen_2.data.data
                    {
                        return TestResult::discard();
                    }
                    log::warn!(
                        "entries_1 = {}",
                        serde_json::to_string_pretty(&gen_1.data).unwrap()
                    );
                    log::warn!(
                        "entries_2 = {}",
                        serde_json::to_string_pretty(&gen_2.data).unwrap()
                    );
                    empty_keys_union_diff_intersection_test_body(
                        gen_1.data,
                        gen_2.data,
                        $redundancy_checks,
                    );

                    TestResult::passed()
                }
                QuickCheck::new()
                    .gen(Gen::new(RNG_DATA_SIZE))
                    // .tests(20_000)
                    .quickcheck(property as fn(gen_1: $type_name, gen_2: $type_name) -> TestResult);
            }

            #[test]
            fn qc_unique_nodes_empty_diff_intersection_and_reversal() {
                fn property(gen_1: $type_name, gen_2: $type_name) -> TestResult {
                    tracing_sub_init();
                    if gen_1.data.data.is_empty()
                        || gen_2.data.data.is_empty()
                        || gen_1.data.data == gen_2.data.data
                    {
                        return TestResult::discard();
                    }

                    log::warn!(
                        "entries_1 = {}",
                        serde_json::to_string_pretty(&gen_1.data).unwrap()
                    );
                    log::warn!(
                        "entries_2 = {}",
                        serde_json::to_string_pretty(&gen_2.data).unwrap()
                    );
                    empty_keys_distinct_diff_empty_intersection_and_reversal_test_body(
                        gen_1.data,
                        gen_2.data,
                        $redundancy_checks,
                    );

                    TestResult::passed()
                }
                QuickCheck::new()
                    .gen(Gen::new(RNG_DATA_SIZE))
                    // .tests(1000)
                    .quickcheck(property as fn(gen_1: $type_name, gen_2: $type_name) -> TestResult);
            }
        }
    };
}

generate_tests! {
    fixed_key=> FixedKeyUniqueValues, true
}

generate_tests! {
    variable_key=> VariableKeyUniqueValues, true
}

generate_tests! {
    inner_fixed_key=> FixedKeyUniqueValuesInner, true
}
generate_tests! {
    inner_variable_key=> VariableKeyUniqueValuesInner, true
}

generate_tests! {
    variable_key_mixed_values=> VariableKeyMixedValues, false
}

generate_tests! {
    inner_variable_key_mixed_values=> VariableKeyMixedValuesInner, false
}
#[test]
fn data_from_qc1() {
    tracing_sub_init();

    let entries_1: EntriesHex = serde_json::from_str(
        r#"[
       [
        "0x",
        "0x7199cc2c1d2501ab5fbcffc4e16e00339affab5c9ea20dff811c3784680108be"
      ]
    ]"#,
    )
    .unwrap();
    let entries_2 = serde_json::from_str(
        r#"[
      [
        "0x3033",
        "0x24144541235e771b0f77e931e2ff29ba3c08018545d99fc2b60005ba02ff2506"
      ],
      [
        "0x777f03",
        "0xc09c763374e63ca45ff7f108ff1942a85fb7a619e200811f8e00de6a40004eca"
      ],
      [
        "0x3b3b37bfbf",
        "0xff1ec626ffc001010783ac9e485a54bdb27d6c4f0bd220964f02fce24ab3f279"
      ],
      [
        "0xf7f3",
        "0x7d2b4d6c70bd00b42074190baf8f94ca4c67ffdbcda63af10048d23865ff3824"
      ]
    ]"#,
    )
    .unwrap();

    log::warn!(
        "entries_1 = {}",
        serde_json::to_string_pretty(&entries_1).unwrap()
    );
    log::warn!(
        "entries_2 = {}",
        serde_json::to_string_pretty(&entries_2).unwrap()
    );
    empty_keys_union_diff_intersection_test_body(entries_1, entries_2, true);
}

// this test was original error, but after fix serialization, is only example of usage reduce method
#[test]
#[should_panic = "Cant reduce, input valid"]
fn data_from_qc2() {
    tracing_sub_init();

    let entries_1: EntriesHex = serde_json::from_str(
        r#"[
  [
    "0xf30f",
    "0x6c144af1669647efad4feba9c30009180e9d844672c1017a009bdcb6b6bb32c7"
  ],
  [
    "0x000b33",
    "0xa65b8663"
  ],
  [
    "0xb0f77f0b7f33",
    "0xe08df91f7bef9c2aeb117d8afb00c0a4fc00b8fcac010327aff400535d385501"
  ],
  [
    "0xf0bb333fb7",
    "0x5568e702e3c22b011b524ce26fb29ea336bddc8084c7b47cedff8d3ddcb8e5dc"
  ],
  [
    "0xf3f0b7",
    "0xf8e63e9312c1fa0b59653cc627562c5f824774374a8183afab15f70134d21e8c"
  ],
  [
    "0x00f3",
    "0x7bff657a"
  ],
  [
    "0x000007",
    "0x202eeaea"
  ],
  [
    "0xf0",
    "0x5b1222f8"
  ],
  [
    "0x37f7bbfb",
    "0x14858b60587d2b06531c1e4ac5d5231e6f96f3c46d6b23fd0d5df09682816ba0"
  ],
  [
    "0x3fb03bf73f3b",
    "0x19ca01d2"
  ],
  [
    "0xf7fb",
    "0x9501c26cfd01b2acff6b9feb9d98004be9ec32ffe28cd4ef1a5db84c05784983"
  ],
  [
    "0xbf7b",
    "0x4b055c40aae61e10d272009d5c1acad400fd4a442300edebff2733dee1fac0c0"
  ],
  [
    "0x3fb737ff",
    "0x2aa9ed57f83d9cb69c8f4854042fd96f8f16415e010763f1f79546d220428486"
  ],
  [
    "0xbb",
    "0x1eb922e3"
  ],
  [
    "0x73",
    "0x6bcf5e7334cd0e00c80106013021ac73037bb1a3daf9d6805b243852432e0001"
  ],
  [
    "0x00",
    "0x913b6f3c"
  ],
  [
    "0xf330b730f0",
    "0xa5bef3162562ffdd8523f33e27fcb23c8d05213c2dce7cb1c6a5d9f7c76832d2"
  ],
  [
    "0xb3b7f70ffb",
    "0xf4a500b5de0f0091c245810720d098904000df927a68ed4d13691957e02027fe"
  ],
  [
    "0xff7000",
    "0x890b3a8ed422e047fd7da0014a67ffaa058300a99aa0437a01ad014e6efe20ff"
  ],
  [
    "0x33fb3f3f",
    "0x292d01ff790c00bc3de801660000ff06f55e9529aa0c1dee00c2bac3e201847b"
  ],
  [
    "0x377f7b3ff073",
    "0xbeec13bd1dd60d6a7982f49e64967275e3f7259900f01fd5bdec25fb2cd41350"
  ],
  [
    "0x3b",
    "0xf48f38f23da27e28957a8ea59270a052e6157e329b3bde171353c7805905b7ad"
  ],
  [
    "0x7b",
    "0x3616d7966fabeaeb03daf401e3bf0c0826f65344baee87ff3e2d13754236ca79"
  ],
  [
    "0x7bbf",
    "0xbcf9697c0bc679ff5da78000818d1509104ba92ef459ff76738fff0c19223013"
  ],
  [
    "0xbf",
    "0x8303ff5be833a018bab53f00303fd5cd06b4fbb3873c416bf23e8808317d6a95"
  ],
  [
    "0x0bf3f0bf",
    "0xedfc20d9128d95085701899fff8ed5dcb7919434e3defff680699fcae5ebc02f"
  ],
  [
    "0xbb703b",
    "0x00e5d886d4e6a5b1da2f7ed0001f4053fdfbff3488ff00629a83d894d994e664"
  ],
  [
    "0x33f3",
    "0xb658a8c0ff8e387329887da3fd0130e5703bd4aff9b6d709c91de6a7432a436a"
  ],
  [
    "0xfbb0",
    "0xfffb9ab9969af7c850c4b3ac12123001a0705f502b0c202826e565a1e46cf4d8"
  ],
  [
    "0x0303bbf7",
    "0xa16546fc0095008ed3599d2ad051ffa77efffff74dd4e9464249e2a549fd37f9"
  ],
  [
    "0x073f377b73",
    "0x8dff04cdd9e0c04661bf0248cf4dbe537158749329b993440800c71f840081ce"
  ],
  [
    "0x377b7030bfb7",
    "0xa96445b1470ed4b351817c4900a253de3cffc57069c6239e0101056d1c290158"
  ],
  [
    "0xb7",
    "0x9709ef8771813000fb638b415a39539e91ad00012634d746f7f439d23a0101fe"
  ],
  [
    "0xbb3bb7",
    "0x32c35437ff34a4bd553d97124e46d71327df74dd42362c0120e92b01ef11fa7e"
  ],
  [
    "0xf3373f7f07",
    "0x450cc5b5459f90f1007af7330100732b4ef93ec12b4e4b0ddf4f7b35004f43fa"
  ],
  [
    "0x70fbbb0ff3",
    "0xf1e5c0217bcbff31d5f0ef01bae7ffd860ffd7291fe9a94fd50bb361b9201925"
  ],
  [
    "0x3f3f",
    "0x8714b788e2506f05df258fb0d92eefa4ac1e6754436a259926531662dfaba3fc"
  ],
  [
    "0x703f33b70f",
    "0xd218f04cd8b3ff90eb21b188a3acd7012ab6cc3b1be5c2295cb250c42101c4e2"
  ],
  [
    "0x3fb7",
    "0x3b38075b7f4828285c78f28b5fff3544f4b77a1a45c661dbf1280dd9afe7d20e"
  ],
  [
    "0x700307333fb3",
    "0x27c6a58892cd54f1362818863b929082cff32becba5a00ffb4f7fb019d5f39a8"
  ],
  [
    "0x7f30bf07bfb3",
    "0xc4489cff00b468f10e71fc07a063c79719c37275eb309533b5003f5abe9f2093"
  ],
  [
    "0x7b3ff7003bbf",
    "0x9c28c3d0d5d148ff9476b4b26d05f2293bb074c527cb35f9150dca0ba8b142ef"
  ],
  [
    "0x73377ff307fb",
    "0xd5c3b2289cdf016d105cb36bc0fc2c19e2bfe102dfa0f0847f2eaf4e33a90010"
  ],
  [
    "0x3037b0ff00",
    "0xd7470f3b385d2c06a3b7ea577ff701a49000016d33d79447054e4167216d5174"
  ],
  [
    "0x0bbf",
    "0xcf6af87b00b98c8f003a5b0020dbef6561df4653a914fb3239a7320b97e85ece"
  ],
  [
    "0x03f33b",
    "0x5537d535923d60864d01160ca82701007fb79bcaf113e79516003a4da9cbf282"
  ],
  [
    "0xb3733ff0b7",
    "0x0141d5f45300cc13c166e61ae39ce8eee92b0133fd46566d0029d605aae5518e"
  ]
]"#,
    )
    .unwrap();

    log::warn!(
        "entries_1 = {}",
        serde_json::to_string_pretty(&entries_1).unwrap()
    );

    fn reduce(mut entries: EntriesHex, func: impl Fn(&EntriesHex) -> bool) {
        if func(&entries) {
            panic!("Cant reduce, input valid")
        }
        let mut idx_remove = 0;
        while dbg!(entries.data.len()) > dbg!(idx_remove) {
            let mut entries_cloned = entries.clone();
            entries_cloned.data.remove(idx_remove);
            let stil_panic = !func(&entries_cloned);
            if stil_panic {
                entries = entries_cloned
            } else {
                idx_remove += 1;
            }
        }
        log::warn!(
            "result_entries = {}",
            serde_json::to_string_pretty(&entries).unwrap()
        );
    }

    reduce(entries_1, |entries_1| {
        std::panic::catch_unwind(|| {
            let collection = TrieCollection::new(SyncDashMap::default());
            let first_root = insert_entries(&collection, empty_trie_hash(), entries_1);
            debug::draw(
                &collection.database,
                debug::Child::Hash(first_root.root),
                vec![],
                EntriesHex::extract,
            )
            .print();
        })
        .is_ok()
    });
    // panic!();
}
