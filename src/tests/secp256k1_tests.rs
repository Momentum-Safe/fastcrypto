// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{
    secp256k1::{
        Secp256k1KeyPair, Secp256k1PrivateKey, Secp256k1PublicKey, Secp256k1PublicKeyBytes,
        Secp256k1Signature,
    },
    traits::{EncodeDecodeBase64, KeyPair, ToFromBytes, VerifyingKey},
};

use digest::Digest;
use rand::{rngs::StdRng, SeedableRng as _};
use rust_secp256k1::{constants, ecdsa::Signature};
use signature::{Signer, Verifier};
use wycheproof::ecdsa::{TestName::EcdsaSecp256k1Sha256, TestSet};

pub fn keys() -> Vec<Secp256k1KeyPair> {
    let mut rng = StdRng::from_seed([0; 32]);

    (0..4)
        .map(|_| Secp256k1KeyPair::generate(&mut rng))
        .collect()
}

#[test]
fn serialize_deserialize() {
    let kpref = keys().pop().unwrap();
    let public_key = kpref.public();

    let bytes = bincode::serialize(&public_key).unwrap();
    let pk2 = bincode::deserialize::<Secp256k1PublicKey>(&bytes).unwrap();
    assert_eq!(public_key.as_ref(), pk2.as_ref());

    let private_key = kpref.private();
    let bytes = bincode::serialize(&private_key).unwrap();
    let privkey = bincode::deserialize::<Secp256k1PrivateKey>(&bytes).unwrap();
    let bytes2 = bincode::serialize(&privkey).unwrap();
    assert_eq!(bytes, bytes2);

    let signature = Secp256k1Signature::default();
    let bytes = bincode::serialize(&signature).unwrap();
    let sig = bincode::deserialize::<Secp256k1Signature>(&bytes).unwrap();
    let bytes2 = bincode::serialize(&sig).unwrap();
    assert_eq!(bytes, bytes2);

    // test serde_json serialization
    let serialized = serde_json::to_string(&signature).unwrap();
    println!("{:?}", serialized);
    let deserialized: Secp256k1Signature = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.as_ref(), signature.as_ref());
}

#[test]
fn import_export_public_key() {
    let kpref = keys().pop().unwrap();
    let public_key = kpref.public();
    let export = public_key.encode_base64();
    let import = Secp256k1PublicKey::decode_base64(&export);
    assert!(import.is_ok());
    assert_eq!(import.unwrap().as_ref(), public_key.as_ref());
}

#[test]
fn test_public_key_bytes_conversion() {
    let kp = keys().pop().unwrap();
    let pk_bytes: Secp256k1PublicKeyBytes = kp.public().into();
    let rebuilded_pk: Secp256k1PublicKey = pk_bytes.try_into().unwrap();
    assert_eq!(kp.public().as_bytes(), rebuilded_pk.as_bytes());
}

#[test]
fn test_public_key_recovery() {
    let kp = keys().pop().unwrap();
    let message: &[u8] = b"Hello, world!";
    let signature: Secp256k1Signature = kp.sign(message);
    let recovered_key = signature
        .recover(<sha3::Keccak256 as sha3::digest::Digest>::digest(message).as_slice())
        .unwrap();
    assert_eq!(*kp.public(), recovered_key);
}

#[test]
fn test_public_key_recovery_error() {
    // incorrect length
    assert!(<Secp256k1Signature as ToFromBytes>::from_bytes(&[0u8; 1]).is_err());

    // invalid recovery id at index 65
    assert!(<Secp256k1Signature as ToFromBytes>::from_bytes(&[4u8; 65]).is_err());

    let signature = <Secp256k1Signature as ToFromBytes>::from_bytes(&[0u8; 65]).unwrap();
    let message: &[u8] = b"Hello, world!";
    assert!(signature
        .recover(<sha3::Keccak256 as sha3::digest::Digest>::digest(message).as_slice())
        .is_err());

    let kp = keys().pop().unwrap();
    let signature_2: Secp256k1Signature = kp.sign(message);
    assert!(signature_2.recover(message).is_err());
}
#[test]
fn import_export_secret_key() {
    let kpref = keys().pop().unwrap();
    let secret_key = kpref.private();
    let export = secret_key.encode_base64();
    let import = Secp256k1PrivateKey::decode_base64(&export);
    assert!(import.is_ok());
    assert_eq!(import.unwrap().as_ref(), secret_key.as_ref());
}

#[test]
fn test_copy_key_pair() {
    let kp = keys().pop().unwrap();
    let kp_copied = kp.copy();

    assert_eq!(kp.public().as_bytes(), kp_copied.public().as_bytes());
    assert_eq!(kp.private().as_bytes(), kp_copied.private().as_bytes());
}

#[test]
fn to_from_bytes_signature() {
    let kpref = keys().pop().unwrap();
    let signature = kpref.sign(b"Hello, world!");
    let sig_bytes = signature.as_ref();
    let rebuilt_sig = <Secp256k1Signature as ToFromBytes>::from_bytes(sig_bytes).unwrap();
    assert_eq!(rebuilt_sig.as_ref(), signature.as_ref())
}

#[test]
fn verify_valid_signature() {
    // Get a keypair.
    let kp = keys().pop().unwrap();

    // Sign over raw message, hashed to keccak256.
    let message: &[u8] = b"Hello, world!";
    let digest = message.digest();

    let signature = kp.sign(&digest.0);

    // Verify the signature.
    assert!(kp.public().verify(&digest.0, &signature).is_ok());
}

#[test]
fn verify_valid_signature_against_hashed_msg() {
    // Get a keypair.
    let kp = keys().pop().unwrap();

    // Sign over raw message (hashed to keccak256 internally).
    let message: &[u8] = b"Hello, world!";
    let signature = kp.sign(message);

    // Verify the signature against hashed message.
    assert!(kp
        .public()
        .verify_hashed(
            <sha3::Keccak256 as sha3::digest::Digest>::digest(message).as_slice(),
            &signature
        )
        .is_ok());
}

fn signature_test_inputs() -> (Vec<u8>, Vec<Secp256k1PublicKey>, Vec<Secp256k1Signature>) {
    // Make signatures.
    let message: &[u8] = b"Hello, world!";
    let digest = message.digest();
    let (pubkeys, signatures): (Vec<Secp256k1PublicKey>, Vec<Secp256k1Signature>) = keys()
        .into_iter()
        .take(3)
        .map(|kp| {
            let sig = kp.sign(&digest.0);
            (kp.public().clone(), sig)
        })
        .unzip();

    (digest.to_vec(), pubkeys, signatures)
}

#[test]
fn verify_valid_batch() {
    let (digest, pubkeys, signatures) = signature_test_inputs();

    let res = Secp256k1PublicKey::verify_batch_empty_fail(&digest[..], &pubkeys, &signatures);
    assert!(res.is_ok(), "{:?}", res);
}

#[test]
fn verify_invalid_batch() {
    let (digest, pubkeys, mut signatures) = signature_test_inputs();
    // mangle one signature
    signatures[0] = Secp256k1Signature::default();

    let res = Secp256k1PublicKey::verify_batch_empty_fail(&digest, &pubkeys, &signatures);
    assert!(res.is_err(), "{:?}", res);
}

#[test]
fn verify_empty_batch() {
    let (digest, _, _) = signature_test_inputs();

    let res = Secp256k1PublicKey::verify_batch_empty_fail(&digest[..], &[], &[]);
    assert!(res.is_err(), "{:?}", res);
}

#[test]
fn verify_batch_missing_public_keys() {
    let (digest, pubkeys, signatures) = signature_test_inputs();

    // missing leading public keys
    let res = Secp256k1PublicKey::verify_batch_empty_fail(&digest, &pubkeys[1..], &signatures);
    assert!(res.is_err(), "{:?}", res);

    // missing trailing public keys
    let res = Secp256k1PublicKey::verify_batch_empty_fail(
        &digest,
        &pubkeys[..pubkeys.len() - 1],
        &signatures,
    );
    assert!(res.is_err(), "{:?}", res);
}

#[test]
fn verify_hashed_failed_if_message_unhashed() {
    // Get a keypair.
    let kp = keys().pop().unwrap();

    // Sign over raw message (hashed to keccak256 internally).
    let message: &[u8] = &[0u8; 1];
    let signature = kp.sign(message);

    // Verify the signature against unhashed msg fails.
    assert!(kp.public().verify_hashed(message, &signature).is_err());
}

#[test]
fn verify_invalid_signature() {
    // Get a keypair.
    let kp = keys().pop().unwrap();

    // Make signature.
    let message: &[u8] = b"Hello, world!";
    let digest = message.digest();

    // Verify the signature against good digest passes.
    let signature = kp.sign(&digest.0);
    assert!(kp.public().verify(&digest.0, &signature).is_ok());

    // Verify the signature against bad digest fails.
    let bad_message: &[u8] = b"Bad message!";
    let digest = bad_message.digest();

    assert!(kp.public().verify(&digest.0, &signature).is_err());
}

#[tokio::test]
async fn signature_service() {
    // Get a keypair.
    let kp = keys().pop().unwrap();
    let pk = kp.public().clone();

    // Spawn the signature service.
    let mut service = SignatureService::new(kp);

    // Request signature from the service.
    let message: &[u8] = b"Hello, world!";
    let digest = message.digest();
    let signature = service.request_signature(digest).await;

    // Verify the signature we received.
    assert!(pk.verify(digest.as_ref(), &signature).is_ok());
}

#[test]
fn test_sk_zeroization_on_drop() {
    let ptr: *const u8;
    let bytes_ptr: *const u8;

    let mut sk_bytes = Vec::new();

    {
        let mut rng = StdRng::from_seed([9; 32]);
        let kp = Secp256k1KeyPair::generate(&mut rng);
        let sk = kp.private();
        sk_bytes.extend_from_slice(sk.as_ref());

        ptr = std::ptr::addr_of!(sk.privkey) as *const u8;
        bytes_ptr = &sk.as_ref()[0] as *const u8;

        let sk_memory: &[u8] =
            unsafe { ::std::slice::from_raw_parts(bytes_ptr, constants::SECRET_KEY_SIZE) };
        // Assert that this is equal to sk_bytes before deletion
        assert_eq!(sk_memory, &sk_bytes[..]);
    }

    // Check that self.privkey is set to ONE_KEY (workaround to all zero SecretKey considered as invalid)
    unsafe {
        for i in 0..constants::SECRET_KEY_SIZE - 1 {
            assert!(*ptr.add(i) == 0);
        }
        assert!(*ptr.add(constants::SECRET_KEY_SIZE - 1) == 1);
    }

    // Check that self.bytes is zeroized
    let sk_memory: &[u8] =
        unsafe { ::std::slice::from_raw_parts(bytes_ptr, constants::SECRET_KEY_SIZE) };
    assert_ne!(sk_memory, &sk_bytes[..]);
}

use proptest::arbitrary::Arbitrary;
use wycheproof::TestResult;

proptest::proptest! {
    #[test]
    fn test_k256_against_secp256k1_lib_with_recovery(
        r in <[u8; 32]>::arbitrary()
) {
        let message: &[u8] = b"hello world!";
        let hashed_msg = rust_secp256k1::Message::from_slice(<sha3::Keccak256 as sha3::digest::Digest>::digest(message).as_slice()).unwrap();

        // contruct private key with bytes and signs message
        let priv_key = <Secp256k1PrivateKey as ToFromBytes>::from_bytes(&r).unwrap();
        let key_pair = Secp256k1KeyPair::from(priv_key);
        let key_pair_copied = key_pair.copy();
        let key_pair_copied_2 = key_pair.copy();
        let signature: Secp256k1Signature = key_pair.sign(message);
        assert!(key_pair.public().verify(message, &signature).is_ok());

        // construct a signature with r, s, v where v is flipped from the original signature.
        let bytes = ToFromBytes::as_bytes(&signature);
        let mut flipped_bytes = [0u8; 65];
        flipped_bytes[..64].copy_from_slice(&bytes[..64]);
        if bytes[64] == 0 {
            flipped_bytes[64] = 1;
        } else {
            flipped_bytes[64] = 0;
        }
        let malleated_signature: Secp256k1Signature = <Secp256k1Signature as signature::Signature>::from_bytes(&flipped_bytes).unwrap();

        // malleated signature with opposite sign fails to verify
        assert!(key_pair.public().verify(message, &malleated_signature).is_err());

        // use k256 to construct private key with the same bytes and signs the same message
        let priv_key_1 = k256::ecdsa::SigningKey::from_bytes(&r).unwrap();
        let pub_key_1 = priv_key_1.verifying_key();
        let signature_1: k256::ecdsa::recoverable::Signature = priv_key_1.sign(message);
        assert!(pub_key_1.verify(message, &signature_1).is_ok());

        // two private keys are serialized the same
        assert_eq!(key_pair_copied.private().as_bytes(), priv_key_1.to_bytes().as_slice());

        // two pubkeys are the same
        assert_eq!(
            key_pair.public().as_bytes(),
            pub_key_1.to_bytes().as_slice()
        );

        // same recovered pubkey are recovered
        let recovered_key = signature.sig.recover(&hashed_msg).unwrap();
        let recovered_key_1 = signature_1.recover_verifying_key(message).expect("couldn't recover pubkey");
        assert_eq!(recovered_key.serialize(),recovered_key_1.to_bytes().as_slice());

        // same signatures produced from both implementations
        assert_eq!(signature.as_ref(), ToFromBytes::as_bytes(&signature_1));

        // use ffi-implemented keypair to verify sig constructed by k256
        let sig_bytes_1 = bincode::serialize(&signature_1.as_ref()).unwrap();
        let secp_sig1 = bincode::deserialize::<Secp256k1Signature>(&sig_bytes_1).unwrap();
        assert!(key_pair_copied_2.public().verify(message, &secp_sig1).is_ok());

        // use k256 keypair to verify sig constructed by ffi-implementation
        let typed_sig = k256::ecdsa::recoverable::Signature::try_from(signature.as_ref()).unwrap();
        assert!(pub_key_1.verify(message, &typed_sig).is_ok());
    }
}

#[test]
fn wycheproof_test() {
    let test_set = TestSet::load(EcdsaSecp256k1Sha256).unwrap();
    for test_group in test_set.test_groups {
        let pk = Secp256k1PublicKey::from_uncompressed(&test_group.key.key);
        for test in test_group.tests {
            let bytes = match Signature::from_der(&test.sig) {
                Ok(s) => s.serialize_compact(),
                Err(_) => {
                    assert!(test.result == wycheproof::TestResult::Invalid);
                    continue;
                }
            };

            // Wycheproof tests do not provide a recovery id, iterate over all possible ones to verify.
            let mut n_bytes = [0u8; 65];
            n_bytes[..64].copy_from_slice(&bytes[..]);
            let mut res = TestResult::Invalid;

            for i in 0..4 {
                n_bytes[64] = i;
                let sig = <Secp256k1Signature as ToFromBytes>::from_bytes(&n_bytes).unwrap();
                if pk
                    .verify_hashed(&k256::sha2::Sha256::digest(&test.msg), &sig)
                    .is_ok()
                {
                    res = TestResult::Valid;
                    break;
                } else {
                    continue;
                }
            }
            assert_eq!(map_result(test.result), res);
        }
    }
}

fn map_result(t: TestResult) -> TestResult {
    match t {
        TestResult::Valid => TestResult::Valid,
        _ => TestResult::Invalid, // Treat Acceptable as Invalid
    }
}
