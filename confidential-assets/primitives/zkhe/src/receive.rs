//! Accept-envelope bytes + transcript binding (shared by prover & verifier)
//! Layout (Option A):
//!   accept_envelope := delta_comm(32)
//!                      || len1(u16 LE) || range_avail_new(len1)
//!                      || len2(u16 LE) || range_pending_new(len2)
//!
//! Where:
//! - delta_comm is the 32B compressed Ristretto commitment for ΔC (Σ chosen UTXOs)
//! - range_* are Bulletproof single-value range proofs (opaque bytes here)
//!
//! Transcript binding for receiver-accept (must match on both sides):
//!   ctx = accept_ctx_bytes(network_id, asset_id, receiver_pk, to_old_commit, delta_comm)
//!   Each range proof must bind to:
//!     transcript label: b"bp64"
//!     messages: ("ctx", ctx) and ("commit", new_commit_bytes)
//!
//! Invariants the verifier will enforce (math lives in verifier):
//! - Sum(pending_commits) == delta_comm
//! - new_available = old_available + delta_comm
//! - new_pending   = old_pending   - delta_comm
//! - range(new_available), range(new_pending)

#![cfg_attr(not(feature = "std"), no_std)]

pub mod accept_envelope {
    use core::convert::TryInto;
    use merlin::Transcript;

    // --- Stable labels (match your existing `labels` module) ---
    pub const PROTOCOL: &[u8]   = b"zk-elgamal-conf-xfer";
    pub const PROTOCOL_V: &[u8] = b"zk-elgamal-conf-xfer/v1";

    /// Canonical 32-byte compressed Ristretto commitment
    pub type Commitment = [u8; 32];

    /// Parsed accept envelope (borrowed slices, zero-copy).
    /// - `delta_comm`: the ΔC commitment (32B)
    /// - `rp_avail_new`: Bulletproof bytes proving range of new available
    /// - `rp_pending_new`: Bulletproof bytes proving range of new pending
    #[derive(Debug, Clone, Copy)]
    pub struct AcceptEnvelope<'a> {
        pub delta_comm: Commitment,
        pub rp_avail_new: &'a [u8],
        pub rp_pending_new: &'a [u8],
    }

    impl<'a> AcceptEnvelope<'a> {
        /// Parse: delta_comm(32) || len1(u16) || rp1 || len2(u16) || rp2
        pub fn parse(bytes: &'a [u8]) -> Result<Self, ()> {
            if bytes.len() < 32 + 2 + 2 { return Err(()); }
            let delta_comm: Commitment = bytes[0..32].try_into().map_err(|_| ())?;

            // rp_avail_new
            let (len1, off1) = read_u16_le(bytes, 32)?;
            let end1 = off1.checked_add(len1).ok_or(())?;
            if end1 > bytes.len() { return Err(()); }
            let rp1 = &bytes[off1..end1];

            // rp_pending_new
            let (len2, off2) = read_u16_le(bytes, end1)?;
            let end2 = off2.checked_add(len2).ok_or(())?;
            if end2 != bytes.len() { return Err(()); } // must consume all
            let rp2 = &bytes[off2..end2];

            Ok(Self { delta_comm, rp_avail_new: rp1, rp_pending_new: rp2 })
        }
    }

    /// Compute the 32B “context tag” used to bind receiver-accept proofs.
    /// This must be byte-for-byte identical between prover and verifier.
    ///
    /// Binding order:
    ///   proto label -> version -> network_id(32) -> asset_id(32)
    ///   -> receiver_pk (compressed 32B) -> to_old_commit(32) -> delta_comm(32)
    ///
    /// Note: `receiver_pk_compressed` must be the 32-byte compressed point.
    pub fn accept_ctx_bytes(
        network_id: [u8; 32],
        asset_id:   [u8; 32],
        receiver_pk_compressed: [u8; 32],
        to_old_commit: [u8; 32],   // 0s for identity
        delta_comm:    [u8; 32],
    ) -> [u8; 32] {
        let mut t = Transcript::new(PROTOCOL);
        t.append_message(b"proto", PROTOCOL_V);
        t.append_message(b"sdk_version", &1u32.to_le_bytes()); // keep in sync with SDK_VERSION
        t.append_message(b"network_id", &network_id);
        t.append_message(b"asset_id",   &asset_id);
        t.append_message(b"receiver_pk", &receiver_pk_compressed);
        t.append_message(b"to_old",      &to_old_commit);
        t.append_message(b"delta_comm",  &delta_comm);

        let mut out = [0u8; 32];
        t.challenge_bytes(b"ctx", &mut out);
        out
    }

    /// Tiny LE u16 reader returning (len, next_offset).
    #[inline]
    fn read_u16_le(buf: &[u8], offset: usize) -> Result<(usize, usize), ()> {
        let next = offset.checked_add(2).ok_or(())?;
        if next > buf.len() { return Err(()); }
        let len = u16::from_le_bytes(buf[offset..next].try_into().map_err(|_| ())?) as usize;
        Ok((len, next))
    }
}
