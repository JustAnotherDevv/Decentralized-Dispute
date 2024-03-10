#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod dispute {
    use dia_oracle_randomness_getter::RandomOracleGetter;
    use ink::contract_ref;
    use ink::prelude::string::String;
    use ink::prelude::vec::Vec;
    // use ink::storage::collections::HashMap;
    use ink::storage::Mapping;
    // use ink_primitives::random::Random;
    // use ink_storage::collections::HashMap;
    // use openbrush::contracts::traits::psp22::PSP22Error;
    use openbrush::contracts::traits::psp22::*;

    // use ink_prelude::collections::HashMap;

    pub const ONE_MINUTE: u64 = 60 * 1000;
    pub const ONE_DAY: u64 = 60 * 1000; //24 * 60 * ONE_MINUTE;
    pub const ONE_YEAR: u64 = 365 * ONE_DAY;

    #[derive(scale::Decode, scale::Encode, Clone, PartialEq, Debug)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    enum EscrowStatus {
        Open,
        Completed,
        Claimed,
        RefundRequested,
        RefundRefused,
        RefundAccepted,
    }

    #[ink(storage)]
    pub struct Dispute {
        oracle: contract_ref!(RandomOracleGetter),
        token: AccountId,
        escrow_amount: u128,
        escrows: Mapping<u128, Escrow>,
        stakers_amount: u128,
        stakers: Mapping<AccountId, u128>,
        total_staked: u128,
        escrow_jury: Mapping<u128, ink_prelude::vec::Vec<AccountId>>,
    }

    #[derive(scale::Decode, scale::Encode, Clone)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Escrow {
        id: u128,
        buyer: AccountId,
        seller: AccountId,
        status: EscrowStatus,
        amount: u128,
        metadata: String,
        start_timestamp: u64,
    }

    pub struct EscrowArbitration {
        votes: bool,
        jury: AccountId,
    }

    impl Dispute {
        #[ink(constructor)]
        pub fn new(oracle_address: AccountId, token: AccountId) -> Self {
            let escrows = Mapping::new();
            let stakers = Mapping::new();
            let escrow_jury = Mapping::new();
            Self {
                oracle: oracle_address.into(),
                token,
                escrow_amount: 0,
                escrows,
                stakers_amount: 0,
                stakers,
                total_staked: 0,
                escrow_jury,
            }
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn open_escrow(
            &mut self,
            seller: AccountId,
            payment: u128,
            metadata: String,
        ) -> Result<(), ()> {
            let caller = self.env().caller();
            let now = self.env().block_timestamp();
            let contract = self.env().account_id();

            let escrow = Escrow {
                id: self.escrow_amount,
                buyer: caller,
                seller,
                status: EscrowStatus::Open,
                amount: payment,
                metadata,
                start_timestamp: now,
            };

            self.escrows.insert(self.escrow_amount, &escrow);
            self.escrow_amount += 1;

            PSP22Ref::transfer_from(&self.token, caller, contract, payment, Vec::new());

            Ok(())
        }

        #[ink(message)]
        pub fn complete_escrow(&mut self, id: u128) -> Result<(), ()> {
            let caller = self.env().caller();
            let now = self.env().block_timestamp();
            let mut selected_escrow = self.escrows.get(&id).unwrap();

            assert_eq!(selected_escrow.buyer, caller);
            assert_eq!(selected_escrow.status, EscrowStatus::Open);

            selected_escrow.status = EscrowStatus::Completed;

            Ok(())
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn withdraw_escrow(&self, id: u128) -> Result<(), ()> {
            let caller = self.env().caller();
            let now = self.env().block_timestamp();
            let mut selected_escrow = self.escrows.get(&id).unwrap();
            let time_since = now - selected_escrow.start_timestamp;

            if selected_escrow.status == EscrowStatus::RefundAccepted {
                assert_eq!(selected_escrow.buyer, caller);
            } else {
                assert_eq!(selected_escrow.seller, caller);
                if time_since > (ONE_DAY * 7) {
                    assert_eq!(selected_escrow.status, EscrowStatus::Open);
                } else {
                    assert_eq!(selected_escrow.status, EscrowStatus::Completed);
                }
        }

            selected_escrow.status = EscrowStatus::Claimed;

            PSP22Ref::transfer(&self.token, caller, selected_escrow.amount, Vec::new());

            Ok(())
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn request_escrow_refund(&mut self, id: u128) -> Result<(), ()> {
            let caller = self.env().caller();
            let now = self.env().block_timestamp();
            let mut selected_escrow = self.escrows.get(&id).unwrap();
            let time_since = now - selected_escrow.start_timestamp;

            assert_eq!(selected_escrow.buyer, caller);
            assert_eq!(selected_escrow.status, EscrowStatus::Open);
            if time_since > (ONE_DAY * 7) {
                // return Err(DisputeError::DisputeTimePassed);
                return Err(());
            }

            selected_escrow.status = EscrowStatus::RefundRequested;

            let mut accounts = Vec::new();
            accounts.push(caller);
            self.escrow_jury.insert(id, &accounts);

            Ok(())
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn vote_escrow(&self, id: u128) -> Result<(), ()> {
            let caller = self.env().caller();
            let mut selected_escrow = self.escrows.get(&id).unwrap();

            if !self.escrow_jury.get(&id).map_or(false, |jury| jury.contains(&caller)) {
                return Err("Caller is not a member of the jury for this escrow");
            }

            Ok(())
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn complete_escrow_vote(&self, id: u128) -> Result<(), ()> {
            let caller = self.env().caller();
            let mut selected_escrow = self.escrows.get(&id).unwrap();

            selected_escrow.status = EscrowStatus::RefundAccepted;

            Ok(())
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn stake(&mut self, amount: u128) -> Result<(), ()> {
            let caller = self.env().caller();
            let contract = self.env().account_id();

            assert_eq!(amount, 10000000000000000000 as u128);

            self.stakers.insert(caller, &amount);
            self.stakers_amount += 1;
            self.stakers_amount += amount;

            PSP22Ref::transfer_from(&self.token, caller, contract, amount, Vec::new());

            Ok(())
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn unstake(&mut self) -> Result<(), ()> {
            let caller = self.env().caller();
            let contract = self.env().account_id();
            let mut stake_entry = self.stakers.get(&caller).unwrap();

            if stake_entry == 0 {
                // return Err(DisputeError::EmptyStakeEntry);
                return Err(());
            }

            PSP22Ref::transfer(
                &self.token,
                caller,
                10000000000000000000 as u128,
                Vec::new(),
            );

            self.stakers.insert(caller, &0u128);
            self.stakers_amount -= 1;
            self.total_staked -= 10000000000000000000 as u128;

            Ok(())
        }

        #[ink(message)]
        pub fn total_staked(&self) -> u128 {
            self.total_staked
        }

        #[ink(message)]
        pub fn stakers_amount(&self) -> u128 {
            self.stakers_amount
        }

        #[ink(message)]
        pub fn escrow_amount(&self) -> u128 {
            self.escrow_amount
        }

        #[ink(message)]
        pub fn get_random_value(&self, key: u64) -> Option<Vec<u8>> {
            self.oracle.get_random_value_for_round(key)
        }

        #[ink(message)]
        pub fn fetch_latest(&self) -> u64 {
            self.oracle.get_latest_round()
        }

        #[allow(clippy::arithmetic_side_effects)]
        #[ink(message)]
        pub fn get_random_number_in_range(&self, max_value: u8) -> u8 {
            let hash_value_option = self.get_random_value(self.fetch_latest());

            let hash_value = match hash_value_option {
                Some(value) => value,
                None => return 0,
            };

            let first_byte = hash_value.first().copied().unwrap_or(0);

            if first_byte >= max_value {
                return max_value;
            }

            if max_value == u8::MAX {
                return first_byte;
            }

            let remainder = first_byte % (max_value + 1);
            remainder
        }
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum DisputeError {
        DisputeTimePassed,
        EmptyStakeEntry,
    }
}
