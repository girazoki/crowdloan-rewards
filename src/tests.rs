// Copyright 2019-2021 PureStake Inc.
// This file is part of Moonbeam.

// Moonbeam is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Moonbeam is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Moonbeam.  If not, see <http://www.gnu.org/licenses/>.

//! Unit testing
use crate::*;
use frame_support::dispatch::{DispatchError, Dispatchable};
use frame_support::{assert_noop, assert_ok};
use mock::*;
use parity_scale_codec::Encode;
use sp_core::Pair;
use sp_runtime::MultiSignature;

// Constant that reflects the desired vesting period for the tests
// Most tests complete initialization passing initRelayBlock + VESTING as the endRelayBlock
const VESTING: u32 = 8;

#[test]
fn geneses() {
	empty().execute_with(|| {
		assert!(System::events().is_empty());
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));
		assert_eq!(Crowdloan::total_contributors(), 5);

		// accounts_payable
		assert!(Crowdloan::accounts_payable(&1).is_some());
		assert!(Crowdloan::accounts_payable(&2).is_some());
		assert!(Crowdloan::accounts_payable(&3).is_none());
		assert!(Crowdloan::accounts_payable(&4).is_none());
		assert!(Crowdloan::accounts_payable(&5).is_none());

		// claimed address existence
		assert_eq!(Crowdloan::claimed_relay_chain_ids(&[1u8; 32]).unwrap(), 1);
		assert_eq!(Crowdloan::claimed_relay_chain_ids(&[2u8; 32]).unwrap(), 2);
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[0].public().as_array_ref()).is_none());
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[1].public().as_array_ref()).is_none());
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[2].public().as_array_ref()).is_none());

		// unassociated_contributions
		assert!(Crowdloan::unassociated_contributions(&[1u8; 32]).is_none());
		assert!(Crowdloan::unassociated_contributions(&[2u8; 32]).is_none());
		assert!(Crowdloan::unassociated_contributions(pairs[0].public().as_array_ref()).is_some());
		assert!(Crowdloan::unassociated_contributions(pairs[1].public().as_array_ref()).is_some());
		assert!(Crowdloan::unassociated_contributions(pairs[2].public().as_array_ref()).is_some());
	});
}

#[test]
fn proving_assignation_works() {
	let pairs = get_ed25519_pairs(4);
	let signature: MultiSignature = pairs[0].sign(&3u64.encode()).into();
	let signature_2: MultiSignature = pairs[0].sign(&5u64.encode()).into();
	let non_contributed_signature: MultiSignature = pairs[3].sign(&5u64.encode()).into();

	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(4);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			],
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));
		// 4 is not payable first
		assert!(Crowdloan::accounts_payable(&3).is_none());
		roll_to(4);
		// Signature is wrong, prove fails
		assert_noop!(
			Crowdloan::associate_native_identity(
				Origin::signed(4),
				4,
				vec![(pairs[0].public().into(), signature.clone())]
			),
			Error::<Test>::InvalidClaimSignature
		);
		// Signature is right, prove passes to unassociated
		assert_ok!(Crowdloan::associate_native_identity(
			Origin::signed(4),
			3,
			vec![(pairs[0].public().into(), signature.clone())]
		));

		// now three is payable
		assert!(Crowdloan::accounts_payable(&3).is_some());
		assert!(Crowdloan::unassociated_contributions(pairs[0].public().as_array_ref()).is_none());
		assert_eq!(
			Crowdloan::claimed_relay_chain_ids(pairs[0].public().as_array_ref()).unwrap(),
			3
		);

		// We are going to change the reward address through relay chains key
		assert_ok!(Crowdloan::associate_native_identity(
			Origin::signed(4),
			5,
			vec![(pairs[0].public().into(), signature_2.clone())]
		),);

		// now five is payable
		assert!(Crowdloan::accounts_payable(&3).is_none());
		assert!(Crowdloan::accounts_payable(&5).is_some());
		assert_eq!(
			Crowdloan::claimed_relay_chain_ids(pairs[0].public().as_array_ref()).unwrap(),
			5
		);

		// Ensure a non-contributed relay cannot do anything
		assert_noop!(
			Crowdloan::associate_native_identity(
				Origin::signed(4),
				5,
				vec![(pairs[3].public().into(), non_contributed_signature.clone())]
			),
			Error::<Test>::NoAssociatedClaim
		);

		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(2, 100),
			crate::Event::InitialPaymentMade(3, 100),
			crate::Event::NativeIdentityAssociated(3, 500),
			crate::Event::NativeIdentityAssociated(5, 500),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn initializing_multi_relay_to_single_native_address_works() {
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(1), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));
		// 1 is payable
		assert!(Crowdloan::accounts_payable(&1).is_some());
		roll_to(4);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 400);
		assert_noop!(
			Crowdloan::claim(Origin::signed(3)),
			Error::<Test>::NoAssociatedClaim
		);

		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::RewardsPaid(1, 200),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn paying_works_step_by_step() {
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));
		// 1 is payable
		assert!(Crowdloan::accounts_payable(&1).is_some());
		roll_to(4);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 200);
		assert_noop!(
			Crowdloan::claim(Origin::signed(3)),
			Error::<Test>::NoAssociatedClaim
		);
		roll_to(5);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 250);
		roll_to(6);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 300);
		roll_to(7);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 350);
		roll_to(8);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 400);
		roll_to(9);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 450);
		roll_to(10);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 500);
		roll_to(11);
		assert_noop!(
			Crowdloan::claim(Origin::signed(1)),
			Error::<Test>::RewardsAlreadyClaimed
		);

		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(2, 100),
			crate::Event::RewardsPaid(1, 100),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn paying_works_after_unclaimed_period() {
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		// 1 is payable
		assert!(Crowdloan::accounts_payable(&1).is_some());
		roll_to(4);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 200);
		assert_noop!(
			Crowdloan::claim(Origin::signed(3)),
			Error::<Test>::NoAssociatedClaim
		);
		roll_to(5);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 250);
		roll_to(6);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 300);
		roll_to(7);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 350);
		roll_to(11);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 500);
		roll_to(330);
		assert_noop!(
			Crowdloan::claim(Origin::signed(1)),
			Error::<Test>::RewardsAlreadyClaimed
		);

		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(2, 100),
			crate::Event::RewardsPaid(1, 100),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 50),
			crate::Event::RewardsPaid(1, 150),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn paying_late_joiner_works() {
	let pairs = get_ed25519_pairs(3);
	let signature: MultiSignature = pairs[0].sign(&3u64.encode()).into();
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		roll_to(12);
		assert_ok!(Crowdloan::associate_native_identity(
			Origin::signed(4),
			3,
			vec![(pairs[0].public().into(), signature.clone())]
		));
		assert_ok!(Crowdloan::claim(Origin::signed(3)));
		assert_eq!(Crowdloan::accounts_payable(&3).unwrap().claimed_reward, 500);
		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(2, 100),
			crate::Event::InitialPaymentMade(3, 100),
			crate::Event::NativeIdentityAssociated(3, 500),
			crate::Event::RewardsPaid(3, 400),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn update_address_works() {
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		roll_to(4);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_noop!(
			Crowdloan::claim(Origin::signed(8)),
			Error::<Test>::NoAssociatedClaim
		);
		assert_ok!(Crowdloan::update_reward_address(
			Origin::signed(1),
			8,
			[1u8; 32].into()
		));
		assert_eq!(Crowdloan::claimed_relay_chain_ids(&[1u8; 32]).unwrap(), 8);

		assert_eq!(Crowdloan::accounts_payable(&8).unwrap().claimed_reward, 200);
		roll_to(6);
		assert_ok!(Crowdloan::claim(Origin::signed(8)));
		assert_eq!(Crowdloan::accounts_payable(&8).unwrap().claimed_reward, 300);
		// The initial payment is not
		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(2, 100),
			crate::Event::RewardsPaid(1, 100),
			crate::Event::RewardAddressUpdated([1u8; 32].into(), 1, 8),
			crate::Event::RewardsPaid(8, 100),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn update_address_with_existing_address_works() {
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		roll_to(4);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));
		assert_ok!(Crowdloan::claim(Origin::signed(2)));
		assert_ok!(Crowdloan::update_reward_address(
			Origin::signed(1),
			2,
			[1u8; 32].into()
		));
		assert_eq!(Crowdloan::claimed_relay_chain_ids(&[1u8; 32]).unwrap(), 2);

		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().claimed_reward, 400);
		assert_noop!(
			Crowdloan::claim(Origin::signed(1)),
			Error::<Test>::NoAssociatedClaim
		);
		roll_to(6);
		assert_ok!(Crowdloan::claim(Origin::signed(2)));
		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().claimed_reward, 600);
		let expected = vec![
			crate::Event::InitialPaymentMade(1, 100),
			crate::Event::InitialPaymentMade(2, 100),
			crate::Event::RewardsPaid(1, 100),
			crate::Event::RewardsPaid(2, 100),
			crate::Event::RewardAddressUpdated([1u8; 32].into(), 1, 2),
			crate::Event::RewardsPaid(2, 200),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn update_address_with_existing_with_multi_address_works() {
	empty().execute_with(|| {
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(1), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		roll_to(4);
		assert_ok!(Crowdloan::claim(Origin::signed(1)));

		// We make sure all rewards go to the new address
		assert_ok!(Crowdloan::update_reward_address(
			Origin::signed(1),
			2,
			[1u8; 32].into()
		));
		assert_eq!(Crowdloan::claimed_relay_chain_ids(&[1u8; 32]).unwrap(), 2);

		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().claimed_reward, 400);
		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().total_reward, 1000);

		assert_noop!(
			Crowdloan::claim(Origin::signed(1)),
			Error::<Test>::NoAssociatedClaim
		);
	});
}

#[test]
fn initialize_new_addresses() {
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		// Insert contributors
		let pairs = get_ed25519_pairs(3);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 500u32.into()),
				(pairs[2].public().into(), None, 500u32.into())
			]
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		assert_eq!(Crowdloan::initialized(), true);

		roll_to(4);
		assert_noop!(
			Crowdloan::initialize_reward_vec(
				Origin::root(),
				vec![([1u8; 32].into(), Some(1), 500u32.into())]
			),
			Error::<Test>::RewardVecAlreadyInitialized,
		);

		assert_noop!(
			Crowdloan::complete_initialization(Origin::root(), init_block + VESTING * 2),
			Error::<Test>::RewardVecAlreadyInitialized,
		);
	});
}

#[test]
fn initialize_new_addresses_handle_dust() {
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		// Insert contributors
		let pairs = get_ed25519_pairs(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 999u32.into()),
			]
		));

		let crowdloan_pot = Crowdloan::pot();
		let previous_issuance = Balances::total_issuance();
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		// We have burnt 1 unit
		assert!(Crowdloan::pot() == crowdloan_pot - 1);
		assert!(Balances::total_issuance() == previous_issuance - 1);

		assert_eq!(Crowdloan::initialized(), true);
		assert_eq!(Balances::free_balance(10), 0);
	});
}

#[test]
fn initialize_new_addresses_not_matching_funds() {
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		// Insert contributors
		let pairs = get_ed25519_pairs(2);
		let init_block = Crowdloan::init_relay_block();
		// Total supply is 2500.Lets ensure inserting 2495 is not working.
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 500u32.into()),
				([2u8; 32].into(), Some(2), 500u32.into()),
				(pairs[0].public().into(), None, 500u32.into()),
				(pairs[1].public().into(), None, 995u32.into()),
			]
		));
		assert_noop!(
			Crowdloan::complete_initialization(Origin::root(), init_block + VESTING),
			Error::<Test>::RewardsDoNotMatchFund
		);
	});
}

#[test]
fn initialize_new_addresses_with_batch() {
	empty().execute_with(|| {
		// This time should succeed trully
		roll_to(10);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(mock::Call::Utility(UtilityCall::batch_all(vec![
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[4u8; 32].into(),
				Some(3),
				1250
			)],)),
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[5u8; 32].into(),
				Some(1),
				1250
			)],))
		]))
		.dispatch(Origin::root()));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));
		assert_eq!(Crowdloan::total_contributors(), 2);
		// Verify that the second ending block provider had no effect
		assert_eq!(Crowdloan::end_relay_block(), init_block + VESTING);

		// Batch calls always succeed. We just need to check the inner event
		assert_ok!(
			mock::Call::Utility(UtilityCall::batch(vec![mock::Call::Crowdloan(
				crate::Call::initialize_reward_vec(vec![([4u8; 32].into(), Some(3), 500)])
			)]))
			.dispatch(Origin::root())
		);

		let expected = vec![
			pallet_utility::Event::BatchCompleted,
			pallet_utility::Event::BatchInterrupted(
				0,
				DispatchError::Module {
					index: 2,
					error: 7,
					message: None,
				},
			),
		];
		assert_eq!(batch_events(), expected);
	});
}

#[test]
fn floating_point_arithmetic_works() {
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(mock::Call::Utility(UtilityCall::batch_all(vec![
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[4u8; 32].into(),
				Some(1),
				1190
			)])),
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[5u8; 32].into(),
				Some(2),
				1185
			)])),
			// We will work with this. This has 100/8=12.5 payable per block
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[3u8; 32].into(),
				Some(3),
				125
			)]))
		]))
		.dispatch(Origin::root()));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));
		assert_eq!(Crowdloan::total_contributors(), 3);

		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			25u128
		);

		// Block relay number is 2 post init initialization
		// In this case there is no problem. Here we pay 12.5*2=25
		// Total claimed reward: 25+25 = 50
		roll_to(4);

		assert_ok!(Crowdloan::claim(Origin::signed(3)));

		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			50u128
		);
		roll_to(5);
		// If we claim now we have to pay 12.5. 12 will be paid.
		assert_ok!(Crowdloan::claim(Origin::signed(3)));

		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			62u128
		);
		roll_to(6);
		// Now we should pay 12.5. However the calculus will be:
		// Account 3 should have claimed 50 + 25 at this block, but
		// he only claimed 62. The payment is 13
		assert_ok!(Crowdloan::claim(Origin::signed(3)));
		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			75u128
		);
		let expected = vec![
			crate::Event::InitialPaymentMade(1, 238),
			crate::Event::InitialPaymentMade(2, 237),
			crate::Event::InitialPaymentMade(3, 25),
			crate::Event::RewardsPaid(3, 25),
			crate::Event::RewardsPaid(3, 12),
			crate::Event::RewardsPaid(3, 13),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn reward_below_vesting_period_works() {
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(mock::Call::Utility(UtilityCall::batch_all(vec![
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[4u8; 32].into(),
				Some(1),
				1247
			)])),
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[5u8; 32].into(),
				Some(2),
				1247
			)])),
			// We will work with this. This has 5/8=0.625 payable per block
			mock::Call::Crowdloan(crate::Call::initialize_reward_vec(vec![(
				[3u8; 32].into(),
				Some(3),
				6
			)]))
		]))
		.dispatch(Origin::root()));

		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			1u128
		);

		// Block relay number is 2 post init initialization
		// Here we should pay floor(0.625*2)=1
		// Total claimed reward: 1+1 = 2
		roll_to(4);

		assert_ok!(Crowdloan::claim(Origin::signed(3)));

		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			2u128
		);
		roll_to(5);
		// If we claim now we have to pay floor(0.625) = 0
		assert_ok!(Crowdloan::claim(Origin::signed(3)));

		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			2u128
		);
		roll_to(6);
		// Now we should pay 1 again. The claimer should have claimed floor(0.625*4) + 1
		// but he only claimed 2
		assert_ok!(Crowdloan::claim(Origin::signed(3)));
		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			3u128
		);
		roll_to(10);
		// We pay the remaining
		assert_ok!(Crowdloan::claim(Origin::signed(3)));
		assert_eq!(
			Crowdloan::accounts_payable(&3).unwrap().claimed_reward,
			6u128
		);
		roll_to(11);
		// Nothing more to claim
		assert_noop!(
			Crowdloan::claim(Origin::signed(3)),
			Error::<Test>::RewardsAlreadyClaimed
		);

		let expected = vec![
			crate::Event::InitialPaymentMade(1, 249),
			crate::Event::InitialPaymentMade(2, 249),
			crate::Event::InitialPaymentMade(3, 1),
			crate::Event::RewardsPaid(3, 1),
			crate::Event::RewardsPaid(3, 0),
			crate::Event::RewardsPaid(3, 1),
			crate::Event::RewardsPaid(3, 3),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn test_initialization_errors() {
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		let init_block = Crowdloan::init_relay_block();

		let pot = Crowdloan::pot();

		// Too many contributors
		assert_noop!(
			Crowdloan::initialize_reward_vec(
				Origin::root(),
				vec![
					([1u8; 32].into(), Some(1), 1),
					([2u8; 32].into(), Some(2), 1),
					([3u8; 32].into(), Some(3), 1),
					([4u8; 32].into(), Some(4), 1),
					([5u8; 32].into(), Some(5), 1),
					([6u8; 32].into(), Some(6), 1),
					([7u8; 32].into(), Some(7), 1),
					([8u8; 32].into(), Some(8), 1),
					([9u8; 32].into(), Some(9), 1)
				]
			),
			Error::<Test>::TooManyContributors
		);

		// Go beyond fund pot
		assert_noop!(
			Crowdloan::initialize_reward_vec(
				Origin::root(),
				vec![([1u8; 32].into(), Some(1), pot + 1)]
			),
			Error::<Test>::BatchBeyondFundPot
		);

		// Dont fill rewards
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![([1u8; 32].into(), Some(1), pot - 1)]
		));

		// Fill rewards
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![([2u8; 32].into(), Some(2), 1)]
		));

		// Insert a non-valid vesting period
		assert_noop!(
			Crowdloan::complete_initialization(Origin::root(), init_block),
			Error::<Test>::VestingPeriodNonValid
		);

		// Cannot claim if we dont complete initialization
		assert_noop!(
			Crowdloan::claim(Origin::signed(1)),
			Error::<Test>::RewardVecNotFullyInitializedYet
		);
		// Complete
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		// Cannot initialize again
		assert_noop!(
			Crowdloan::complete_initialization(Origin::root(), init_block),
			Error::<Test>::RewardVecAlreadyInitialized
		);
	});
}

#[test]
fn assignation_with_existing_key_works() {
	let pairs = get_ed25519_pairs(1);
	let signature: MultiSignature = pairs[0].sign(&1u64.encode()).into();
	empty().execute_with(|| {
		// The init relay block gets inserted
		roll_to(2);
		// Insert contributors
		let init_block = Crowdloan::init_relay_block();
		assert_ok!(Crowdloan::initialize_reward_vec(
			Origin::root(),
			vec![
				([1u8; 32].into(), Some(1), 1250u32.into()),
				(pairs[0].public().into(), None, 1250u32.into()),
			],
		));
		assert_ok!(Crowdloan::complete_initialization(
			Origin::root(),
			init_block + VESTING
		));

		// 1 is payable, but its rewards only contain the associated part
		assert!(Crowdloan::accounts_payable(&1).is_some());
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 250);
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().total_reward, 1250);

		roll_to(4);

		// Let's say we claim now
		// Every step we should claim 125
		// We started at 2
		assert_ok!(Crowdloan::claim(Origin::signed(1),));

		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 500);

		// Signature is right, prove passes
		assert_ok!(Crowdloan::associate_native_identity(
			Origin::signed(1),
			1,
			vec![(pairs[0].public().into(), signature.clone())]
		));

		roll_to(5);

		// 3 periods, 125 per period * 3 periods = 375, plus initial payment = 625
		// But we have two relay accounts pointing at the same native account, so 1250
		assert_ok!(Crowdloan::claim(Origin::signed(1),));

		assert!(Crowdloan::accounts_payable(&1).is_some());
		assert_eq!(
			Crowdloan::accounts_payable(&1).unwrap().claimed_reward,
			1250
		);
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().total_reward, 2500);
	});
}
