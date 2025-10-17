# Why replace DoubleMap with Child Tries?

Child tries per asset maps perfectly to MimbeWimble's per-asset conservation and cut-through by providing per asset isolation.

In MimbeWimble you still end up with an unspent set of commitments. With multiple assets, you want conservation and cut-through to hold per asset. A child trie per AssetId:
- scopes reads/writes and proofs to that asset (cheaper hashing paths)
- aligns with MW cut-through (you only cut-through within the same asset anyway)
- gives you cheap pruning/snapshot of a single asset’s UTXO set
- enables light-client inclusion proofs “UTXO X for Asset A exists” without mixing other assets

State bloat control. Verify heavy proofs during the extrinsic, but do not store them. After verification, you only persist the minimal UTXO record (commitment + tiny metadata) in the child trie. This keeps state small.