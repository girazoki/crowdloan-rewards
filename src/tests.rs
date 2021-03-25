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
use frame_support::dispatch::DispatchError;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use parity_scale_codec::Encode;
use sp_core::Pair;
use sp_runtime::MultiSignature;

#[test]
fn geneses() {
	let pairs = get_ed25519_pairs(3);
	two_assigned_three_unassigned().execute_with(|| {
		assert!(Sys::events().is_empty());
		// accounts_payable
		assert!(Crowdloan::accounts_payable(&1).is_some());
		assert!(Crowdloan::accounts_payable(&2).is_some());
		assert!(Crowdloan::accounts_payable(&3).is_none());
		assert!(Crowdloan::accounts_payable(&4).is_none());
		assert!(Crowdloan::accounts_payable(&5).is_none());

		// claimed address existence
		assert!(Crowdloan::claimed_relay_chain_ids(&[1u8; 32]).is_some());
		assert!(Crowdloan::claimed_relay_chain_ids(&[2u8; 32]).is_some());
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
	let pairs = get_ed25519_pairs(3);
	let signature: MultiSignature = pairs[0].sign(&3u64.encode()).into();
	two_assigned_three_unassigned().execute_with(|| {
		// 4 is not payable first
		assert!(Crowdloan::accounts_payable(&3).is_none());
		roll_to(4);
		// Signature is wrong, prove fails
		assert_noop!(
			Crowdloan::associate_native_identity(
				Origin::signed(4),
				4,
				pairs[0].public().into(),
				signature.clone()
			),
			Error::<Test>::InvalidClaimSignature
		);
		// Signature is right, prove passes
		assert_ok!(Crowdloan::associate_native_identity(
			Origin::signed(4),
			3,
			pairs[0].public().into(),
			signature.clone()
		));
		// Signature is right, but address already claimed
		assert_noop!(
			Crowdloan::associate_native_identity(
				Origin::signed(4),
				3,
				pairs[0].public().into(),
				signature
			),
			Error::<Test>::AlreadyAssociated
		);
		// now three is payable
		assert!(Crowdloan::accounts_payable(&3).is_some());
		assert!(Crowdloan::unassociated_contributions(pairs[0].public().as_array_ref()).is_none());
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[0].public().as_array_ref()).is_some());

		let expected = vec![Event::NativeIdentityAssociated(
			pairs[0].public().into(),
			3,
			500,
		)];
		assert_eq!(events(), expected);
	});
}

#[test]
fn paying_works() {
	two_assigned_three_unassigned().execute_with(|| {
		// 1 is payable
		assert!(Crowdloan::accounts_payable(&1).is_some());
		roll_to(4);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().last_paid, 4u64);
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 248);
		assert_noop!(
			Crowdloan::show_me_the_money(Origin::signed(3)),
			Error::<Test>::NoAssociatedClaim
		);
		roll_to(5);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().last_paid, 5u64);
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 310);
		roll_to(6);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().last_paid, 6u64);
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 372);
		roll_to(7);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().last_paid, 7u64);
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 434);
		roll_to(230);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_eq!(Crowdloan::accounts_payable(&1).unwrap().claimed_reward, 500);
		roll_to(330);
		assert_noop!(
			Crowdloan::show_me_the_money(Origin::signed(1)),
			Error::<Test>::RewardsAlreadyClaimed
		);

		let expected = vec![
			Event::RewardsPaid(1, 248),
			Event::RewardsPaid(1, 62),
			Event::RewardsPaid(1, 62),
			Event::RewardsPaid(1, 62),
			Event::RewardsPaid(1, 66),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn paying_late_joiner_works() {
	let pairs = get_ed25519_pairs(3);
	let signature: MultiSignature = pairs[0].sign(&3u64.encode()).into();
	two_assigned_three_unassigned().execute_with(|| {
		//
		roll_to(12);
		assert_ok!(Crowdloan::associate_native_identity(
			Origin::signed(4),
			3,
			pairs[0].public().into(),
			signature.clone()
		));
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(3)));
		assert_eq!(Crowdloan::accounts_payable(&3).unwrap().last_paid, 12u64);
		assert_eq!(Crowdloan::accounts_payable(&3).unwrap().claimed_reward, 500);
		let expected = vec![
			Event::NativeIdentityAssociated(pairs[0].public().into(), 3, 500),
			Event::RewardsPaid(3, 500),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn update_address_works() {
	two_assigned_three_unassigned().execute_with(|| {
		roll_to(4);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_noop!(
			Crowdloan::show_me_the_money(Origin::signed(8)),
			Error::<Test>::NoAssociatedClaim
		);
		assert_ok!(Crowdloan::update_reward_address(Origin::signed(1), 8));
		assert_eq!(Crowdloan::accounts_payable(&8).unwrap().last_paid, 4u64);
		assert_eq!(Crowdloan::accounts_payable(&8).unwrap().claimed_reward, 248);
		roll_to(6);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(8)));
		assert_eq!(Crowdloan::accounts_payable(&8).unwrap().last_paid, 6u64);
		assert_eq!(Crowdloan::accounts_payable(&8).unwrap().claimed_reward, 372);
		let expected = vec![
			Event::RewardsPaid(1, 248),
			Event::RewardAddressUpdated(1, 8),
			Event::RewardsPaid(8, 124),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn update_address_with_existing_address_works() {
	two_assigned_three_unassigned().execute_with(|| {
		roll_to(4);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(1)));
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(2)));
		assert_ok!(Crowdloan::update_reward_address(Origin::signed(1), 2));
		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().last_paid, 4u64);
		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().claimed_reward, 496);
		assert_noop!(
			Crowdloan::show_me_the_money(Origin::signed(1)),
			Error::<Test>::NoAssociatedClaim
		);
		roll_to(6);
		assert_ok!(Crowdloan::show_me_the_money(Origin::signed(2)));
		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().last_paid, 6u64);
		assert_eq!(Crowdloan::accounts_payable(&2).unwrap().claimed_reward, 746);
		let expected = vec![
			Event::RewardsPaid(1, 248),
			Event::RewardsPaid(2, 248),
			Event::RewardAddressUpdated(1, 2),
			Event::RewardsPaid(2, 250),
		];
		assert_eq!(events(), expected);
	});
}

#[test]
fn initialize_new_addresses() {
	let pairs = get_ed25519_pairs(7);
	two_assigned_three_unassigned().execute_with(|| {
		roll_to(4);
		assert_noop!(
			Crowdloan::initialize_reward_vec(
				Origin::root(),
				vec![([1u8; 32].into(), Some(1), 500)],
				1
			),
			Error::<Test>::AlreadyInitialized
		);
		assert_ok!(Crowdloan::batch_calls(
			Origin::root(),
			vec![mock::Call::Crowdloan(crate::Call::initialize_reward_vec(
				vec![([1u8; 32].into(), Some(1), 500)],
				1
			))]
		),);
		assert_ok!(Crowdloan::batch_calls(
			Origin::root(),
			vec![
				mock::Call::Crowdloan(crate::Call::initialize_reward_vec(
					vec![(pairs[4].public().into(), Some(3), 500)],
					1
				)),
				mock::Call::Crowdloan(crate::Call::initialize_reward_vec(
					vec![([1u8; 32].into(), Some(1), 500)],
					1
				))
			]
		));

		// assert pairs[4] has been associated
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[4].public().as_array_ref()).is_some());

		assert_ok!(Crowdloan::batch_calls(
			Origin::root(),
			vec![
				mock::Call::Crowdloan(crate::Call::initialize_reward_vec(
					vec![(pairs[5].public().into(), Some(6), 500)],
					1
				)),
				mock::Call::Crowdloan(crate::Call::initialize_reward_vec(
					vec![(pairs[6].public().into(), Some(7), 500)],
					1
				))
			]
		));

		// assert pairs[4] has been associated
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[5].public().as_array_ref()).is_some());
		assert!(Crowdloan::claimed_relay_chain_ids(pairs[6].public().as_array_ref()).is_some());

		let expected = vec![
			Event::BatchInterrupted(
				0,
				DispatchError::Module {
					index: 0,
					error: 1,
					message: None,
				},
			),
			Event::BatchInterrupted(
				1,
				DispatchError::Module {
					index: 0,
					error: 1,
					message: None,
				},
			),
			Event::BatchCompleted,
		];
		assert_eq!(events(), expected);
	});
}
