/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

use support::{decl_module, decl_storage, decl_event, StorageMap, StorageValue, dispatch::Result, ensure, Parameter};
use system::ensure_signed;
use runtime_primitives::traits::{As, Hash, Zero, Member, Bounded, SimpleArithmetic, MaybeSerializeDebug };
use parity_codec::{Encode, Decode, Codec};
use rstd::prelude::Vec;
use rstd::result;
use crate::token;

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub struct TradePair<Hash>{
    hash:Hash,
    base:Hash,
    quote:Hash,
}
#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq)]
// #[derive(PartialEq, Eq, Copy, Clone, Encode, Decode)]
// #[cfg_attr(feature = "std", derive(Debug))]
pub enum OrderType{
    Buy,
    Sell
}
impl Default for OrderType {
    fn default() -> Self {
        OrderType::Buy
    }
}


#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq)]
pub enum OrderStatus{
    Created,
    PartialFilled,
    Filled,
    Canceled,
}

#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq)]
pub struct LimitOrder<T> where T: Trait{
    hash: T::Hash,
    base: T::Hash,
    quote: T::Hash,
    owner: T::AccountId,
    price: T::Price,
    amount: T::Balance,
    remained_amount: T::Balance,
    otype: OrderType,
    status: OrderStatus,
}

impl<T:Trait> LimitOrder<T>{
    pub fn new (base: T::Hash, quote:T::Hash, owner:T::AccountId, price : T::Price, amount : T::Balance, otype: OrderType) ->Self {
        let hash = (base, quote, price, amount , owner.clone()).using_encoded(<T as system::Trait>::Hashing::hash);
        LimitOrder {
            hash, base, quote, owner, price, otype, amount,
            status: OrderStatus::Created,
            remained_amount:amount,
        }
        
    }
}



/// The module's configuration trait.
pub trait Trait:  token::Trait + system::Trait {
	// TODO: Add other types and constants required configure this module.

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	// type Price: Parameter + Member + Default + Codec +  Bounded + SimpleArithmetic + Copy;
	type Price: Parameter + Member + SimpleArithmetic + Codec + Default + Copy + MaybeSerializeDebug + From<Self::BlockNumber>;
}

/// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as trade{
		// TradePairByHash ==> TradePair
		TradePairByHash get(trade_pair_by_hash): map T::Hash => Option<TradePair<T::Hash>>;
        // (BaseTokenHash, QuoteTokenHash) ==> TradePairHash
        TradePairHashByBaseQuote get(get_trade_pair_hash_by_base_quote) : map (T::Hash, T::Hash) => Option<T::Hash>;
        Orders get(order) :map T::Hash => Option<LimitOrder<T>> ;
        OwnedOrders get(owned_order): map( T::AccountId, u64) => Option<T::Hash> ;
        OwnedOrdersIndex get(owned_order_index):map T::AccountId => u64;
        
        TradePairOwnedOrders get(trade_pair_owned_order):map (T::Hash, u64)=> Option<T::Hash>;
        TradePairOwnedOrdersIndex get(trade_pair_owned_order_index):map T::Hash => u64;
        Nonce: u64;

	}
}

decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		// Initializing events
		// this is needed only if you are using events in your module
		fn deposit_event<T>() = default;

		// Just a dummy entry point.
		// function that can be called by the external world as an extrinsics call
		// takes a parameter of the type `AccountId`, stores it and emits an event
		pub fn create_trade_pair(origin, base: T::Hash, quote: T::Hash) -> Result {
		    let sender = ensure_signed(origin)?;
            Self::do_create_trade_pair(sender, base, quote)
        }
        pub fn test_fn(origin, price: T::Price) -> Result {
		    let sender = ensure_signed(origin)?;
            Self::deposit_event(RawEvent::TestFn(sender.clone()));
            Ok(())
        }
        pub fn create_limit_order(origin, base:T::Hash, quote:T::Hash, otype: OrderType, price: T::Price, amount: T::Balance) -> Result{
		    let sender = ensure_signed(origin)?;
            Self::do_create_limit_order(sender, base, quote, otype, price, amount)
        }
		
	}
}

decl_event!(
	pub enum Event<T> where  
    <T as system::Trait>::AccountId,
    <T as system::Trait>::Hash,
    <T as Trait>::Price,
    <T as balances::Trait>::Balance,
    TradePair = TradePair<<T as system::Trait>::Hash>,
    {
		// Just a dummy event.
		// Event `Something` is declared with a parameter of the type `u32` and `AccountId`
		// To emit this event, we call the deposit funtion, from our runtime funtions
		TradePairCreated( AccountId, Hash, Hash, Hash, TradePair),
		// TradePairCreated( AccountId, Hash, Hash, Hash),
		OrderCreated( AccountId, Hash, Hash, Hash, Price, Balance),
		TestFn( AccountId),
	}
);




impl<T: Trait> Module<T>{
    pub fn ensure_trade_pair(base: T::Hash, quote: T::Hash)->result::Result<T::Hash, &'static str>{
        let tp = Self::get_trade_pair_hash_by_base_quote((base,quote));
        ensure!(tp.is_some(), "");
        match tp {
            Some(tp) => {return Ok( tp )},
            None => {
                return Err("")
            }
        }
    }
    pub fn do_create_trade_pair(sender: T::AccountId, base: T::Hash, quote: T::Hash) -> Result {
        ensure!(base != quote, "base shouldnt' equal with quote!");
        let base_owner = <token::Module<T>>::owner(base);
        let quote_owner = <token::Module<T>>::owner(quote);
        ensure!(base_owner.is_some() && quote_owner.is_some() , "base owner or quote owner doesn't exist!");
        let base_owner = base_owner.unwrap();
        let quote_owner = quote_owner.unwrap();

        ensure!(sender == base_owner || sender == quote_owner , "sender is neither owner of base nor owner of quote.");

        let bq = Self::get_trade_pair_hash_by_base_quote((base, quote));
        let qb = Self::get_trade_pair_hash_by_base_quote((quote, base));
        ensure!(!(bq.is_some() && qb.is_some()), "tradepair confilict.");

        let nonce = <Nonce<T>>::get();
        let hash = (base, quote, sender.clone(), nonce, <system::Module<T>>::random_seed()).using_encoded(<T as system::Trait>::Hashing::hash);
        let tp = TradePair{
            hash,base,quote
        };
        <Nonce<T>>::mutate(|n| *n += 1);
        <TradePairByHash<T>>::insert(hash, tp.clone());
        <TradePairHashByBaseQuote<T>>::insert((base, quote), hash);


        Self::deposit_event(RawEvent::TradePairCreated(sender.clone(), hash, base, quote, tp));
        // Self::deposit_event(RawEvent::TradePairCreated(sender.clone(), hash, base, quote));
		Ok(())
	}
    pub fn do_create_limit_order(sender:T::AccountId, base:T::Hash, quote:T::Hash, otype: OrderType, price:T::Price, amount: T::Balance) -> Result{
        let tp = Self::ensure_trade_pair(base, quote)?;
        ensure!(price > Zero::zero(), "price must be positive!");
        let op_token_hash ;
        match otype {
            OrderType::Buy => op_token_hash = base,
            OrderType::Sell=> op_token_hash = quote,
        }

        let order = LimitOrder::<T>::new(base, quote, sender.clone(), price, amount, otype);

        let hash = order.hash;
        <token::Module<T>>::ensure_free_balance(sender.clone(), op_token_hash, amount)?;
        <token::Module<T>>::do_freeze(sender.clone(), op_token_hash, amount)?;
        <Orders<T>>::insert(hash, order);

        let owned_index = Self::owned_order_index(sender.clone());
        <OwnedOrders<T>>::insert((sender.clone(),owned_index),hash);
        <OwnedOrdersIndex<T>>::insert(sender.clone(),owned_index+1);
        let tp_owned_index = Self::trade_pair_owned_order_index(tp);
        <TradePairOwnedOrders<T>>::insert((tp ,tp_owned_index),hash);
        <TradePairOwnedOrdersIndex<T>>::insert(tp,tp_owned_index+1);

        
        Self::deposit_event(RawEvent::OrderCreated(sender, base, quote, hash, price, amount));

        Ok(())
    }

   
}



/// tests for this module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok};
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl Trait for Test {
		type Event = ();
	}
	type token = Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	#[test]
	fn it_works_for_default_value() {
		with_externalities(&mut new_test_ext(), || {
			// Just a dummy test for the dummy funtion `do_something`
			// calling the `do_something` function with a value 42
			assert_ok!(token::do_something(Origin::signed(1), 42));
			// asserting that the stored value is equal to what we stored
			assert_eq!(token::something(), Some(42));
		});
	}
}
