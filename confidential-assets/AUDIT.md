# Notes for Reviewers

## Ready for Review:

1. Following crates should be reviewed closely:
- `zkhe-verifier`: curve type defs, expected proof format, verification algorithm
- `zkhe-prover`: proof generation matches verifier expectations

Code blocks worth reviewing closely:
- `zkhe-verifier/src/range_verifier.rs`: custom `no_std` `bulletproof::verify` implementation for a range proof that uses the proof transcript to generate a deterministic RNG
- `verify_transfer_and_apply` function in `zkhe-verifier/src/lib.rs`: verifies transfer logic before applying it (and uses the custom `no_std` `bulletproof::verify` function internally for range proof verification) 

2. There are two UX design hurdles that exist in the Solana CT implementation. I have proposed a solution for each in the following internal docs: [(1) ZK El Gamal Cryptography Requires Proof from Receiver to Update Available Balance On-Chain](https://www.notion.so/openzeppelin/Solana-Confidential-Token-UX-Hurdle-297cbd12786080b3a1bbccdd48f8dfc1?v=3d927d6f3c704a9abfe817c2e0490f95&source=copy_link), [(2) Solana CT solution to (1) is too easily griefed by incoming transfers (trivial attack on liquidity)](https://www.notion.so/openzeppelin/Solana-Confidential-Token-UX-Hurdle-accept_pending-design-in-Solana-CT-is-too-easily-griefed-298cbd12786080ddb1b1eaff9bd050b8?v=3d927d6f3c704a9abfe817c2e0490f95&source=copy_link)

## WIP NOT Ready for Review

The pallet should also be reviewed closely but it is undergoing refactoring:
- pallet-zkhe: authenticated and verified storage for zkhe processing

### Regression Testing and Verification

Solana’s ZK ElGamal proof system for its Token-2022 “Confidential Transfer” extension had a flaw in its Fiat–Shamir transcript hashing which could let invalid proofs pass verification. To make sure we don't make the same mistake, review and complete the following checklist prior to release:

#### A. Transcript & Fiat–Shamir discipline  
- **Include all public inputs** (commitments, ciphertexts, account IDs, fees, versions) in the Fiat–Shamir transcript.  
  - Root cause in Solana: some algebraic components were omitted from the transcript. ([AICoin summary](https://www.aicoin.com/en/article/457871?utm_source=chatgpt.com))  
- **Domain separation:** include protocol/curve/version tags in the transcript (e.g., `b"Substrate-ZK-ElGamal-v1-Ristretto255"`).  
- **No transcript reuse** across proofs (each proof derives its own challenge).  
- Document explicitly which fields are hashed so reviewers can verify coverage.

#### B. Curve & group usage (“curve choice” hygiene)  
- Use **Ristretto255** or another **prime-order group** to avoid cofactor issues.  
- Separate **account keys** (e.g., ed25519) from **proof group keys** (Ristretto); never mix or reinterpret.  
- Canonically decode points and **reject low-order or invalid points**.  
- Bind any generator/parameter choices into the transcript with versioning.

#### C. Proof system structure, fees & range checks  
- **Bind fee policies and fee commitments** into the transcript to prevent fee-bypass bugs. ([Blockworks article](https://blockworks.co/news/solana-bug-patch-zero-knowledge-proofs?utm_source=chatgpt.com))  
- Implement and bind **range proofs** for non-negativity and bit-length correctness. ([Solana docs](https://solana.com/docs/tokens/extensions/confidential-transfer/transfer-tokens?utm_source=chatgpt.com))  
- Include **equality proofs** for commitment consistency (`old_balance - amount - fee = new_balance`).  
- Verify **ciphertext validity proofs** for all participants (sender, receiver, auditor).  
- Audit application logic to ensure no bypass of proof verification or mint logic.

#### D. Implementation hardening  
- Use **constant-time crypto operations**; avoid side-channels or timing leaks. ([QuickNode guide](https://www.quicknode.com/guides/solana-development/spl-tokens/token-2022/confidential?utm_source=chatgpt.com))  
- Fuzz-test decoders for malformed inputs and reject non-canonical encodings.  
- Add a **version field** to all proof objects for future parameter changes.  
- Include a **kill-switch/feature flag** to disable or rotate proof verifiers if a bug is discovered. ([Solana StackExchange Q&A](https://solana.stackexchange.com/questions/22501/zkhe-proof-program-is-temporarily-disabled?utm_source=chatgpt.com))

#### E. Application logic & token rules  
- Enforce token logic (supply limits, authority controls, transferability) even under confidential flows.  
- Emit **auditable commitments or Merkle roots** for public verification of supply conservation.  
- Restrict who can enable confidential transfers; require explicit mint authority approval.  
- Validate **withdrawal flows** (no leaking more funds back to public accounts).

#### F. Testing & verification  
- Build **negative tests** where you omit public data from transcripts—verifier must reject.  
- Perform **fuzz tests** and malformed input tests (proof/ciphertext corruption).  
- Test **branch mixing / OR-proof** scenarios for replay or branch confusion.  
- Optionally add **formal specs or static checks** of your confidential transfer invariant.  
- Plan for an **external audit** and bug-bounty disclosure program. ([Phemex postmortem](https://phemex.com/news/article/solana-foundation-addresses-potential-vulnerability-in-zkhe-proof-program_11257?utm_source=chatgpt.com))  
- Verify **upgrade/migration safety**: old proofs must not verify under new parameters.

#### G. Documentation  
- Clearly document the protocol, transcript format, curve parameters, and versioning rules.  
- Include a **Security assumptions** section (e.g., discrete log hardness, group choice).  
- Maintain a **Known limitations** list (e.g., max range size, dependency on library versions).  
- Provide a **change log** when rotating parameters or updating cryptographic primitives.

#### Links Relevant to the Post Mortem
- **Official Solana Docs:** [Confidential Transfer (Token-2022)](https://solana.com/docs/tokens/extensions/confidential-transfer)  
- **Post-mortems:**  
  - [CoinDesk](https://www.coindesk.com/markets/2025/05/05/solana-quietly-fixes-bug-that-could-have-let-attackers-mint-and-steal-certain-tokens?utm_source=chatgpt.com)  
  - [CoinTelegraph](https://cointelegraph.com/news/solana-devs-validators-fix-critical-bug-criticism-mounts?utm_source=chatgpt.com)  
  - [Phemex](https://phemex.com/news/article/solana-foundation-addresses-potential-vulnerability-in-zkhe-proof-program_11257?utm_source=chatgpt.com)  
- **Developer guides:**  
  - [QuickNode – Confidential Transfers](https://www.quicknode.com/guides/solana-development/spl-tokens/token-2022/confidential?utm_source=chatgpt.com)  
  - [Solana StackExchange – Program Disabled Notice](https://solana.stackexchange.com/questions/22501/zkhe-proof-program-is-temporarily-disabled?utm_source=chatgpt.com)  
- **Audit and analysis:**  
  - [AICoin summary](https://www.aicoin.com/en/article/457871?utm_source=chatgpt.com)  
  - [Blockworks write-up](https://blockworks.co/news/solana-bug-patch-zero-knowledge-proofs?utm_source=chatgpt.com)
