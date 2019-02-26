// Copyright 2018 Commonwealth Labs, Inc.
// This file is part of Edgeware.

// Edgeware is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Edgeware is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Edgeware.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate serde;

// Needed for deriving `Serialize` and `Deserialize` for various types.
// We only implement the serde traits for std builds - they're unneeded
// in the wasm runtime.
//#[cfg(feature = "std")]

extern crate parity_codec as codec;
extern crate substrate_primitives as primitives;
extern crate sr_std as rstd;
extern crate srml_support as runtime_support;
extern crate sr_primitives as runtime_primitives;
extern crate sr_io as runtime_io;
extern crate srml_balances as balances;
extern crate srml_system as system;
extern crate edge_delegation as delegation;
extern crate bellman;
extern crate ff;
extern crate num_bigint;
extern crate num_traits;
extern crate sapling_crypto;

use sapling_crypto::{
    babyjubjub::{
        JubjubBn256,
    },
};
use num_traits::Num;
use ff::{BitIterator, PrimeField, Field};
use pairing::{bn256::{Bn256, Fr}};
use rstd::prelude::*;
use system::ensure_signed;
use runtime_support::{StorageValue, StorageMap};
use runtime_support::dispatch::Result;
use runtime_primitives::traits::Hash;
use runtime_primitives::traits::{Zero};
use codec::Encode;


use bellman::groth16::{Proof, Parameters, verify_proof, prepare_verifying_key};
use num_bigint::BigInt;

#[cfg_attr(feature = "std", derive(Debug))]
#[derive(Encode, Decode, PartialEq)]
pub struct MTree<Balance> {
    pub fee: Balance,
    pub depth: u32,
    pub leaf_count: u64
}

const DEFAULT_TREE_DEPTH: u32 = 32;
// TODO: Better estimates/decisions
const MAX_DEPTH: u32 = 256;

pub trait Trait: balances::Trait + delegation::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

        pub fn create_tree(origin, _fee: Option<T::Balance>, _depth: Option<u32>, _leaves: Option<Vec<Vec<u8>>>) -> Result {
            let _sender = ensure_signed(origin)?;

            let fee = match _fee {
                Some(f) => f,
                None => Zero::zero(),
            };

            let depth = match _depth {
                Some(d) => d,
                None => DEFAULT_TREE_DEPTH,
            };
            ensure!(depth < MAX_DEPTH, "Fee is too large");

            let ctr = Self::number_of_trees();
            for i in 0..depth {
                let empty_level = vec![vec![]; 2_i32.pow(i) as usize];
                <MerkleTreeLevels<T>>::insert((ctr, i), empty_level);
            }

            let mtree = MTree {
                fee: fee,
                depth: depth,
                leaf_count: 0,
            };
            
            <MerkleTreeMetadata<T>>::insert(ctr, mtree);
            <NumberOfTrees<T>>::put(ctr + 1);

            if let Some(leaves) = _leaves {
                for i in 0..leaves.len() {
                    Self::add_leaf_element(ctr, leaves[i].clone());
                }
            }

            Ok(())
        }

        pub fn add_leaf(origin, tree_id: u32, leaf_value: Vec<u8>) -> Result {
            let _sender = ensure_signed(origin)?;
            let tree = <MerkleTreeMetadata<T>>::get(tree_id).ok_or("Tree doesn't exist")?;
            ensure!(<balances::Module<T>>::free_balance(_sender.clone()) >= tree.fee, "Insufficient balance from sender");    
            ensure!(tree.leaf_count < 2_i32.pow(tree.depth) as u64, "Insufficient capacity in tree");

            Self::add_leaf_element(tree_id, leaf_value);
            Ok(())
        }

        pub fn verify_zkproof(origin, tree_id: u32, _params: Vec<u8>, _proof: Vec<u8>, _nullifier_hex: Vec<u8>, _root_hex: Vec<u8>) -> Result {
            let _sender = ensure_signed(origin)?;
            let params = String::from_utf8(_params).expect("Found invalid UTF-8");
            let proof = String::from_utf8(_proof).expect("Found invalid UTF-8");
            let nullifier_hex = String::from_utf8(_nullifier_hex.clone()).expect("Found invalid UTF-8");

            let tree_root = &<MerkleTreeLevels<T>>::get((tree_id, 0)).unwrap()[0];
            let root_hex = String::from_utf8(tree_root.to_vec()).expect("Invalid root");

            let params_hex = hex::decode(params).expect("Decoding params failed");
            let de_params = Parameters::read(&params_hex[..], true).expect("Param bellman decode failed");


            let pvk = prepare_verifying_key::<Bn256>(&de_params.vk);
            // Nullifier
            let nullifier_big = BigInt::from_str_radix(&nullifier_hex, 16).expect("Nullfier decode failed");
            let nullifier_raw = &nullifier_big.to_str_radix(10);
            let nullifier = Fr::from_str(nullifier_raw).ok_or("couldn't parse Fr")?;
            // Root hash
            let root_big = BigInt::from_str_radix(&root_hex, 16).expect("Root decode failed");
            let root_raw = &root_big.to_str_radix(10);
            let root = Fr::from_str(root_raw).ok_or("couldn't parse Fr")?;
            let _result = verify_proof(
                &pvk,
                &Proof::read(&hex::decode(proof).expect("Proof hex decode failed")[..]).expect("Proof decode failed"),
                &[
                    nullifier,
                    root
                ]).expect("Verify proof failed");

            if _result {
                <UsedNullifiers<T>>::insert(_nullifier_hex, true);    
            }
            
            Ok(())
        }
	}
}

impl<T: Trait> Module<T> {
    fn add_leaf_element(key: u32, leaf: Vec<u8>) {
        let mut tree = <MerkleTreeMetadata<T>>::get(key).ok_or("Tree doesn't exist").unwrap();
        // Add element
        let leaf_index = tree.leaf_count;
        tree.leaf_count += 1;
        if let Some(mut mt_level) = <MerkleTreeLevels<T>>::get((key, tree.depth)) {
            mt_level.push(leaf);
            <MerkleTreeLevels<T>>::insert((key, tree.depth), mt_level);
        }
        

        let mut curr_index = leaf_index as usize;
        let mut leaf1: Vec<u8>;
        let mut leaf2: Vec<u8>;
        // Update the tree
        for i in 0..tree.depth {
            let next_index = curr_index / 2;
            let inx = i as usize;
            if curr_index % 2 == 0 {
                let leaf1_val = &<MerkleTreeLevels<T>>::get((key, tree.depth - i)).unwrap()[curr_index];
                leaf1 = leaf1_val.to_vec();
                let leaf2_val = &<MerkleTreeLevels<T>>::get((key, tree.depth - i)).unwrap()[curr_index + 1];
                leaf2 = Self::get_unique_leaf(leaf2_val.to_vec(), inx);
            } else {
                let leaf1_val = &<MerkleTreeLevels<T>>::get((key, tree.depth - i)).unwrap()[curr_index - 1];
                leaf1 = Self::get_unique_leaf(leaf1_val.to_vec(), inx);
                let leaf2_val = &<MerkleTreeLevels<T>>::get((key, tree.depth - i)).unwrap()[curr_index];
                leaf2 = leaf2_val.to_vec();
            }

            if let Some(mut level) = <MerkleTreeLevels<T>>::get((key, tree.depth - i + 1)) {
                level[next_index] = Self::convert_point_to_bytes(
                    Self::hash_from_halves(
                        Self::convert_bytes_to_point(leaf1),
                        Self::convert_bytes_to_point(leaf2),
                        Some(inx)
                    )
                );

                <MerkleTreeLevels<T>>::insert((key, tree.depth - i + 1), level);
            }

            curr_index = next_index;
        }

        <MerkleTreeMetadata<T>>::insert(key, tree);
    }

    fn convert_bytes_to_point(bytes: Vec<u8>) -> Fr {
        let big = BigInt::from_str_radix(&hex::encode(bytes), 16).unwrap();
        let raw = &big.to_str_radix(10);
        let pt = Fr::from_str(raw).ok_or("couldn't parse Fr").unwrap();
        return pt;
    }

    fn convert_point_to_bytes(pt: Fr) -> Vec<u8> {
        return pt.to_hex().as_bytes().to_vec();
    }

    fn hash_from_halves(left: Fr, right: Fr, index: Option<usize>) -> Fr {
        let params = &JubjubBn256::new();
        let mut lhs: Vec<bool> = BitIterator::new(left.into_repr()).collect();
        let mut rhs: Vec<bool> = BitIterator::new(right.into_repr()).collect();
        lhs.reverse();
        rhs.reverse();

        let personalization = if index.is_none() {
            sapling_crypto::baby_pedersen_hash::Personalization::NoteCommitment
        } else {
            sapling_crypto::baby_pedersen_hash::Personalization::MerkleTree(index.unwrap())
        };

        let hash = sapling_crypto::baby_pedersen_hash::pedersen_hash::<Bn256, _>(
            personalization,
            lhs.into_iter()
               .take(Fr::NUM_BITS as usize)
               .chain(rhs.into_iter().take(Fr::NUM_BITS as usize)),
            params
        ).into_xy().0;
        
        return hash;
    }

    fn compute_new_root(mut nodes: Vec<Fr>, depth: usize) -> Fr {
        if nodes.len() == 2 {
            let l = nodes.remove(0);
            let r = nodes.remove(0);
            return Self::hash_from_halves(l, r, Some(depth - 1));
        } else {
            let left_nodes = nodes[..(nodes.len() / 2)].to_vec();
            let right_nodes = nodes[(nodes.len() / 2)..].to_vec();
            return Self::hash_from_halves(
                Self::compute_new_root(left_nodes, depth - 1),
                Self::compute_new_root(right_nodes, depth - 1),
                Some(depth),
            );
        }
    }

    pub fn get_unique_leaf(leaf: Vec<u8>, index: usize) -> Vec<u8> {
        if leaf != vec![] {
            return leaf;
        } else {
            return Self::convert_point_to_bytes(Self::get_precomputes(index));
        }

    }

    pub fn get_precomputes(index: usize) -> Fr {
        let mut pt = pairing::bn256::Fr::zero();

        for _ in 0..index {
            pt = Self::hash_from_halves(pt.clone(), pt.clone(), Some(index));
        }

        return pt;
    }
}

/// An event in this module.
decl_event!(
	pub enum Event<T> where <T as system::Trait>::Hash {
		/// new vote (id, creator, type of vote)
		NewLeaf(Hash, Hash),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as MerkleTree {
		pub NumberOfTrees get(number_of_trees): u32;
		pub MerkleTreeMetadata get(merkle_tree_metadata): map u32 => Option<MTree<T::Balance>>;
        pub MerkleTreeLevels get(merkle_tree_level): map (u32, u32) => Option<Vec<Vec<u8>>>;
        pub UsedNullifiers get(used_nullifiers): map Vec<u8> => bool;
	}
}
