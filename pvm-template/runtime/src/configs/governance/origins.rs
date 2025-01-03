//! Custom origins for governance interventions.

pub use pallet_custom_origins::*;

#[frame_support::pallet]
pub mod pallet_custom_origins {
    use frame_support::pallet_prelude::*;

    use crate::configs::governance::{Balance, CENTS, GRAND};

    #[pallet::config]
    pub trait Config: frame_system::Config {}

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[derive(PartialEq, Eq, Clone, MaxEncodedLen, Encode, Decode, TypeInfo, RuntimeDebug)]
    #[pallet::origin]
    pub enum Origin {
        /// Origin for spending (any amount of) funds.
        Treasurer,
        /// Origin able to cancel referenda.
        ReferendumCanceller,
        /// Origin able to kill referenda.
        ReferendumKiller,
        /// Origin able to spend the amount specified in Spender.
        SmallTipper,
        /// Origin able to spend the amount specified in Spender.
        BigTipper,
        /// Origin able to spend the amount specified in Spender.
        SmallSpender,
        /// Origin able to spend the amount specified in Spender.
        MediumSpender,
        /// Origin able to spend the amount specified in Spender.
        BigSpender,
        /// Origin able to dispatch a whitelisted call.
        WhitelistedCaller,
    }

    macro_rules! decl_unit_ensures {
        ( $name:ident: $success_type:ty = $success:expr ) => {
            pub struct $name;
            impl<O: Into<Result<Origin, O>> + From<Origin>>
                EnsureOrigin<O> for $name
            {
                type Success = $success_type;
                fn try_origin(o: O) -> Result<Self::Success, O> {
                    o.into().and_then(|o| match o {
                        Origin::$name => Ok($success),
                        r => Err(O::from(r)),
                    })
                }
                #[cfg(feature = "runtime-benchmarks")]
                fn try_successful_origin() -> Result<O, ()> {
                    Ok(O::from(Origin::$name))
                }
            }
        };
        ( $name:ident ) => { decl_unit_ensures! { $name : () = () } };
        ( $name:ident: $success_type:ty = $success:expr, $( $rest:tt )* ) => {
            decl_unit_ensures! { $name: $success_type = $success }
            decl_unit_ensures! { $( $rest )* }
        };
        ( $name:ident, $( $rest:tt )* ) => {
            decl_unit_ensures! { $name }
            decl_unit_ensures! { $( $rest )* }
        };
        () => {}
    }
    decl_unit_ensures!(Treasurer, ReferendumCanceller, ReferendumKiller, WhitelistedCaller,);

    macro_rules! decl_ensure {
        (
            $vis:vis type $name:ident: EnsureOrigin<Success = $success_type:ty> {
                $( $item:ident = $success:expr, )*
            }
        ) => {
            $vis struct $name;
            impl<O: Into<Result<Origin, O>> + From<Origin>>
                EnsureOrigin<O> for $name
            {
                type Success = $success_type;
                fn try_origin(o: O) -> Result<Self::Success, O> {
                    o.into().and_then(|o| match o {
                        $(
                            Origin::$item => Ok($success),
                        )*
                        r => Err(O::from(r)),
                    })
                }
                #[cfg(feature = "runtime-benchmarks")]
                fn try_successful_origin() -> Result<O, ()> {
                    // By convention the more privileged origins go later, so for greatest chance
                    // of success, we want the last one.
                    let _result: Result<O, ()> = Err(());
                    $(
                        let _result: Result<O, ()> = Ok(O::from(Origin::$item));
                    )*
                    _result
                }
            }
        }
    }

    decl_ensure! {
        pub type Spender: EnsureOrigin<Success = Balance> {
            SmallTipper = 250 * 3 * CENTS,
            BigTipper = GRAND,
            SmallSpender = 10 * GRAND,
            MediumSpender = 100 * GRAND,
            BigSpender = 1_000 * GRAND,
            Treasurer = 10_000 * GRAND,
        }
    }
}
