//! Track configurations for governance.

use super::*;
use crate::constants::MINUTES;

const fn percent(x: i32) -> sp_arithmetic::FixedI64 {
    sp_arithmetic::FixedI64::from_rational(x as u128, 100)
}
use pallet_referenda::Curve;
const APP_ROOT: Curve = Curve::make_reciprocal(4, 28, percent(80), percent(50), percent(100));
const SUP_ROOT: Curve = Curve::make_linear(28, 28, percent(0), percent(50));
const APP_TREASURER: Curve = Curve::make_reciprocal(4, 28, percent(80), percent(50), percent(100));
const SUP_TREASURER: Curve = Curve::make_linear(28, 28, percent(0), percent(50));
const APP_REFERENDUM_CANCELLER: Curve = Curve::make_linear(17, 28, percent(50), percent(100));
const SUP_REFERENDUM_CANCELLER: Curve =
    Curve::make_reciprocal(12, 28, percent(1), percent(0), percent(50));
const APP_REFERENDUM_KILLER: Curve = Curve::make_linear(17, 28, percent(50), percent(100));
const SUP_REFERENDUM_KILLER: Curve =
    Curve::make_reciprocal(12, 28, percent(1), percent(0), percent(50));
const APP_SMALL_TIPPER: Curve = Curve::make_linear(10, 28, percent(50), percent(100));
const SUP_SMALL_TIPPER: Curve = Curve::make_reciprocal(1, 28, percent(4), percent(0), percent(50));
const APP_BIG_TIPPER: Curve = Curve::make_linear(10, 28, percent(50), percent(100));
const SUP_BIG_TIPPER: Curve = Curve::make_reciprocal(8, 28, percent(1), percent(0), percent(50));
const APP_SMALL_SPENDER: Curve = Curve::make_linear(17, 28, percent(50), percent(100));
const SUP_SMALL_SPENDER: Curve =
    Curve::make_reciprocal(12, 28, percent(1), percent(0), percent(50));
const APP_MEDIUM_SPENDER: Curve = Curve::make_linear(23, 28, percent(50), percent(100));
const SUP_MEDIUM_SPENDER: Curve =
    Curve::make_reciprocal(16, 28, percent(1), percent(0), percent(50));
const APP_BIG_SPENDER: Curve = Curve::make_linear(28, 28, percent(50), percent(100));
const SUP_BIG_SPENDER: Curve = Curve::make_reciprocal(20, 28, percent(1), percent(0), percent(50));
const APP_WHITELISTED_CALLER: Curve =
    Curve::make_reciprocal(16, 28 * 24, percent(96), percent(50), percent(100));
const SUP_WHITELISTED_CALLER: Curve =
    Curve::make_reciprocal(1, 28, percent(20), percent(5), percent(50));

const TRACKS_DATA: [(u16, pallet_referenda::TrackInfo<Balance, BlockNumber>); 10] = [
    (
        0,
        pallet_referenda::TrackInfo {
            name: "root",
            max_deciding: 1,
            decision_deposit: 100 * GRAND,
            prepare_period: 8 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 12 * MINUTES,
            min_enactment_period: 5 * MINUTES,
            min_approval: APP_ROOT,
            min_support: SUP_ROOT,
        },
    ),
    (
        1,
        pallet_referenda::TrackInfo {
            name: "whitelisted_caller",
            max_deciding: 100,
            decision_deposit: 10 * GRAND,
            prepare_period: 6 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 4 * MINUTES,
            min_enactment_period: 3 * MINUTES,
            min_approval: APP_WHITELISTED_CALLER,
            min_support: SUP_WHITELISTED_CALLER,
        },
    ),
    (
        11,
        pallet_referenda::TrackInfo {
            name: "treasurer",
            max_deciding: 10,
            decision_deposit: GRAND,
            prepare_period: 8 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 8 * MINUTES,
            min_enactment_period: 5 * MINUTES,
            min_approval: APP_TREASURER,
            min_support: SUP_TREASURER,
        },
    ),
    (
        20,
        pallet_referenda::TrackInfo {
            name: "referendum_canceller",
            max_deciding: 1_000,
            decision_deposit: 10 * GRAND,
            prepare_period: 8 * MINUTES,
            decision_period: 14 * MINUTES,
            confirm_period: 8 * MINUTES,
            min_enactment_period: 3 * MINUTES,
            min_approval: APP_REFERENDUM_CANCELLER,
            min_support: SUP_REFERENDUM_CANCELLER,
        },
    ),
    (
        21,
        pallet_referenda::TrackInfo {
            name: "referendum_killer",
            max_deciding: 1_000,
            decision_deposit: 50 * GRAND,
            prepare_period: 8 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 8 * MINUTES,
            min_enactment_period: 3 * MINUTES,
            min_approval: APP_REFERENDUM_KILLER,
            min_support: SUP_REFERENDUM_KILLER,
        },
    ),
    (
        30,
        pallet_referenda::TrackInfo {
            name: "small_tipper",
            max_deciding: 200,
            decision_deposit: 3 * CENTS,
            prepare_period: MINUTES,
            decision_period: 14 * MINUTES,
            confirm_period: 4 * MINUTES,
            min_enactment_period: MINUTES,
            min_approval: APP_SMALL_TIPPER,
            min_support: SUP_SMALL_TIPPER,
        },
    ),
    (
        31,
        pallet_referenda::TrackInfo {
            name: "big_tipper",
            max_deciding: 100,
            decision_deposit: 10 * 3 * CENTS,
            prepare_period: 4 * MINUTES,
            decision_period: 14 * MINUTES,
            confirm_period: 12 * MINUTES,
            min_enactment_period: 3 * MINUTES,
            min_approval: APP_BIG_TIPPER,
            min_support: SUP_BIG_TIPPER,
        },
    ),
    (
        32,
        pallet_referenda::TrackInfo {
            name: "small_spender",
            max_deciding: 50,
            decision_deposit: 100 * 3 * CENTS,
            prepare_period: 10 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 10 * MINUTES,
            min_enactment_period: 5 * MINUTES,
            min_approval: APP_SMALL_SPENDER,
            min_support: SUP_SMALL_SPENDER,
        },
    ),
    (
        33,
        pallet_referenda::TrackInfo {
            name: "medium_spender",
            max_deciding: 50,
            decision_deposit: 200 * 3 * CENTS,
            prepare_period: 10 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 12 * MINUTES,
            min_enactment_period: 5 * MINUTES,
            min_approval: APP_MEDIUM_SPENDER,
            min_support: SUP_MEDIUM_SPENDER,
        },
    ),
    (
        34,
        pallet_referenda::TrackInfo {
            name: "big_spender",
            max_deciding: 50,
            decision_deposit: 400 * 3 * CENTS,
            prepare_period: 10 * MINUTES,
            decision_period: 20 * MINUTES,
            confirm_period: 14 * MINUTES,
            min_enactment_period: 5 * MINUTES,
            min_approval: APP_BIG_SPENDER,
            min_support: SUP_BIG_SPENDER,
        },
    ),
];

pub struct TracksInfo;
impl pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo {
    type Id = u16;
    type RuntimeOrigin = <RuntimeOrigin as frame_support::traits::OriginTrait>::PalletsOrigin;

    fn tracks() -> &'static [(Self::Id, pallet_referenda::TrackInfo<Balance, BlockNumber>)] {
        &TRACKS_DATA[..]
    }

    fn track_for(id: &Self::RuntimeOrigin) -> Result<Self::Id, ()> {
        if let Ok(system_origin) = frame_system::RawOrigin::try_from(id.clone()) {
            match system_origin {
                frame_system::RawOrigin::Root => Ok(0),
                _ => Err(()),
            }
        } else if let Ok(custom_origin) = origins::Origin::try_from(id.clone()) {
            match custom_origin {
                origins::Origin::WhitelistedCaller => Ok(1),
                origins::Origin::Treasurer => Ok(11),
                // Referendum admins
                origins::Origin::ReferendumCanceller => Ok(20),
                origins::Origin::ReferendumKiller => Ok(21),
                // Limited treasury spenders
                origins::Origin::SmallTipper => Ok(30),
                origins::Origin::BigTipper => Ok(31),
                origins::Origin::SmallSpender => Ok(32),
                origins::Origin::MediumSpender => Ok(33),
                origins::Origin::BigSpender => Ok(34),
            }
        } else {
            Err(())
        }
    }
}
pallet_referenda::impl_tracksinfo_get!(TracksInfo, Balance, BlockNumber);

#[cfg(test)]
mod tests {
    use super::origins;
    use crate::{Balance, BlockNumber, OriginCaller};

    #[test]
    fn test_track_system_origin() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::system(frame_system::RawOrigin::Root))
            .expect("incorrect config");
        assert_eq!(track, 0);
    }

    #[test]
    fn test_track_system_origin_error() {
        let () = <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
            Balance,
            BlockNumber,
        >>::track_for(&OriginCaller::system(frame_system::RawOrigin::None))
        .expect_err("incorrect config");
    }

    #[test]
    fn test_track_custom_origin_whitelisted_caller() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::WhitelistedCaller))
            .expect("incorrect config");
        assert_eq!(track, 1);
    }

    #[test]
    fn test_track_custom_origin_treasurer() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::Treasurer))
            .expect("incorrect config");
        assert_eq!(track, 11);
    }

    #[test]
    fn test_track_custom_origin_referendum_canceller() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::ReferendumCanceller))
            .expect("incorrect config");
        assert_eq!(track, 20);
    }

    #[test]
    fn test_track_custom_origin_referendum_killer() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::ReferendumKiller))
            .expect("incorrect config");
        assert_eq!(track, 21);
    }

    #[test]
    fn test_track_custom_origin_small_tipper() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::SmallTipper))
            .expect("incorrect config");
        assert_eq!(track, 30);
    }

    #[test]
    fn test_track_custom_origin_big_tipper() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::BigTipper))
            .expect("incorrect config");
        assert_eq!(track, 31);
    }

    #[test]
    fn test_track_custom_origin_small_spender() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::SmallSpender))
            .expect("incorrect config");
        assert_eq!(track, 32);
    }

    #[test]
    fn test_track_custom_origin_medium_spender() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::MediumSpender))
            .expect("incorrect config");
        assert_eq!(track, 33);
    }

    #[test]
    fn test_track_custom_origin_big_spender() {
        let track =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::Origins(origins::Origin::BigSpender))
            .expect("incorrect config");
        assert_eq!(track, 34);
    }

    #[test]
    fn test_track_error() {
        let () =
            <crate::configs::governance::tracks::TracksInfo as pallet_referenda::TracksInfo<
                Balance,
                BlockNumber,
            >>::track_for(&OriginCaller::CumulusXcm(cumulus_pallet_xcm::Origin::Relay))
            .expect_err("incorrect config");
    }
}
