#![cfg(test)]

use crate as pallet;
use ca_primitives::ZkVerifier as ZkVerifierTrait;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT as G, ristretto::RistrettoPoint, scalar::Scalar,
};
use frame_support::{construct_runtime, derive_impl, parameter_types};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;

// Prover (std-only) used in tests.
use zk_elgamal_prover::{
    prove_receiver_accept, prove_sender_transfer, ReceiverAcceptInput, SenderInput,
};

pub type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        ZkVerifier: pallet,
    }
);

parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights =
        frame_system::limits::BlockWeights::simple_max(
            frame_support::weights::Weight::from_parts(1024, u64::MAX),
        );
    pub static ExistentialDeposit: u64 = 1;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

impl pallet::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    // Use the real Bulletproofs verifier provided by the user (no dummies).
    type RangeVerifier = zk_elgamal_verifier::BulletproofRangeVerifier;
}

pub fn new_test_ext() -> TestExternalities {
    let storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("valid default genesis storage");
    TestExternalities::from(storage)
}

// ------------------------- Helpers (tests-only) -------------------------

fn pedersen_h_generator() -> RistrettoPoint {
    use sha2::Sha512;
    RistrettoPoint::hash_from_bytes::<Sha512>(b"Zether/PedersenH")
}

fn compress(pt: &RistrettoPoint) -> [u8; 32] {
    pt.compress().to_bytes()
}

#[test]
fn builds_runtime_successfully() {
    new_test_ext().execute_with(|| {
        assert_eq!(1 + 1, 2);
    });
}

#[test]
fn verify_sender_and_receiver_happy_path() {
    new_test_ext().execute_with(|| {
        // ----------------- Keys & openings -----------------
        let sk_sender = Scalar::from(5u64);
        let pk_sender = sk_sender * G;
        let pk_receiver = Scalar::from(9u64) * G;

        let h = pedersen_h_generator();

        // Sender starts with a positive balance
        let from_old_v = 1_234u64;
        let from_old_r = Scalar::from(42u64);
        let from_old_C = Scalar::from(from_old_v) * G + from_old_r * h;

        // Receiver starts at zero (identity commitment)
        let to_old_v = 0u64;
        let to_old_r = Scalar::from(0u64);
        let to_old_C = RistrettoPoint::default(); // identity

        // Transfer amount
        let dv = 111u64;

        // IMPORTANT: The verifier currently fixes `network_id = [0;32]`.
        // So we pass the same `[0;32]` here to the prover to match transcript binding.
        let network_id = [0u8; 32];
        let asset_id = b"TEST_ASSET".to_vec();

        // Deterministic seed so sender and receiver agree on witnesses for the test
        let mut seed = [0u8; 32];
        seed[0] = 7;

        // ----------------- Sender phase (prove) -----------------
        let s_in = SenderInput {
            asset_id: asset_id.clone(),
            network_id,
            sender_pk: pk_sender,
            receiver_pk: pk_receiver,
            from_old_C,
            from_old_opening: (from_old_v, from_old_r),
            to_old_C,
            delta_value: dv,
            rng_seed: seed,
            fee_C: None,
        };

        let s_out = prove_sender_transfer(&s_in).expect("sender prover should succeed");

        // Quick shape sanity
        assert_eq!(s_out.delta_ct_bytes.len(), 64);
        // bundle = 32 + 192 + 2 + range_from_len + 2
        assert!(s_out.sender_bundle_bytes.len() >= 32 + 192 + 2 + 1 + 2);

        // ----------------- Sender phase (verify) -----------------
        let (from_new_bytes_v, to_new_bytes_v) =
            <pallet::Pallet<Test> as ZkVerifierTrait>::verify_transfer_sent(
                &asset_id,
                &compress(&pk_sender),
                &compress(&pk_receiver),
                &compress(&from_old_C),
                &compress(&to_old_C),
                &s_out.delta_ct_bytes,
                &s_out.sender_bundle_bytes,
            )
            .expect("sender-side verification did not pass");

        // Check verifier matches prover's expected commitments
        assert_eq!(from_new_bytes_v.as_slice(), &s_out.from_new_C);
        assert_eq!(to_new_bytes_v.as_slice(), &s_out.to_new_C);

        // ----------------- Receiver phase (prove) -----------------
        // For a KISS test, reuse the same seed to derive Δρ (see Prover test note).
        use rand::{RngCore, SeedableRng};
        use rand_chacha::ChaCha20Rng;

        let delta_rho = Scalar::from(ChaCha20Rng::from_seed(seed).next_u64());

        // Deserialize ΔC from sender output
        let delta_comm = {
            use curve25519_dalek::ristretto::CompressedRistretto;
            CompressedRistretto(s_out.delta_comm_bytes)
                .decompress()
                .expect("valid ΔC")
        };

        let r_in = ReceiverAcceptInput {
            asset_id: asset_id.clone(),
            network_id,
            receiver_pk: pk_receiver,
            to_old_C, // same as sender used
            to_old_opening: (to_old_v, to_old_r),
            delta_comm,
            delta_value: dv,
            delta_rho,
        };

        let r_out = prove_receiver_accept(&r_in).expect("receiver prover should succeed");
        assert!(r_out.accept_envelope.len() > 34); // 32 + 2 + >=1

        // ----------------- Receiver phase (verify) -----------------
        let to_new_bytes_recv_v =
            <pallet::Pallet<Test> as ZkVerifierTrait>::verify_transfer_received(
                &asset_id,
                &compress(&pk_receiver),
                &compress(&to_old_C),
                &r_out.accept_envelope,
            )
            .expect("receiver acceptance did not verify");

        // Should match the prover's to_new commitment from sender computation / receiver recompute
        assert_eq!(to_new_bytes_recv_v.as_slice(), &r_out.to_new_C);
    });
}

#[test]
fn rejects_tampered_sender_bundle() {
    new_test_ext().execute_with(|| {
        // Setup identical to the happy path, then flip one byte in the link proof section.
        let sk_sender = Scalar::from(5u64);
        let pk_sender = sk_sender * G;
        let pk_receiver = Scalar::from(9u64) * G;

        let h = pedersen_h_generator();

        let from_old_v = 555u64;
        let from_old_r = Scalar::from(7u64);
        let from_old_C = Scalar::from(from_old_v) * G + from_old_r * h;

        let to_old_C = RistrettoPoint::default();

        let dv = 10u64;
        let network_id = [0u8; 32];
        let asset_id = b"TEST_ASSET".to_vec();

        let mut seed = [0u8; 32];
        seed[1] = 99;

        let s_in = SenderInput {
            asset_id: asset_id.clone(),
            network_id,
            sender_pk: pk_sender,
            receiver_pk: pk_receiver,
            from_old_C,
            from_old_opening: (from_old_v, from_old_r),
            to_old_C,
            delta_value: dv,
            rng_seed: seed,
            fee_C: None,
        };

        let mut s_out = prove_sender_transfer(&s_in).expect("sender prover should succeed");

        // Tamper a byte inside the link proof region of the bundle:
        // bundle layout: [0..32)=ΔC | [32..224)=link(192) | [224..]=ranges...
        if s_out.sender_bundle_bytes.len() >= 33 {
            s_out.sender_bundle_bytes[32 + 10] ^= 0x01;
        }

        let err = <pallet::Pallet<Test> as ZkVerifierTrait>::verify_transfer_sent(
            &asset_id,
            &compress(&pk_sender),
            &compress(&pk_receiver),
            &compress(&from_old_C),
            &compress(&to_old_C),
            &s_out.delta_ct_bytes,
            &s_out.sender_bundle_bytes,
        );

        assert!(err.is_err(), "tampered sender bundle must be rejected");
    });
}
