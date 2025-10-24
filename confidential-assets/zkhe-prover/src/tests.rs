use crate::*;

#[test]
fn sender_receiver_round_trip_shapes() {
    let mut seed = [0u8; 32];
    seed[0] = 7;

    let sk_sender = Scalar::from(5u64);
    let pk_sender = sk_sender * G;
    let pk_receiver = Scalar::from(9u64) * G;

    let h = pedersen_h_generator();
    let from_old_v = 1234u64;
    let from_old_r = Scalar::from(42u64);
    let from_old_c = Scalar::from(from_old_v) * G + from_old_r * h;

    let to_old_v = 0u64;
    let to_old_r = Scalar::from(0u64);
    let to_old_c = RistrettoPoint::identity();

    let dv = 111u64;

    let s_in = SenderInput {
        asset_id: b"TEST_ASSET".to_vec(),
        network_id: [1u8; 32],
        sender_pk: pk_sender,
        receiver_pk: pk_receiver,
        from_old_c,
        from_old_opening: (from_old_v, from_old_r),
        to_old_c,
        delta_value: dv,
        rng_seed: seed,
        fee_c: None,
    };
    let s_out = prove_sender_transfer(&s_in).expect("sender prove");

    assert_eq!(s_out.delta_ct_bytes.len(), 64);
    assert!(s_out.sender_bundle_bytes.len() >= 32 + 192 + 2 + 1 + 2);

    // Receiver acceptance with known witnesses (KISS mode)
    let r_in = ReceiverAcceptInput {
        asset_id: b"TEST_ASSET".to_vec(),
        network_id: [1u8; 32],
        receiver_pk: pk_receiver,
        to_old_c,
        to_old_opening: (to_old_v, to_old_r),
        delta_comm: {
            use curve25519_dalek::ristretto::CompressedRistretto;
            CompressedRistretto(s_out.delta_comm_bytes)
                .decompress()
                .unwrap()
        },
        delta_value: dv,
        delta_rho: {
            // Not exported from sender routine; in real usage receiver derives it.
            // For this smoke test we just reuse seed to simulate consistent rho.
            Scalar::from(ChaCha20Rng::from_seed(seed).next_u64())
        },
    };

    let r_out = prove_receiver_accept(&r_in).expect("receiver accept");
    assert!(r_out.accept_envelope.len() > 32 + 2);
}
