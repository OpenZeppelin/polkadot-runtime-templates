//! PolkaVM Precompile
// TODO: downgrade to current version of pallet-revive in deps

use alloc::vec::Vec;
use core::{fmt, marker::PhantomData, num::NonZero};

use pallet_revive::{
    precompiles::{
        alloy::{self, primitives::Address as AlloyAddress, sol_types::SolValue},
        AddressMatcher, Error, Ext, Precompile,
    },
    DispatchInfo, Origin,
};
use sp_core::H160;
use tracing::error;

use crate::{pallet, AssetId, Balance, Cipher, Config as PalletConfig, RequestId, WeightInfo};

const LOG_TARGET: &str = "confidential::precompiles";

// --- ABI ---------------------------------------------------------------------

// Put the interface file at: src/precompiles/IConfidentialAssets.sol
alloy::sol!("IConfidentialAssets.sol");
use IConfidentialAssets::IConfidentialAssetsCalls;

// --- Helpers -----------------------------------------------------------------

fn revert(error: &impl fmt::Debug, message: &str) -> Error {
    error!(target: LOG_TARGET, ?error, "{}", message);
    Error::Revert(message.into())
}

/// Convert an EVM/PolkaVM 20-byte address (H160) into the runtime AccountId.
fn h160_to_account<Runtime>(
    addr: &AlloyAddress,
) -> Result<<Runtime as frame_system::Config>::AccountId, Error>
where
    Runtime: pallet_revive::Config + frame_system::Config,
{
    let h160 = H160(addr.0);
    let acc = <Runtime as pallet_revive::Config>::AddressMapping::into_account_id(h160);
    Ok(acc)
}

/// Light guard ensuring bytes exist when required.
fn ensure_non_empty(data: &[u8], what: &str) -> Result<(), Error> {
    if data.is_empty() {
        return Err(Error::Revert(format!("{what} must not be empty").into()));
    }
    Ok(())
}

// --- Precompile --------------------------------------------------------------

pub struct ConfidentialPrecompile<T>(PhantomData<T>);

impl<Runtime> Precompile for ConfidentialPrecompile<Runtime>
where
    Runtime: pallet_revive::Config + PalletConfig + frame_system::Config,
{
    type Interface = IConfidentialAssets::IConfidentialAssetsCalls;
    type T = Runtime;

    const HAS_CONTRACT_INFO: bool = false;
    // Pick a fixed 20-byte address for this precompile (example: 0x...11).
    // Ensure it does not collide with your other precompiles.
    const MATCHER: AddressMatcher = AddressMatcher::Fixed(NonZero::new(0x11).unwrap());

    fn call(
        _address: &[u8; 20],
        input: &Self::Interface,
        env: &mut impl Ext<T = Self::T>,
    ) -> Result<Vec<u8>, Error> {
        // Map PolkaVM caller into a FRAME origin
        let origin = env.caller();
        let frame_origin = match &origin {
            Origin::Root => frame_system::RawOrigin::Root.into(),
            Origin::Signed(account_id) =>
                frame_system::RawOrigin::Signed(account_id.clone()).into(),
        };

        match input {
            // -----------------------------------------------------------------
            // setOperator(asset, operator, until)
            // holder := msg.sender
            // -----------------------------------------------------------------
            IConfidentialAssetsCalls::setOperator(IConfidentialAssets::setOperatorCall {
                asset,
                operator,
                until,
            }) => {
                let _ = env.charge(<Runtime as PalletConfig>::WeightInfo::set_operator())?;

                // holder == Signed caller only (Root wouldn’t make sense here)
                let holder = match origin {
                    Origin::Signed(ref who) => who.clone(),
                    Origin::Root =>
                        return Err(Error::Revert(
                            "setOperator must be called by a signed account".into(),
                        )),
                };

                let operator_acc = h160_to_account::<Runtime>(operator)?;

                pallet::Pallet::<Runtime>::set_operator(
                    frame_origin,
                    *asset as AssetId,
                    operator_acc,
                    (*until).into(),
                )
                .map(|_| Vec::new())
                .map_err(|e| revert(&e, "setOperator failed"))
            }

            // -----------------------------------------------------------------
            // confidentialTransfer(asset, to, amount)
            // from := msg.sender
            // amount := ciphertext bytes
            // -----------------------------------------------------------------
            IConfidentialAssetsCalls::confidentialTransfer(
                IConfidentialAssets::confidentialTransferCall { asset, to, amount },
            ) => {
                let _ =
                    env.charge(<Runtime as PalletConfig>::WeightInfo::confidential_transfer())?;

                ensure_non_empty(amount, "amount (cipher)")?;
                let to_acc = h160_to_account::<Runtime>(to)?;
                let cipher: Cipher = amount.as_slice().try_into()
                    .map_err(|_| Error::Revert("Cipher must be exactly 32 bytes".into()))?;

                pallet::Pallet::<Runtime>::confidential_transfer(
                    frame_origin,
                    *asset as AssetId,
                    to_acc,
                    cipher,
                )
                .map(|_| Vec::new())
                .map_err(|e| revert(&e, "confidentialTransfer failed"))
            }

            // -----------------------------------------------------------------
            // confidentialMint(asset, to, amount)
            // NOTE: Pallet requires Root origin.
            // -----------------------------------------------------------------
            IConfidentialAssetsCalls::confidentialMint(IConfidentialAssets::confidentialMintCall {
                asset,
                to,
                amount,
            }) => {
                let _ =
                    env.charge(<Runtime as PalletConfig>::WeightInfo::confidential_transfer())?;

                ensure_non_empty(amount, "amount (cipher)")?;
                let to_acc = h160_to_account::<Runtime>(to)?;
                let cipher: Cipher = amount.as_slice().try_into()
                    .map_err(|_| Error::Revert("Cipher must be exactly 32 bytes".into()))?;

                pallet::Pallet::<Runtime>::confidential_mint(
                    frame_origin,
                    *asset as AssetId,
                    to_acc,
                    cipher,
                )
                .map(|_| Vec::new())
                .map_err(|e| revert(&e, "confidentialMint failed (requires Root)"))
            }

            // -----------------------------------------------------------------
            // confidentialBurn(asset, from, amount)
            // NOTE: Pallet requires Root origin.
            // -----------------------------------------------------------------
            IConfidentialAssetsCalls::confidentialBurn(IConfidentialAssets::confidentialBurnCall {
                asset,
                from,
                amount,
            }) => {
                let _ =
                    env.charge(<Runtime as PalletConfig>::WeightInfo::confidential_transfer())?;

                ensure_non_empty(amount, "amount (cipher)")?;
                let from_acc = h160_to_account::<Runtime>(from)?;
                let cipher: Cipher = amount.as_slice().try_into()
                    .map_err(|_| Error::Revert("Cipher must be exactly 32 bytes".into()))?;

                pallet::Pallet::<Runtime>::confidential_burn(
                    frame_origin,
                    *asset as AssetId,
                    from_acc,
                    cipher,
                )
                .map(|_| Vec::new())
                .map_err(|e| revert(&e, "confidentialBurn failed (requires Root)"))
            }

            // -----------------------------------------------------------------
            // requestDecryption(encrypted)
            // Stores the request, bumps counter, kicks RuntimeFhe::request_decryption
            // -----------------------------------------------------------------
            IConfidentialAssetsCalls::requestDecryption(IConfidentialAssets::requestDecryptionCall {
                encrypted,
            }) => {
                let _ = env.charge(<Runtime as PalletConfig>::WeightInfo::request_decryption())?;

                ensure_non_empty(encrypted, "encrypted (cipher)")?;
                let cipher: Cipher = encrypted.as_slice().try_into()
                    .map_err(|_| Error::Revert("Cipher must be exactly 32 bytes".into()))?;

                // We let the pallet manage request id & event emission.
                pallet::Pallet::<Runtime>::request_decryption(
                    frame_origin,
                    cipher,
                )
                .map(|_| Vec::new())
                .map_err(|e| revert(&e, "requestDecryption failed"))
            }

            // -----------------------------------------------------------------
            // decrypt(request, amount, proof)
            // Checks signatures via RuntimeFhe::check_signatures, emits AmountDisclosed
            // -----------------------------------------------------------------
            IConfidentialAssetsCalls::decrypt(IConfidentialAssets::decryptCall {
                request,
                amount,
                proof,
            }) => {
                let pre = DispatchInfo {
                    call_weight: <Runtime as PalletConfig>::WeightInfo::decrypt(),
                    extension_weight: Default::default(),
                    ..Default::default()
                };

                let charged = env.charge(pre.call_weight)?;

                ensure_non_empty(proof, "proof (cipher)")?;

                let proof_cipher: Cipher = proof.as_slice().try_into()
                    .map_err(|_| Error::Revert("Cipher must be exactly 32 bytes".into()))?;

                let result = pallet::Pallet::<Runtime>::decrypt(
                    frame_origin,
                    *request as RequestId,
                    *amount as Balance,
                    proof_cipher,
                );

                // If you want to adjust gas to actual weight (if you later add dynamic weights),
                // extract actual weight like in your XCM precompile:
                // let actual = frame_support::dispatch::extract_actual_weight(&result, &pre);
                // env.adjust_gas(charged, actual);
                // For now, your weights are flat, so we can just keep the charged amount.

                result.map(|_| Vec::new()).map_err(|e| revert(&e, "decrypt failed"))
            }
        }
    }
}
