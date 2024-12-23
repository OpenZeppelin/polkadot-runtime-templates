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

#[cfg(test)]
mod tests {
    use frame_support::traits::EnsureOrigin;

    use crate::{
        configs::{governance::Spender, pallet_custom_origins::Origin},
        constants::currency::{CENTS, GRAND},
        Balance,
    };

    #[derive(Debug, PartialEq)]
    enum TestOrigin {
        SmallTipper,
        SmallSpender,
        Treasurer,
        BigTipper,
        MediumSpender,
        BigSpender,
        WhitelistedCaller,
        ReferendumCanceller,
        ReferendumKiller,
    }

    impl From<TestOrigin> for Result<Origin, TestOrigin> {
        fn from(value: TestOrigin) -> Self {
            match value {
                TestOrigin::SmallTipper => Ok(Origin::SmallTipper),
                TestOrigin::SmallSpender => Ok(Origin::SmallSpender),
                TestOrigin::Treasurer => Ok(Origin::Treasurer),
                TestOrigin::ReferendumCanceller => Ok(Origin::ReferendumCanceller),
                TestOrigin::ReferendumKiller => Ok(Origin::ReferendumKiller),
                TestOrigin::BigTipper => Ok(Origin::BigTipper),
                TestOrigin::MediumSpender => Ok(Origin::MediumSpender),
                TestOrigin::BigSpender => Ok(Origin::BigSpender),
                TestOrigin::WhitelistedCaller => Ok(Origin::WhitelistedCaller),
            }
        }
    }

    impl From<Origin> for TestOrigin {
        fn from(value: Origin) -> Self {
            match value {
                Origin::SmallTipper => TestOrigin::SmallTipper,
                Origin::SmallSpender => TestOrigin::SmallSpender,
                Origin::Treasurer => TestOrigin::Treasurer,
                Origin::ReferendumCanceller => TestOrigin::ReferendumCanceller,
                Origin::ReferendumKiller => TestOrigin::ReferendumKiller,
                Origin::BigTipper => TestOrigin::BigTipper,
                Origin::MediumSpender => TestOrigin::MediumSpender,
                Origin::BigSpender => TestOrigin::BigSpender,
                Origin::WhitelistedCaller => TestOrigin::WhitelistedCaller,
            }
        }
    }
    mod spender {
        use super::*;

        #[test]
        fn test_small_tipper() {
            let a: Balance =
                Spender::try_origin(TestOrigin::SmallTipper).expect("SmallTipper misconfigured");
            assert_eq!(a, 250 * 3 * CENTS);
        }

        #[test]
        fn test_small_spender() {
            let a: Balance =
                Spender::try_origin(TestOrigin::SmallSpender).expect("SmallSpender misconfigured");
            assert_eq!(a, 10 * GRAND);
        }

        #[test]
        fn test_big_tipper() {
            let a: Balance =
                Spender::try_origin(TestOrigin::BigTipper).expect("BigTipper misconfigured");
            assert_eq!(a, GRAND);
        }

        #[test]
        fn test_medium_spender() {
            let a: Balance = Spender::try_origin(TestOrigin::MediumSpender)
                .expect("MediumSpender misconfigured");
            assert_eq!(a, 100 * GRAND);
        }

        #[test]
        fn test_big_spender() {
            let a: Balance =
                Spender::try_origin(TestOrigin::BigSpender).expect("BigSpender misconfigured");
            assert_eq!(a, 1_000 * GRAND);
        }

        #[test]
        fn test_treasurer() {
            let a: Balance =
                Spender::try_origin(TestOrigin::Treasurer).expect("Treasurer misconfigured");
            assert_eq!(a, 10_000 * GRAND);
        }

        #[test]
        fn test_referendum_killer() {
            let a: TestOrigin = Spender::try_origin(TestOrigin::ReferendumKiller)
                .expect_err("ReferendumKiller misconfigured");
            assert_eq!(a, TestOrigin::ReferendumKiller);
        }

        #[test]
        fn test_referendum_canceller() {
            let a: TestOrigin = Spender::try_origin(TestOrigin::ReferendumCanceller)
                .expect_err("ReferendumCanceller misconfigured");
            assert_eq!(a, TestOrigin::ReferendumCanceller);
        }

        #[test]
        fn test_whitelisted_caller() {
            let a: TestOrigin = Spender::try_origin(TestOrigin::WhitelistedCaller)
                .expect_err("WhitelistedCaller misconfigured");
            assert_eq!(a, TestOrigin::WhitelistedCaller);
        }

        #[test]
        #[cfg(feature = "runtime-benchmarks")]
        fn test_try_successful_origin() {
            let a: TestOrigin = Spender::try_successful_origin().expect("incorrect setup");
            assert_eq!(a, TestOrigin::Treasurer);
        }
    }

    mod treasurer {
        use super::*;
        use crate::configs::pallet_custom_origins::Treasurer;

        #[test]
        pub fn test_treasurer_try_origin() {
            let () = Treasurer::try_origin(TestOrigin::Treasurer).expect("incorrect configuration");
            let a = Treasurer::try_origin(TestOrigin::MediumSpender).expect_err("should be err");
            assert_eq!(a, TestOrigin::MediumSpender)
        }

        #[test]
        #[cfg(feature = "runtime-benchmarks")]
        fn test_treasurer_try_successful_origin() {
            let a: Result<TestOrigin, ()> = Treasurer::try_successful_origin();
            assert_eq!(a, Ok(TestOrigin::Treasurer));
        }
    }

    mod referendum_canceller {
        use super::*;
        use crate::configs::pallet_custom_origins::ReferendumCanceller;

        #[test]
        pub fn test_referendum_canceller_try_origin() {
            let () = ReferendumCanceller::try_origin(TestOrigin::ReferendumCanceller)
                .expect("incorrect configuration");
            let a = ReferendumCanceller::try_origin(TestOrigin::MediumSpender)
                .expect_err("should be err");
            assert_eq!(a, TestOrigin::MediumSpender)
        }

        #[test]
        #[cfg(feature = "runtime-benchmarks")]
        fn test_treasurer_try_successful_origin() {
            let a: Result<TestOrigin, ()> = ReferendumCanceller::try_successful_origin();
            assert_eq!(a, Ok(TestOrigin::ReferendumCanceller));
        }
    }
}
