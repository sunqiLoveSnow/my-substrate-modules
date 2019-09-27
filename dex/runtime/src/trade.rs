/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

use support::{decl_module, decl_storage, decl_event, StorageMap, StorageValue, dispatch::Result, ensure, Parameter};
use system::ensure_signed;
use runtime_primitives::traits::{As, Hash, Zero, Member, Bounded, SimpleArithmetic , CheckedSub};
use parity_codec::{Encode, Decode, Codec};
use rstd::prelude::Vec;
use rstd::{result, ops::Not};
use crate::token;
use primitives::U256;
use core::convert::TryInto;


#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
pub struct TradePair<T> where T: Trait {
	hash: T::Hash,
	base: T::Hash,
	quote: T::Hash,
	buy_one_price:Option<T::Price>,
	sell_one_price:Option<T::Price>,
	latest_matched_price:Option<T::Price>,
}

#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq, Copy)]
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
impl Not for OrderType{
	type Output = OrderType;
	fn not(self) -> Self::Output{
		match self{
			OrderType::Sell => OrderType::Buy,
			OrderType::Buy => OrderType::Sell,
		}
	}
}

#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq, Copy)]
pub struct Trade<T> where T:Trait{
	hash:T::Hash,
	base:T::Hash,
	quote:T::Hash,
	buyer: T::AccountId,
	seller: T::AccountId,
	maker:T::AccountId,
	taker:T::AccountId,
	otype:OrderType,
	price: T::Price,
	base_amount: T::Balance,
	quote_amount: T::Balance,
}
#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq, Copy)]
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
    sell_amount: T::Balance,
    buy_amount: T::Balance,
    remained_sell_amount: T::Balance,
    remained_buy_amount: T::Balance,
    otype: OrderType,
    status: OrderStatus,
}

impl<T:Trait> LimitOrder<T>{
    pub fn new (base: T::Hash, 
	quote:T::Hash, 
	owner:T::AccountId, 
	price : T::Price, 
	sell_amount : T::Balance, 
	buy_amount : T::Balance, 
	otype: OrderType) ->Self {
		let nonce = <Nonce<T>>::get();
        let hash = (<system::Module<T>>::random_seed(), 
					<system::Module<T>>::block_number(), 
					base, quote, price , owner.clone(), sell_amount, buy_amount, otype, nonce).using_encoded(<T as system::Trait>::Hashing::hash);
        LimitOrder {
            hash, base, quote, owner, price, otype, 
			sell_amount,
			buy_amount,
            remained_sell_amount:sell_amount,
            remained_buy_amount: buy_amount,
			status: OrderStatus::Created,
        }
        
    }
    fn is_finished(&self) -> bool {
        (self.remained_buy_amount == Zero::zero() && self.status == OrderStatus::Filled)|| self.status == OrderStatus::Canceled
    }

}

impl<T> Trade<T> where T:Trait{
	fn new(base: T::Hash, quote:T::Hash, maker_order:&LimitOrder<T>,taker_order:&LimitOrder<T>,base_amount:T::Balance, quote_amount:T::Balance ) -> Self{
		let nonce = <Nonce<T>>::get();
		let hash = (<system::Module<T>>::random_seed(), <system::Module<T>>::block_number(), nonce, 
			maker_order.hash, maker_order.sell_amount, maker_order.owner.clone(),
			taker_order.hash, taker_order.remained_sell_amount, taker_order.owner.clone()
		).using_encoded(<T as system::Trait>::Hashing::hash);
		
		<Nonce<T>>::mutate(|x| *x +=1);
		let buyer;
		let seller;
		if taker_order.otype == OrderType::Buy{
			buyer = taker_order.owner.clone();
			seller = maker_order.owner.clone();

		}else{
			buyer = maker_order.owner.clone();
			seller = taker_order.owner.clone();

		}
		Trade {
			hash, base, quote, buyer, seller, base_amount, quote_amount, 
			maker:maker_order.owner.clone(),
			taker:taker_order.owner.clone(),
			otype : taker_order.otype,
			price : maker_order.price,
		}
	}
}

#[derive(Debug,Encode, Decode, Clone, PartialEq, Eq)]
pub struct LinkedItem<K1, K2> {
    pub price : Option<K2>,
    pub next: Option<K2>,
    pub prev:Option<K2>,
    pub orders : Vec<K1>,
}


pub struct LinkedList<T, S, K1, K2>(rstd::marker::PhantomData<(T, S, K1, K2)>);

impl<T, S, K1, K2>  LinkedList<T,S, K1,K2> where
	T: Trait,
	K1:Encode + Decode + Clone +  Copy + rstd::borrow::Borrow<<T as system::Trait>::Hash> + PartialEq + Eq,
	K2: Parameter + Member + SimpleArithmetic + Codec + Default + Copy + Bounded ,
	S:StorageMap<(K1, Option<K2>), LinkedItem<K1, K2>, Query = Option<LinkedItem<K1, K2>>>,
{
    pub fn read_bottom(key: K1) ->LinkedItem<K1,K2>{
        Self::read(key, Some(K2::min_value()))
    }
    pub fn read_top(key: K1) ->LinkedItem<K1,K2>{
        Self::read(key, Some(K2::max_value()))
    }
    
    pub fn read_head(key: K1) ->LinkedItem<K1,K2>{
        Self::read(key, None)
    }

    pub fn read(key1: K1, key2: Option<K2>) ->LinkedItem<K1,K2>{
        S::get((key1,key2)).unwrap_or_else(||{
            let bottom = LinkedItem {
                prev: Some(K2::max_value()),
                next: None,
                price: Some(K2::min_value()),
                orders: Vec::new(),
            };
            let top = LinkedItem {
                prev: None,
                next: Some(K2::min_value()),
                price: Some(K2::max_value()),
                orders: Vec::new(),
            };
            let item = LinkedItem {
                prev: Some(K2::min_value()),
                next: Some(K2::max_value()),
                price: None,
                orders: Vec::new(),
            };
            
            Self::write(key1,key2, item.clone());
            Self::write(key1,bottom.price, bottom);
            Self::write(key1,top.price, top);
            item
        })
    }
    pub fn write (key1: K1, key2:Option<K2>, item:LinkedItem<K1,K2>) {
        S::insert((key1, key2), item);
    }
    pub fn append(key1:K1, key2: K2, order_hash: K1, otype: OrderType){
        let item = S::get((key1, Some(key2)));
        match item {
            Some(mut item) => {
                item.orders.push(order_hash);
                Self::write(key1, Some(key2), item);
                return
            },
            None => {
                let start_item;
                let end_item;
                match otype{
                    OrderType::Buy => {
                        start_item = Some(K2::min_value());
                        end_item = None;
                    },
                    OrderType::Sell => {
                        end_item = Some(K2::max_value());
                        start_item = None;
                    },
                    
                }
                // let mut item = Self::read_head(key1);
                let mut item = Self::read(key1,start_item);

                while item.next != end_item{
                    match item.next {
                        None => {},
                        Some(price) => {
                            if key2 < price {
                                break;
                            }
                        }
                    }
                    item = Self::read(key1, item.next);
                }
                // add key2 after item
                let new_prev = LinkedItem{
                    next: Some(key2),
                    ..item
                };
                Self::write(key1, new_prev.price, new_prev.clone());
                let next = Self::read(key1, item.next);
                let new_next = LinkedItem{
                    prev:Some(key2),
                    ..next
                };
                Self::write(key1, new_next.price, new_next.clone());
                let mut v = Vec::new();
                v.push(order_hash);
                let item = LinkedItem{
                    prev: new_prev.price,
                    next: new_next.price,
                    price:Some(key2),
                    orders:v
                };
                Self::write(key1,Some(key2), item);

            }
        }
    }

	pub fn next_match_price(item: &LinkedItem<K1, K2>, otype: OrderType) -> Option<K2> {
		if otype == OrderType::Buy {
			item.prev
		} else {
			item.next
		}
	}
    
    pub fn remove_all(key1:K1, otype : OrderType) -> Result {
        let end_item ;

        if otype == OrderType::Buy{
            end_item = Some(K2::min_value());
        }else{
            end_item = Some(K2::max_value());
        }
        let mut head = Self::read_head(key1);
        loop{
            let key2 = Self::next_match_price(&head, otype);
            
            if key2 == end_item {
                break;
            }
            Self::remove_orders_in_one_item(key1, key2.unwrap())?;
            head = Self::read_head(key1);

        } 

        Ok(())
    }
	pub fn remove_item(key1: K1, key2: K2) {
		if let Some(item) = S::take((key1,Some(key2))){
			S::mutate((key1.clone(), item.prev), |x| {
				if let Some(x) = x{
					x.next = item.next;
				}
			});
			S::mutate((key1.clone(), item.next), |x| {
				if let Some(x) = x{
					x.prev = item.prev;
				}
			});
		}
		
	}
	fn remove_order(key1: K1, key2: K2, order_hash: K1)  -> Result {
		match S::get((key1, Some(key2))) {
			Some(mut item) => {
				ensure!(item.orders.contains(&order_hash), "cancel the order but not in market order list");
				item.orders.retain(|&x| x != order_hash);
				Self::write(key1, Some(key2), item.clone());
				if item.orders.len() == 0 {
                    Self::remove_item(key1, key2);
                }
			},
			_ => {}
		}
		Ok(())
	}
    pub fn remove_orders_in_one_item(key1:K1, key2: K2) -> Result {
        match S::get((key1, Some(key2))){
            Some(mut item) => {
                while item.orders.len()> 0 {
                    let order_hash = item.orders.get(0).ok_or("can not get order hash")?;
                    let order = <Orders<T>>::get(order_hash.borrow()).ok_or("can not get order")?;
                    ensure!(order.is_finished(), "try to remove not finished order");
                    item.orders.remove(0);
                    Self::write(key1, Some(key2), item.clone());
                }
                if item.orders.len() == 0 {
                    Self::remove_item(key1, key2);
                }
            },
            None => {

            }
        }

        Ok(())
    }

}
/// The module's configuration trait.
pub trait Trait:  token::Trait + system::Trait {
	// TODO: Add other types and constants required configure this module.

	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	// type Price: Parameter + Member + Default + Codec +  Bounded + SimpleArithmetic + Copy;
	type Price: Parameter + Member + SimpleArithmetic + Codec + Default + Copy  + From<u64> + Into<u64>;
}



type OrderLinkedItem<T> = LinkedItem<<T as system::Trait>::Hash, <T as Trait>::Price>;
type OrderLinkedItemList<T> = LinkedList<T,LinkedItemList<T>,<T as system::Trait>::Hash, <T as Trait>::Price>;
/// This module's storage items.
decl_storage! {
	trait Store for Module<T: Trait> as trade{
		// TradePairsByHash ==> TradePair
		TradePairsByHash get(trade_pair_by_hash): map T::Hash => Option<TradePair<T>>;
        // (BaseTokenHash, QuoteTokenHash) ==> TradePairHash
        TradePairHashByBaseQuote get(get_trade_pair_hash_by_base_quote) : map (T::Hash, T::Hash) => Option<T::Hash>;
        Orders get(order) :map T::Hash => Option<LimitOrder<T>> ;
        OwnedOrders get(owned_order): map( T::AccountId, u64) => Option<T::Hash> ;
        OwnedOrdersIndex get(owned_order_index):map T::AccountId => u64;
        
        TradePairOwnedOrders get(trade_pair_owned_order):map (T::Hash, u64)=> Option<T::Hash>;
        TradePairOwnedOrdersIndex get(trade_pair_owned_order_index):map T::Hash => u64;
        Nonce: u64;
        LinkedItemList get(sell_order):map (T::Hash, Option<T::Price>) => Option<OrderLinkedItem<T>>;
		// trade hash => trade
		Trades get(trade) : map T::Hash => Option<Trade<T>>;
		// Accountid => Vec<trade hash>
		OwnedTrades get(owned_trade) : map T::AccountId => Option<Vec<T::Hash>>;
		// tradepair hash => vec<trade pair>
		TradePairOwnedTrades get(trade_pair_owned_trade): map T::Hash => Option<Vec<T::Hash>>;
		// order hash => vec<trade pair>
		OrderOwnedTrades get(order_owned_trade): map T::Hash => Option<Vec<T::Hash>>;
		// account id tradepairhash) => vec<trade hash>
		OwnedTPTrades get(owned_trade_pair_trade): map (T::AccountId, T::Hash) => Option<Vec<T::Hash>>;
		//
		// OwnedTrades get(owned_trade): map T::AccountId => Option<Vec<T::Hash>>;

	}
}

impl <T> OrderOwnedTrades<T> where T: Trait{
	fn add_trade(order_hash: T::Hash, trade_hash: T::Hash){
		let mut trades ;
		if let Some(ts) = Self::get(order_hash){
			trades = ts;
		}else {
			trades = Vec::new();
		}
		trades.push(trade_hash);
		Self::insert(order_hash, trades);
	}
}

impl <T> TradePairOwnedTrades<T> where T: Trait{
	fn add_trade(tp_hash: T::Hash, trade_hash: T::Hash){
		let mut trades ;
		if let Some(ts) = Self::get(tp_hash){
			trades = ts;
		}else {
			trades = Vec::new();
		}
		trades.push(trade_hash);
		Self::insert(tp_hash, trades);
	}
}


impl <T> OwnedTrades<T> where T: Trait{
	fn add_trade(account_id: T::AccountId, trade_hash: T::Hash){
		let mut trades ;
		if let Some(ts) = Self::get(account_id.clone()){
			trades = ts;
		}else {
			trades = Vec::new();
		}
		trades.push(trade_hash);
		Self::insert(account_id, trades);
	}
}


impl <T> OwnedTPTrades<T> where T: Trait{
	fn add_trade(account_id: T::AccountId, tp_hash :T::Hash,trade_hash: T::Hash){
		let mut trades ;
		if let Some(ts) = Self::get((account_id.clone(), tp_hash)){
			trades = ts;
		}else {
			trades = Vec::new();
		}
		trades.push(trade_hash);
		Self::insert((account_id, tp_hash), trades);
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
		pub fn cancel_limit_order(origin, order_hash : T::Hash ) ->Result{
			let sender = ensure_signed(origin)?;
			Self::do_cancel_limit_order(sender, order_hash)
		}

	}
}

decl_event!(
	pub enum Event<T> where  
    <T as system::Trait>::AccountId,
    <T as system::Trait>::Hash,
    <T as Trait>::Price,
    <T as balances::Trait>::Balance,
    // TradePair = TradePair<<T as system::Trait>::Hash>,
    TradePair = TradePair<T>,
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
const price_factor:u64 = 100_000_000u64;



impl<T: Trait> Module<T>{
	fn ensure_bonds(price:T::Price, sell_amount:T::Balance) -> Result {
		ensure!(price > Zero::zero() && price <= T::Price::max_value(), "price bounds check failed");
		ensure!(sell_amount > Zero::zero() && sell_amount <= T::Balance::max_value(), "sell acmount bounds check failed");
		Ok(())
	}
	fn ensure_counterparty_amount_bounds(otype: OrderType, price:T::Price, amount:T::Balance) -> result::Result<T::Balance, &'static str>{
		let price_256 = U256::from(price.as_());
		let amount_256 = U256::from(amount.as_());
		let price_factor_256 = U256::from(price_factor);
		
		let amount_v2 :U256;
		let counterparty_amount :U256;

		match otype{
			OrderType::Buy => {
				counterparty_amount = amount_256 * price_factor_256 / price_256;
				amount_v2 = counterparty_amount * price_256/price_factor_256;

			},
			OrderType::Sell => {
				counterparty_amount = amount_256 * price_256 / price_factor_256;
				amount_v2 = counterparty_amount * price_factor_256/ price_256;
			}
		}
		if amount_v2 != amount_256{
			return Err("amount have digits parts")
		}
		if counterparty_amount == 0u32.into() || counterparty_amount > T::Balance::max_value().as_().into(){
			return Err("counterparty bound check failed")
		}
		let result :u64 = counterparty_amount.try_into().map_err(|_| "Overflow error")?;
		Ok(<T as balances::Trait>::Balance::sa(result))
	}
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
            hash,base,quote,
			buy_one_price: None,
			sell_one_price: None,
			latest_matched_price: None,
        };
        <Nonce<T>>::mutate(|n| *n += 1);
        <TradePairsByHash<T>>::insert(hash, tp.clone());
        <TradePairHashByBaseQuote<T>>::insert((base, quote), hash);


        Self::deposit_event(RawEvent::TradePairCreated(sender.clone(), hash, base, quote, tp));
        // Self::deposit_event(RawEvent::TradePairCreated(sender.clone(), hash, base, quote));
		Ok(())
	}

    pub fn do_create_limit_order(sender:T::AccountId, base:T::Hash, quote:T::Hash, otype: OrderType, price:T::Price, sell_amount: T::Balance) -> Result{
        // let tp = Self::ensure_trade_pair(base, quote)?;
        // ensure!(price > Zero::zero(), "price must be positive!");
		Self::ensure_bonds(price, sell_amount);
		let buy_amount = Self::ensure_counterparty_amount_bounds(otype, price, sell_amount)?;
		let tp_hash = Self::ensure_trade_pair(base, quote)?;
        let op_token_hash ;
        match otype {
            OrderType::Buy => op_token_hash = base,
            OrderType::Sell=> op_token_hash = quote,
        }

        let order = LimitOrder::<T>::new(base, quote, sender.clone(), price, sell_amount, buy_amount, otype);

        let hash = order.hash;
        <token::Module<T>>::ensure_free_balance(sender.clone(), op_token_hash, sell_amount)?;
        <token::Module<T>>::do_freeze(sender.clone(), op_token_hash, sell_amount)?;
        <Orders<T>>::insert(hash, order.clone());

		<Nonce<T>>::mutate(|n| *n += 1);

        let owned_index = Self::owned_order_index(sender.clone());
        <OwnedOrders<T>>::insert((sender.clone(),owned_index),hash);
        <OwnedOrdersIndex<T>>::insert(sender.clone(),owned_index + 1);

        let tp_owned_index = Self::trade_pair_owned_order_index(tp_hash);
        <TradePairOwnedOrders<T>>::insert((tp_hash ,tp_owned_index),hash);
        <TradePairOwnedOrdersIndex<T>>::insert(tp_hash, tp_owned_index + 1);

		let filled = Self::order_match(tp_hash, order.clone())?;

		if !filled{
			<OrderLinkedItemList<T>>::append(tp_hash, price, hash, otype);
			Self::set_tp_buy_one_or_sell_one_price(tp_hash, otype)?;
		}
        
        Self::deposit_event(RawEvent::OrderCreated(sender, base, quote, hash, price, sell_amount));

        Ok(())
    }
	pub fn do_cancel_limit_order(sender: T::AccountId, order_hash : T::Hash ) ->Result{
		let order = Self::order(order_hash).ok_or("can not get order")?;
		ensure!(order.owner == sender, "can only cancel your owned order");
		ensure!(!order.is_finished(), "can not cancel finished order");
		let tp_hash = Self::ensure_trade_pair(order.base, order.quote)?;
		<OrderLinkedItemList<T>>::remove_order(tp_hash, order.price, order.hash)?;
		Ok(())
	}

	fn order_match(tp_hash:T::Hash, mut order: LimitOrder<T>) -> result::Result<bool, &'static str>{
		let mut head = <OrderLinkedItemList<T>>::read_head(tp_hash);
		let end_item_price;
		let otype = order.otype;
		let oprice = order.price;
		if otype == OrderType::Buy{
			end_item_price = Some(T::Price::min_value());
		}else{
			end_item_price = Some(T::Price::max_value());
		}
		let tp = Self::trade_pair_by_hash(tp_hash).ok_or("can not get trade pair")?;
		let give :T::Hash;
		let have :T::Hash;
		match otype{
			OrderType::Buy => {
				give = tp.base;
				have = tp.quote;
			},
			OrderType::Sell => {
				have = tp.base;
				give = tp.quote;
			},
		};
		loop {
			if order.status == OrderStatus::Filled{
				break;
			}
			let item_price = Self::next_match_price(&head, !otype);
			if item_price == end_item_price {
				break;
			}
			let item_price = item_price.ok_or("can not get item price")?;
			if !Self::price_match(oprice, otype, item_price){
				break;
			}
			let item = <LinkedItemList<T>>::get((tp_hash, Some(item_price))).ok_or("can not unwrap linked list item")?;
			for o in item.orders.iter(){
				let mut o = Self::order(o).ok_or("can not get order")?;
				// let ex_amount = order.remained_sell_amount.min(o.remained_sell_amount);
				let (base_qty, quote_qty) = Self::calculate_ex_amount(&o, &order)?;
				
				let give_qty: T::Balance;
				let have_qty: T::Balance;

				match otype{
					OrderType::Buy => {
						give_qty = base_qty;
						have_qty = quote_qty;
					},
					OrderType::Sell => {
						have_qty = base_qty;
						give_qty = quote_qty;
					},
				}
				if order.status == OrderStatus::Created {
					order.status = OrderStatus::PartialFilled;
				}
				if o.status == OrderStatus::Created {
					o.status = OrderStatus::PartialFilled;
				}

				<token::Module<T>>::do_unfreeze(order.owner.clone(), give, give_qty)?;
				<token::Module<T>>::do_unfreeze(o.owner.clone(), have, have_qty)?;
				
				<token::Module<T>>::do_transfer(order.owner.clone(), o.owner.clone(), give, give_qty);
				<token::Module<T>>::do_transfer(o.owner.clone(), order.owner.clone(), have, have_qty);
				
				order.remained_sell_amount = order.remained_sell_amount.checked_sub(&give_qty).ok_or("substract error")?;
				o.remained_sell_amount = o.remained_sell_amount.checked_sub(&have_qty).ok_or("substract error")?;
				
				order.remained_buy_amount = order.remained_buy_amount.checked_sub(&have_qty).ok_or("substract error")?;
				o.remained_buy_amount = o.remained_buy_amount.checked_sub(&give_qty).ok_or("substract error")?;

				if order.remained_buy_amount == Zero::zero(){
					order.status = OrderStatus::Filled;
					if order.remained_sell_amount != Zero::zero(){
						<token::Module<T>>::do_unfreeze(order.owner.clone(), give, order.remained_sell_amount)?;
						order.remained_sell_amount = Zero::zero();
					}
				}
				if o.remained_buy_amount == Zero::zero(){
					o.status = OrderStatus::Filled;
					if o.remained_sell_amount != Zero::zero(){
						<token::Module<T>>::do_unfreeze(o.owner.clone(), have, order.remained_sell_amount)?;
						o.remained_sell_amount = Zero::zero();
					}
				}
				<Orders<T>>::insert(order.hash.clone(), order.clone());
				<Orders<T>>::insert(o.hash.clone(), o.clone());
				Self::set_to_latest_matched_price(tp_hash, o.price)?;
				<OrderLinkedItemList<T>>::remove_all(tp_hash, !otype);
				Self::set_tp_buy_one_or_sell_one_price(tp_hash, o.otype)?;


				let trade = Trade::new(tp.base, tp.quote, &o, &order, base_qty, quote_qty);
				<Trades<T>>::insert(trade.hash, trade.clone());
				//order owned trades, 2
				<OrderOwnedTrades<T>>::add_trade(o.hash, trade.hash);
				<OrderOwnedTrades<T>>::add_trade(order.hash, trade.hash);
				// account owned trades, 2
				<OwnedTrades<T>>::add_trade(order.owner.clone(), trade.hash);
				<OwnedTrades<T>>::add_trade(o.owner.clone(), trade.hash);
				// account + tradepair_hash owned trades, 2
				<OwnedTPTrades<T>>::add_trade(order.owner.clone(), tp_hash, trade.hash);
				<OwnedTPTrades<T>>::add_trade(o.owner.clone(), tp_hash, trade.hash);
				// tradepair owned trades, 1
				<TradePairOwnedTrades<T>>::add_trade(tp_hash, trade.hash);

				if order.status == OrderStatus::Filled{
					break
				}
				

			}
			// head = <OrderLinkedItemList<T>>::read(tp_hash, Some(item_price));
			head = <OrderLinkedItemList<T>>::read_head(tp_hash);
		}
		// <OrderLinkedItemList<T>>::remove_all(tp_hash, !otype);
		if order.status == OrderStatus::Filled{
			Ok(true)
		}else{
			Ok(false)
		}
		
	}

	fn set_to_latest_matched_price (tp_hash: T::Hash, price:T::Price) ->Result {
		let mut tp = <TradePairsByHash<T>>::get(tp_hash).ok_or("can not get trade pair")?;
		tp.latest_matched_price  = Some(price);
		<TradePairsByHash<T>>::insert(tp_hash,tp);
		Ok(())
	}

	fn set_tp_buy_one_or_sell_one_price(tp_hash:T::Hash, otype: OrderType) -> Result {
		let mut tp = <TradePairsByHash<T>>::get(tp_hash).ok_or("can not get trade pair")?;
		let head = <OrderLinkedItemList<T>>::read_head(tp_hash);
		if otype == OrderType::Buy{
			if head.prev == Some(T::Price::min_value()){
				tp.buy_one_price = None;
			}else{
				tp.buy_one_price = head.prev;
			}
		}else{
			if head.next == Some(T::Price::max_value()){
				tp.sell_one_price = None;
			}else{
				tp.sell_one_price = head.next;
			}
		}
		<TradePairsByHash<T>>::insert(tp_hash, tp);
		Ok(())

	}
	fn calculate_ex_amount(maker_order: &LimitOrder<T>, taker_order: &LimitOrder<T>) -> result::Result<(T::Balance,T::Balance), &'static str>{
		let buyer_order;
		let seller_order;
		if taker_order.otype == OrderType::Buy{
			buyer_order = taker_order;
			seller_order = maker_order;
		} else{
			buyer_order = maker_order;
			seller_order = taker_order;
		}
		if seller_order.remained_buy_amount <= buyer_order.remained_sell_amount{
			let mut quote_qty :u64 = seller_order.remained_buy_amount.as_() * price_factor /maker_order.price.into();
			let buy_amount_v2 = quote_qty * maker_order.price.into() / price_factor;
			if buy_amount_v2 != seller_order.remained_buy_amount.as_() {
				// seller need to give more to align
				quote_qty = quote_qty +1;

			}
			return Ok((seller_order.remained_buy_amount, <T::Balance as As<u64>>::sa(quote_qty)))
		}else if buyer_order.remained_buy_amount <= seller_order.remained_sell_amount{
			let mut base_qty :u64 = buyer_order.remained_buy_amount.as_() * maker_order.price.into() / price_factor;
			let buy_amount_v2 = base_qty* price_factor / maker_order.price.into() ;
			if buy_amount_v2 != buyer_order.remained_buy_amount.as_() {
				// seller need to give more to align
				base_qty = base_qty + 1;

			}
			return Ok((<T::Balance as As<u64>>::sa(base_qty), buyer_order.remained_buy_amount))
		}
		Err("should never executed here")

	}
	fn next_match_price(item:&OrderLinkedItem<T>, otype:OrderType)-> Option<T::Price>{
	// fn next_match_price(item:&LinkedItem<T::Hash, T::Price>, otype:OrderType)-> Option<T::Price>{
		if otype == OrderType::Buy{
			item.prev
		}else {
			item.next
		}
	}
	fn price_match(order_price:T::Price, order_type:OrderType,linked_item_price:T::Price) -> bool{
		match order_type{
			OrderType::Buy => order_price >= linked_item_price,
			OrderType::Sell => order_price <= linked_item_price,
		}
	}
}





/// tests for this module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok, assert_err};
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
	#[derive(Clone, Eq, PartialEq, Debug)]
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

	impl balances::Trait for Test {
		type Balance = u128;

		type OnFreeBalanceZero = ();

		type OnNewAccount = ();

		type Event = ();

		type TransactionPayment = ();
		type DustRemoval = ();
		type TransferPayment = ();
	}

	impl token::Trait for Test {
		type Event = ();
	}

	impl super::Trait for Test {
		type Event = ();
		type Price = u64;
	}

	type TokenModule = token::Module<Test>;
	type TradeModule = super::Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	fn output_order(tp_hash: <Test as system::Trait>::Hash) {

		let mut item = <OrderLinkedItemList<Test>>::read_bottom(tp_hash);

		println!("[Market Orders]");

		loop {
			if item.price == Some(<Test as Trait>::Price::min_value()) {
				print!("Bottom ==> ");
			} else if item.price == Some(<Test as Trait>::Price::max_value()) {
				print!("Top ==> ");
			} else if item.price == None {
				print!("Head ==> ");
			}

			print!("Price({:?}), Next({:?}), Prev({:?}), Orders({}): ", item.price, item.next, item.prev, item.orders.len());

			let mut orders = item.orders.iter();
			loop {
				match orders.next() {
					Some(order_hash) => {
						let order = <Orders<Test>>::get(order_hash).unwrap();
						print!("({}@[{:?}]: Sell[{}, {}], Buy[{}, {}]), ", order.hash, order.status, 
							order.sell_amount, order.remained_sell_amount, order.buy_amount, order.remained_buy_amount);
					},
					None => break,
				}
			}

			println!("");

			if item.next == Some(<Test as Trait>::Price::min_value()) {
				break;
			} else {
				item = OrderLinkedItemList::<Test>::read(tp_hash, item.next);
			}
		}

		println!("[Market Trades]");

		let trades = TradeModule::trade_pair_owned_trade(tp_hash);
		if let Some(trades) = trades {
			for hash in trades.iter() {
				let trade = <Trades<Test>>::get(hash).unwrap();
				println!("[{}/{}] - {}@{}[{:?}]: [Buyer,Seller][{},{}], [Maker,Taker][{},{}], [Base,Quote][{}, {}]", 
					trade.quote, trade.base, hash, trade.price, trade.otype, trade.buyer, trade.seller, trade.maker, 
					trade.taker, trade.base_amount, trade.quote_amount);
			}
		}

		println!("[Trade Pair Data]");
		let tp = TradeModule::trade_pair_by_hash(tp_hash).unwrap();
		println!("buy one: {:?}, sell one: {:?}, latest matched price: {:?}", tp.buy_one_price, tp.sell_one_price, tp.latest_matched_price);

		println!();
	}

	#[test]
	fn linked_list_test_case() {
		with_externalities(&mut new_test_ext(), || {
			let alice = 10;
			let bob = 20;

			let max = Some(<Test as Trait>::Price::max_value());
			let min = Some(<Test as Trait>::Price::min_value());

			// token1
			assert_ok!(TokenModule::issue(Origin::signed(alice), b"66".to_vec(), 21000000));
			let token1_hash = TokenModule::owned_token((alice, 0)).unwrap();
			let token1 = TokenModule::token(token1_hash).unwrap();

			// token2
			assert_ok!(TokenModule::issue(Origin::signed(bob), b"77".to_vec(), 10000000));
			let token2_hash = TokenModule::owned_token((bob, 0)).unwrap();
			let token2 = TokenModule::token(token2_hash).unwrap();

			// tradepair
			let base = token1.hash;
			let quote = token2.hash;
			assert_ok!(TradeModule::create_trade_pair(Origin::signed(alice), base, quote));
			let tp_hash = TradeModule::get_trade_pair_hash_by_base_quote((base, quote)).unwrap();

			let bottom = OrderLinkedItem::<Test> {
				prev: max,
				next: None,
				price: min,
				orders: Vec::new(),
			};

			let top = OrderLinkedItem::<Test> {
				prev: None,
				next: min,
				price: max,
				orders: Vec::new(),
			};

			let head = OrderLinkedItem::<Test> {
				prev: min,
				next: max,
				price: None,
				orders: Vec::new(),
			};

			assert_eq!(head, <OrderLinkedItemList<Test>>::read_head(tp_hash));
			assert_eq!(bottom, <OrderLinkedItemList<Test>>::read_bottom(tp_hash));
			assert_eq!(top, <OrderLinkedItemList<Test>>::read_top(tp_hash));

			// Bottom ==> Price(Some(0)), Next(None), Prev(Some(18446744073709551615)), Orders(0): 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(0)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			// sell limit order
			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 180_000_000, 100));
			let order1_hash = TradeModule::owned_order((bob, 0)).unwrap();
			let mut order1 = TradeModule::order(order1_hash).unwrap();
			assert_eq!(order1.sell_amount, 100);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 100_000_000, 50));
			let order2_hash = TradeModule::owned_order((bob, 1)).unwrap();
			let mut order2 = TradeModule::order(order2_hash).unwrap();
			assert_eq!(order2.sell_amount, 50);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 50_000_000, 10));
			let order3_hash = TradeModule::owned_order((bob, 2)).unwrap();
			let mut order3 = TradeModule::order(order3_hash).unwrap();
			assert_eq!(order3.sell_amount, 10);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 50_000_000, 20));
			let order4_hash = TradeModule::owned_order((bob, 3)).unwrap();
			let mut order4 = TradeModule::order(order4_hash).unwrap();
			assert_eq!(order4.sell_amount, 20);
			
			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 120_000_000, 10));
			let order5_hash = TradeModule::owned_order((bob, 4)).unwrap();
			let mut order5 = TradeModule::order(order5_hash).unwrap();
			assert_eq!(order5.sell_amount, 10);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 120_000_000, 30));
			let order6_hash = TradeModule::owned_order((bob, 5)).unwrap();
			let mut order6 = TradeModule::order(order6_hash).unwrap();
			assert_eq!(order6.sell_amount, 30);
			
			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 120_000_000, 20));
			let order7_hash = TradeModule::owned_order((bob, 6)).unwrap();
			let mut order7 = TradeModule::order(order7_hash).unwrap();
			assert_eq!(order7.sell_amount, 20);

			// buy limit order
			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 20_000_000, 5));
			let order101_hash = TradeModule::owned_order((alice, 0)).unwrap();
			let mut order101 = TradeModule::order(order101_hash).unwrap();
			assert_eq!(order101.sell_amount, 5);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 10_000_000, 12));
			let order102_hash = TradeModule::owned_order((alice, 1)).unwrap();
			let mut order102 = TradeModule::order(order102_hash).unwrap();
			assert_eq!(order102.sell_amount, 12);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 40_000_000, 100));
			let order103_hash = TradeModule::owned_order((alice, 2)).unwrap();
			let mut order103 = TradeModule::order(order103_hash).unwrap();
			assert_eq!(order103.sell_amount, 100);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 20_000_000, 1000000));
			let order104_hash = TradeModule::owned_order((alice, 3)).unwrap();
			let mut order104 = TradeModule::order(order104_hash).unwrap();
			assert_eq!(order104.sell_amount, 1000000);

			// head
			let mut item = OrderLinkedItem::<Test> {
				next: Some(50000000),
				prev: Some(40000000),
				price: None,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_head(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order3_hash);
			v.push(order4_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(100000000),
				prev: None,
				price: Some(50000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();
			v.push(order2_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(120000000),
				prev: Some(50000000),
				price: Some(100000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item3
			curr = item.next;
			
			v = Vec::new();
			v.push(order5_hash);
			v.push(order6_hash);
			v.push(order7_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(180000000),
				prev: Some(100000000),
				price: Some(120000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item4
			curr = item.next;

			v = Vec::new();
			v.push(order1_hash);

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(120000000),
				price: Some(180000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// top
			item = OrderLinkedItem::<Test> {
				next: min,
				prev: Some(180000000),
				price: max,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_top(tp_hash), item);

			// bottom
			item = OrderLinkedItem::<Test> {
				next: Some(10000000),
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order102_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(20000000),
				prev: min,
				price: Some(10000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();
			v.push(order101_hash);
			v.push(order104_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(40000000),
				prev: Some(10000000),
				price: Some(20000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item3
			curr = item.next;
			
			v = Vec::new();
			v.push(order103_hash);

			item = OrderLinkedItem::<Test> {
				next: None,
				prev: Some(20000000),
				price: Some(40000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// remove sell orders
			OrderLinkedItemList::<Test>::remove_all(tp_hash, OrderType::Sell);
			OrderLinkedItemList::<Test>::remove_all(tp_hash, OrderType::Buy);

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(10000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(10000000)), Next(Some(20000000)), Prev(Some(0)), Orders(1): (0xcc83…6014@[Created]: Sell[12, 12], Buy[120, 120]), 
			// Price(Some(20000000)), Next(Some(40000000)), Prev(Some(10000000)), Orders(2): (0xaaf7…d69c@[Created]: Sell[5, 5], Buy[25, 25]), (0xacd0…2c16@[Created]: Sell[1000000, 1000000], Buy[5000000, 5000000]), 
			// Price(Some(40000000)), Next(None), Prev(Some(20000000)), Orders(1): (0xe873…85c6@[Created]: Sell[100, 100], Buy[250, 250]), 
			// Head ==> Price(None), Next(Some(50000000)), Prev(Some(40000000)), Orders(0): 
			// Price(Some(50000000)), Next(Some(100000000)), Prev(None), Orders(2): (0x4654…32af@[Created]: Sell[10, 10], Buy[5, 5]), (0x4354…4299@[Created]: Sell[20, 20], Buy[10, 10]), 
			// Price(Some(100000000)), Next(Some(120000000)), Prev(Some(50000000)), Orders(1): (0x315b…c4dd@[Created]: Sell[50, 50], Buy[50, 50]), 
			// Price(Some(120000000)), Next(Some(180000000)), Prev(Some(100000000)), Orders(3): (0x0652…61de@[Created]: Sell[10, 10], Buy[12, 12]), (0x4262…0178@[Created]: Sell[30, 30], Buy[36, 36]), (0x3d06…2344@[Created]: Sell[20, 20], Buy[24, 24]), 
			// Price(Some(180000000)), Next(Some(18446744073709551615)), Prev(Some(120000000)), Orders(1): (0x92bd…d0ff@[Created]: Sell[100, 100], Buy[180, 180]), 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(Some(180000000)), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			// price = 5
			order3.remained_buy_amount = Zero::zero();
			order3.status = OrderStatus::Filled;
			<Orders<Test>>::insert(order3.hash, order3);

			order4.status = OrderStatus::Canceled;
			<Orders<Test>>::insert(order4.hash, order4);

			// price = 10
			order2.remained_buy_amount = Zero::zero();
			order2.status = OrderStatus::Filled;
			<Orders<Test>>::insert(order2.hash, order2);

			// price = 12
			order5.status = OrderStatus::Canceled;
			<Orders<Test>>::insert(order5.hash, order5);

			order6.remained_buy_amount = order6.remained_buy_amount.checked_sub(1).unwrap();
			order6.status = OrderStatus::PartialFilled;
			<Orders<Test>>::insert(order6.hash, order6.clone());

			OrderLinkedItemList::<Test>::remove_all(tp_hash, OrderType::Sell);

			// head
			item = OrderLinkedItem::<Test> {
				next: Some(120000000),
				prev: Some(40000000),
				price: None,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_head(tp_hash), item);

			// item1
			curr = item.next;
			
			v = Vec::new();
			v.push(order6_hash);
			v.push(order7_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(180000000),
				prev: None,
				price: Some(120000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();
			v.push(order1_hash);

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(120000000),
				price: Some(180000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			<OrderLinkedItemList<Test>>::remove_all(tp_hash, OrderType::Sell);

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(10000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(10000000)), Next(Some(20000000)), Prev(Some(0)), Orders(1): (0xcc83…6014@[Created]: Sell[12, 12], Buy[120, 120]), 
			// Price(Some(20000000)), Next(Some(40000000)), Prev(Some(10000000)), Orders(2): (0xaaf7…d69c@[Created]: Sell[5, 5], Buy[25, 25]), (0xacd0…2c16@[Created]: Sell[1000000, 1000000], Buy[5000000, 5000000]), 
			// Price(Some(40000000)), Next(None), Prev(Some(20000000)), Orders(1): (0xe873…85c6@[Created]: Sell[100, 100], Buy[250, 250]), 
			// Head ==> Price(None), Next(Some(120000000)), Prev(Some(40000000)), Orders(0): 
			// Price(Some(120000000)), Next(Some(180000000)), Prev(None), Orders(2): (0x4262…0178@[PartialFilled]: Sell[30, 29], Buy[36, 36]), (0x3d06…2344@[Created]: Sell[20, 20], Buy[24, 24]), 
			// Price(Some(180000000)), Next(Some(18446744073709551615)), Prev(Some(120000000)), Orders(1): (0x92bd…d0ff@[Created]: Sell[100, 100], Buy[180, 180]), 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(Some(180000000)), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			// price = 18
			order1.status = OrderStatus::Canceled;
			<Orders<Test>>::insert(order1.hash, order1);

			// price = 12
			order6.remained_buy_amount = Zero::zero();
			order6.status = OrderStatus::Filled;
			<Orders<Test>>::insert(order6.hash, order6);

			order7.remained_buy_amount = Zero::zero();
			order7.status = OrderStatus::Filled;
			<Orders<Test>>::insert(order7.hash, order7);

			<OrderLinkedItemList<Test>>::remove_all(tp_hash, OrderType::Sell);

			// head
			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(40000000),
				price: None,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_head(tp_hash), item);

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(10000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(10000000)), Next(Some(20000000)), Prev(Some(0)), Orders(1): (0xcc83…6014@[Created]: Sell[12, 12], Buy[120, 120]), 
			// Price(Some(20000000)), Next(Some(40000000)), Prev(Some(10000000)), Orders(2): (0xaaf7…d69c@[Created]: Sell[5, 5], Buy[25, 25]), (0xacd0…2c16@[Created]: Sell[1000000, 1000000], Buy[5000000, 5000000]), 
			// Price(Some(40000000)), Next(None), Prev(Some(20000000)), Orders(1): (0xe873…85c6@[Created]: Sell[100, 100], Buy[250, 250]), 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(40000000)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			// remove buy orders
			// price = 4
			order103.remained_buy_amount = Zero::zero();
			order103.status = OrderStatus::Filled;
			<Orders<Test>>::insert(order103.hash, order103);

			// price = 2
			order101.status = OrderStatus::Canceled;
			<Orders<Test>>::insert(order101.hash, order101);

			order104.remained_buy_amount = order104.remained_buy_amount.checked_sub(100).unwrap();
			order104.status = OrderStatus::PartialFilled;
			<Orders<Test>>::insert(order104.hash, order104.clone());

			<OrderLinkedItemList<Test>>::remove_all(tp_hash, OrderType::Buy);

			// bottom
			item = OrderLinkedItem::<Test> {
				next: Some(10000000),
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order102_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(20000000),
				prev: min,
				price: Some(10000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();
			v.push(order104_hash);

			item = OrderLinkedItem::<Test> {
				next: None,
				prev: Some(10000000),
				price: Some(20000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(10000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(10000000)), Next(Some(20000000)), Prev(Some(0)), Orders(1): (0xcc83…6014@[Created]: Sell[12, 12], Buy[120, 120]), 
			// Price(Some(20000000)), Next(None), Prev(Some(10000000)), Orders(1): (0xacd0…2c16@[PartialFilled]: Sell[1000000, 999900], Buy[5000000, 5000000]), 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(20000000)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			// price = 2
			order104.status = OrderStatus::Canceled;
			<Orders<Test>>::insert(order104.hash, order104);

			// price = 1
			order102.remained_buy_amount = Zero::zero();
			order102.status = OrderStatus::Filled;
			<Orders<Test>>::insert(order102.hash, order102);

			<OrderLinkedItemList<Test>>::remove_all(tp_hash, OrderType::Buy);

			let bottom = OrderLinkedItem::<Test> {
				prev: max,
				next: None,
				price: min,
				orders: Vec::new(),
			};

			let top = OrderLinkedItem::<Test> {
				prev: None,
				next: min,
				price: max,
				orders: Vec::new(),
			};

			let head = OrderLinkedItem::<Test> {
				prev: min,
				next: max,
				price: None,
				orders: Vec::new(),
			};

			assert_eq!(head, <OrderLinkedItemList<Test>>::read_head(tp_hash));
			assert_eq!(bottom, <OrderLinkedItemList<Test>>::read_bottom(tp_hash));
			assert_eq!(top, <OrderLinkedItemList<Test>>::read_top(tp_hash));			

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(None), Prev(Some(18446744073709551615)), Orders(0): 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(0)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);
		});
	}

	#[test]
	fn order_match_linked_list_test_case() {
		with_externalities(&mut new_test_ext(), || {
			let alice = 10;
			let bob = 20;

			let max = Some(<Test as Trait>::Price::max_value());
			let min = Some(<Test as Trait>::Price::min_value());

			// token1
			assert_ok!(TokenModule::issue(Origin::signed(alice), b"66".to_vec(), 21000000));
			let token1_hash = TokenModule::owned_token((alice, 0)).unwrap();
			let token1 = TokenModule::token(token1_hash).unwrap();

			// token2
			assert_ok!(TokenModule::issue(Origin::signed(bob), b"77".to_vec(), 10000000));
			let token2_hash = TokenModule::owned_token((bob, 0)).unwrap();
			let token2 = TokenModule::token(token2_hash).unwrap();

			// tradepair
			let base = token1.hash;
			let quote = token2.hash;
			assert_ok!(TradeModule::create_trade_pair(Origin::signed(alice), base, quote));
			let tp_hash = TradeModule::get_trade_pair_hash_by_base_quote((base, quote)).unwrap();

			let bottom = OrderLinkedItem::<Test> {
				prev: max,
				next: None,
				price: min,
				orders: Vec::new(),
			};

			let top = OrderLinkedItem::<Test> {
				prev: None,
				next: min,
				price: max,
				orders: Vec::new(),
			};

			let head = OrderLinkedItem::<Test> {
				prev: min,
				next: max,
				price: None,
				orders: Vec::new(),
			};

			assert_eq!(head, <OrderLinkedItemList<Test>>::read_head(tp_hash));
			assert_eq!(bottom, <OrderLinkedItemList<Test>>::read_bottom(tp_hash));
			assert_eq!(top, <OrderLinkedItemList<Test>>::read_top(tp_hash));	

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(None), Prev(Some(18446744073709551615)), Orders(0): 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(0)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 18_000_000, 200));
			let order1_hash = TradeModule::owned_order((bob, 0)).unwrap();
			let mut order1 = TradeModule::order(order1_hash).unwrap();
			assert_eq!(order1.sell_amount, 200);
			assert_eq!(order1.remained_sell_amount, 200);
			assert_eq!(order1.buy_amount, 36);
			assert_eq!(order1.remained_buy_amount, 36);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 10_000_000, 10));
			let order2_hash = TradeModule::owned_order((bob, 1)).unwrap();
			let mut order2 = TradeModule::order(order2_hash).unwrap();
			assert_eq!(order2.sell_amount, 10);
			assert_eq!(order2.remained_sell_amount, 10);
			assert_eq!(order2.buy_amount, 1);
			assert_eq!(order2.remained_buy_amount, 1);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 11_000_000, 100));
			let order3_hash = TradeModule::owned_order((bob, 2)).unwrap();
			let mut order3 = TradeModule::order(order3_hash).unwrap();
			assert_eq!(order3.sell_amount, 100);
			assert_eq!(order3.remained_sell_amount, 100);
			assert_eq!(order3.buy_amount, 11);
			assert_eq!(order3.remained_buy_amount, 11);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 11_000_000, 10000));
			let order4_hash = TradeModule::owned_order((bob, 3)).unwrap();
			let mut order4 = TradeModule::order(order4_hash).unwrap();
			assert_eq!(order4.sell_amount, 10000);
			assert_eq!(order4.remained_sell_amount, 10000);
			assert_eq!(order4.buy_amount, 1100);
			assert_eq!(order4.remained_buy_amount, 1100);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 6_000_000, 24));
			let order101_hash = TradeModule::owned_order((alice, 0)).unwrap();
			let order101 = TradeModule::order(order101_hash).unwrap();
			assert_eq!(order101.sell_amount, 24);
			assert_eq!(order101.remained_sell_amount, 24);
			assert_eq!(order101.buy_amount, 400);
			assert_eq!(order101.remained_buy_amount, 400);

			// bottom
			let mut item = OrderLinkedItem::<Test> {
				next: Some(6000000),
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order101_hash);

			item = OrderLinkedItem::<Test> {
				next: None,
				prev: Some(0),
				price: Some(6000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();

			item = OrderLinkedItem::<Test> {
				next: Some(10000000),
				prev: Some(6000000),
				price: None,
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item3
			curr = item.next;
			
			v = Vec::new();
			v.push(order2_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(11000000),
				prev: None,
				price: Some(10000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item4
			curr = item.next;

			v = Vec::new();
			v.push(order3_hash);
			v.push(order4_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(18000000),
				prev: Some(10000000),
				price: Some(11000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item5
			curr = item.next;

			v = Vec::new();
			v.push(order1_hash);

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(11000000),
				price: Some(18000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// top
			item = OrderLinkedItem::<Test> {
				next: min,
				prev: Some(18000000),
				price: max,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_top(tp_hash), item);


			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(6000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(6000000)), Next(None), Prev(Some(0)), Orders(1): (0x39ea…f183@[Created]: Sell[24, 24], Buy[400, 400]), 
			// Head ==> Price(None), Next(Some(10000000)), Prev(Some(6000000)), Orders(0): 
			// Price(Some(10000000)), Next(Some(11000000)), Prev(None), Orders(1): (0x2491…b0ff@[Created]: Sell[10, 10], Buy[1, 1]), 
			// Price(Some(11000000)), Next(Some(18000000)), Prev(Some(10000000)), Orders(2): (0xdf61…f22c@[Created]: Sell[100, 100], Buy[11, 11]), (0xe7d6…07ae@[Created]: Sell[10000, 10000], Buy[1100, 1100]), 
			// Price(Some(18000000)), Next(Some(18446744073709551615)), Prev(Some(11000000)), Orders(1): (0xe7da…ee16@[Created]: Sell[200, 200], Buy[36, 36]), 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(Some(18000000)), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);
	
			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 11_000_000, 55));

			let order102_hash = TradeModule::owned_order((alice, 1)).unwrap();
			let order102 = TradeModule::order(order102_hash).unwrap();
			assert_eq!(order102.sell_amount, 55);
			assert_eq!(order102.remained_sell_amount, 0);
			assert_eq!(order102.buy_amount, 500);
			assert_eq!(order102.remained_buy_amount, 0);
			assert_eq!(order102.status, OrderStatus::Filled);

			order2 = TradeModule::order(order2_hash).unwrap();
			assert_eq!(order2.sell_amount, 10);
			assert_eq!(order2.remained_sell_amount, 0);
			assert_eq!(order2.buy_amount, 1);
			assert_eq!(order2.remained_buy_amount, 0);
			assert_eq!(order2.status, OrderStatus::Filled);

			order3 = TradeModule::order(order3_hash).unwrap();
			assert_eq!(order3.sell_amount, 100);
			assert_eq!(order3.remained_sell_amount, 0);
			assert_eq!(order3.buy_amount, 11);
			assert_eq!(order3.remained_buy_amount, 0);
			assert_eq!(order3.status, OrderStatus::Filled);

			order4 = TradeModule::order(order4_hash).unwrap();
			assert_eq!(order4.sell_amount, 10000);
			assert_eq!(order4.remained_sell_amount, 9610);
			assert_eq!(order4.buy_amount, 1100);
			assert_eq!(order4.remained_buy_amount, 1057);
			assert_eq!(order4.status, OrderStatus::PartialFilled);

			// bottom
			let mut item = OrderLinkedItem::<Test> {
				next: Some(6000000),
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order101_hash);

			item = OrderLinkedItem::<Test> {
				next: None,
				prev: Some(0),
				price: Some(6000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();

			item = OrderLinkedItem::<Test> {
				next: Some(11000000),
				prev: Some(6000000),
				price: None,
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item4
			curr = item.next;

			v = Vec::new();
			v.push(order4_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(18000000),
				prev: None,
				price: Some(11000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item5
			curr = item.next;

			v = Vec::new();
			v.push(order1_hash);

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(11000000),
				price: Some(18000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// top
			item = OrderLinkedItem::<Test> {
				next: min,
				prev: Some(18000000),
				price: max,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_top(tp_hash), item);

			let trades = <TradePairOwnedTrades<Test>>::get(tp_hash).unwrap();
			let mut v = Vec::new();
			v.push(trades[0]);
			v.push(trades[1]);
			v.push(trades[2]);

			assert_eq!(<OwnedTrades<Test>>::get(alice), Some(v.clone()));
			assert_eq!(<OwnedTrades<Test>>::get(bob), Some(v.clone()));

			assert_eq!(<OwnedTPTrades<Test>>::get((alice, tp_hash)), Some(v.clone()));
			assert_eq!(<OwnedTPTrades<Test>>::get((bob, tp_hash)), Some(v.clone()));

			assert_eq!(<TradePairOwnedTrades<Test>>::get(tp_hash), Some(v.clone()));

			assert_eq!(<OrderOwnedTrades<Test>>::get(order102_hash).unwrap().len(), 3);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order102_hash), Some(v));

			let mut v = Vec::new();
			v.push(trades[0]);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order2_hash).unwrap().len(), 1);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order2_hash), Some(v));

			let mut v = Vec::new();
			v.push(trades[1]);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order3_hash).unwrap().len(), 1);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order3_hash), Some(v));

			let mut v = Vec::new();
			v.push(trades[2]);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order4_hash).unwrap().len(), 1);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order4_hash), Some(v));

			assert_eq!(trades.len(), 3);
			let t1 = <Trades<Test>>::get(trades[0]).unwrap();
			let trade1 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 10_000_000,
				base_amount: 1,
				quote_amount: 10,
				..t1
			};
			assert_eq!(t1, trade1);

			let t2 = <Trades<Test>>::get(trades[1]).unwrap();
			let trade2 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 11000000,
				base_amount: 11,
				quote_amount: 100,
				..t2
			};
			assert_eq!(t2, trade2);

			let t3 = <Trades<Test>>::get(trades[2]).unwrap();
			let trade3 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 11000000,
				base_amount: 43,
				quote_amount: 390,
				..t3
			};
			assert_eq!(t3, trade3);

			assert_eq!(TokenModule::balance_of((alice, quote)), 500);
			assert_eq!(TokenModule::balance_of((bob, base)), 55);

			let tp = TradeModule::trade_pair_by_hash(tp_hash).unwrap();
			assert_eq!(tp.buy_one_price, Some(6000000));
			assert_eq!(tp.sell_one_price, Some(11000000));
			assert_eq!(tp.latest_matched_price, Some(11000000));

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(6000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(6000000)), Next(None), Prev(Some(0)), Orders(1): (0x39ea…f183@[Created]: Sell[24, 24], Buy[400, 400]), 
			// Head ==> Price(None), Next(Some(11000000)), Prev(Some(6000000)), Orders(0): 
			// Price(Some(11000000)), Next(Some(18000000)), Prev(None), Orders(1): (0xe7d6…07ae@[PartialFilled]: Sell[10000, 9610], Buy[1100, 1057]), 
			// Price(Some(18000000)), Next(Some(18446744073709551615)), Prev(Some(11000000)), Orders(1): (0xe7da…ee16@[Created]: Sell[200, 200], Buy[36, 36]), 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(Some(18000000)), Orders(0): 
			// [Market Trades]
			// [0x72bb…80c0/0x8a33…f642] - 0x3fdf…4371@10000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][1, 10]
			// [0x72bb…80c0/0x8a33…f642] - 0xad18…fab8@11000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][11, 100]
			// [0x72bb…80c0/0x8a33…f642] - 0x1f02…44c7@11000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][43, 390]
			output_order(tp_hash);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 18_000_000, 13212));
			let order103_hash = TradeModule::owned_order((alice, 2)).unwrap();
			let order103 = TradeModule::order(order103_hash).unwrap();
			assert_eq!(order103.sell_amount, 13212);
			assert_eq!(order103.remained_sell_amount, 13212 - 1057 - 36);
			assert_eq!(order103.buy_amount, 73400);
			assert_eq!(order103.remained_buy_amount, 73400 - 9610 - 200);
			assert_eq!(order103.status, OrderStatus::PartialFilled);

			order4 = TradeModule::order(order4_hash).unwrap();
			assert_eq!(order4.sell_amount, 10000);
			assert_eq!(order4.remained_sell_amount, 0);
			assert_eq!(order4.buy_amount, 1100);
			assert_eq!(order4.remained_buy_amount, 0);
			assert_eq!(order4.status, OrderStatus::Filled);

			order1 = TradeModule::order(order1_hash).unwrap();
			assert_eq!(order1.sell_amount, 200);
			assert_eq!(order1.remained_sell_amount, 0);
			assert_eq!(order1.buy_amount, 36);
			assert_eq!(order1.remained_buy_amount, 0);
			assert_eq!(order1.status, OrderStatus::Filled);

			// bottom
			let mut item = OrderLinkedItem::<Test> {
				next: Some(6000000),
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order101_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(18000000),
				prev: Some(0),
				price: Some(6000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();
			v.push(order103_hash);

			item = OrderLinkedItem::<Test> {
				next: None,
				prev: Some(6000000),
				price: Some(18000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item4
			curr = item.next;

			v = Vec::new();

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(18000000),
				price: None,
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// top
			item = OrderLinkedItem::<Test> {
				next: min,
				prev: None,
				price: max,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_top(tp_hash), item);

			let trades = <TradePairOwnedTrades<Test>>::get(tp_hash).unwrap();
			let mut v = Vec::new();
			v.push(trades[0]);
			v.push(trades[1]);
			v.push(trades[2]);
			v.push(trades[3]);
			v.push(trades[4]);

			assert_eq!(<OwnedTrades<Test>>::get(alice), Some(v.clone()));
			assert_eq!(<OwnedTrades<Test>>::get(bob), Some(v.clone()));

			assert_eq!(<OwnedTPTrades<Test>>::get((alice, tp_hash)), Some(v.clone()));
			assert_eq!(<OwnedTPTrades<Test>>::get((bob, tp_hash)), Some(v.clone()));

			assert_eq!(<TradePairOwnedTrades<Test>>::get(tp_hash), Some(v.clone()));

			let mut v = Vec::new();
			v.push(trades[3]);
			v.push(trades[4]);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order103_hash).unwrap().len(), 2);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order103_hash), Some(v));

			let mut v = Vec::new();
			v.push(trades[2]);
			v.push(trades[3]);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order4_hash).unwrap().len(), 2);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order4_hash), Some(v));

			let mut v = Vec::new();
			v.push(trades[4]);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order1_hash).unwrap().len(), 1);
			assert_eq!(<OrderOwnedTrades<Test>>::get(order1_hash), Some(v));

			let trades = <TradePairOwnedTrades<Test>>::get(tp_hash).unwrap();
			assert_eq!(trades.len(), 5);
			let t1 = <Trades<Test>>::get(trades[0]).unwrap();
			let trade1 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 10_000_000,
				base_amount: 1,
				quote_amount: 10,
				..t1
			};
			assert_eq!(t1, trade1);

			let t2 = <Trades<Test>>::get(trades[1]).unwrap();
			let trade2 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 11000000,
				base_amount: 11,
				quote_amount: 100,
				..t2
			};
			assert_eq!(t2, trade2);

			let t3 = <Trades<Test>>::get(trades[2]).unwrap();
			let trade3 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 11000000,
				base_amount: 43,
				quote_amount: 390,
				..t3
			};
			assert_eq!(t3, trade3);

			let t4 = <Trades<Test>>::get(trades[3]).unwrap();
			let trade4 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 11000000,
				base_amount: 1057,
				quote_amount: 9610,
				..t4
			};
			assert_eq!(t4, trade4);

			let t5 = <Trades<Test>>::get(trades[4]).unwrap();
			let trade5 = Trade::<Test> {
				base: base,
				quote: quote,
				buyer: alice,
				seller: bob,
				maker: bob,
				taker: alice,
				otype: OrderType::Buy,
				price: 18000000,
				base_amount: 36,
				quote_amount: 200,
				..t5
			};
			assert_eq!(t5, trade5);

			assert_eq!(TokenModule::balance_of((alice, quote)), 10 + 100 + 10000 + 200);
			assert_eq!(TokenModule::balance_of((bob, base)), 1 + 11 + 1100 + 36);

			let tp = TradeModule::trade_pair_by_hash(tp_hash).unwrap();
			assert_eq!(tp.buy_one_price, Some(18000000));
			assert_eq!(tp.sell_one_price, None);
			assert_eq!(tp.latest_matched_price, Some(18000000));

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(6000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(6000000)), Next(Some(18000000)), Prev(Some(0)), Orders(1): (0x39ea…f183@[Created]: Sell[24, 24], Buy[400, 400]), 
			// Price(Some(18000000)), Next(None), Prev(Some(6000000)), Orders(1): (0x78ae…6f02@[PartialFilled]: Sell[13212, 12119], Buy[73400, 63590]), 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(18000000)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			// [0x72bb…80c0/0x8a33…f642] - 0x3fdf…4371@10000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][1, 10]
			// [0x72bb…80c0/0x8a33…f642] - 0xad18…fab8@11000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][11, 100]
			// [0x72bb…80c0/0x8a33…f642] - 0x1f02…44c7@11000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][43, 390]
			// [0x72bb…80c0/0x8a33…f642] - 0x7794…d2a9@11000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][1057, 9610]
			// [0x72bb…80c0/0x8a33…f642] - 0x5759…febf@18000000[Buy]: [Buyer,Seller][10,20], [Maker,Taker][20,10], [Base,Quote][36, 200]
			output_order(tp_hash);
		});
	}

	#[test]
	fn order_cancel_test_case() {
		with_externalities(&mut new_test_ext(), || {
			let alice = 10;
			let bob = 20;

			let max = Some(<Test as Trait>::Price::max_value());
			let min = Some(<Test as Trait>::Price::min_value());

			// token1
			assert_ok!(TokenModule::issue(Origin::signed(alice), b"66".to_vec(), 21000000));
			let token1_hash = TokenModule::owned_token((alice, 0)).unwrap();
			let token1 = TokenModule::token(token1_hash).unwrap();

			// token2
			assert_ok!(TokenModule::issue(Origin::signed(bob), b"77".to_vec(), 10000000));
			let token2_hash = TokenModule::owned_token((bob, 0)).unwrap();
			let token2 = TokenModule::token(token2_hash).unwrap();

			// tradepair
			let base = token1.hash;
			let quote = token2.hash;
			assert_ok!(TradeModule::create_trade_pair(Origin::signed(alice), base, quote));
			let tp_hash = TradeModule::get_trade_pair_hash_by_base_quote((base, quote)).unwrap();

			let bottom = OrderLinkedItem::<Test> {
				prev: max,
				next: None,
				price: min,
				orders: Vec::new(),
			};

			let top = OrderLinkedItem::<Test> {
				prev: None,
				next: min,
				price: max,
				orders: Vec::new(),
			};

			let head = OrderLinkedItem::<Test> {
				prev: min,
				next: max,
				price: None,
				orders: Vec::new(),
			};

			assert_eq!(head, <OrderLinkedItemList<Test>>::read_head(tp_hash));
			assert_eq!(bottom, <OrderLinkedItemList<Test>>::read_bottom(tp_hash));
			assert_eq!(top, <OrderLinkedItemList<Test>>::read_top(tp_hash));	

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(None), Prev(Some(18446744073709551615)), Orders(0): 
			// Head ==> Price(None), Next(Some(18446744073709551615)), Prev(Some(0)), Orders(0): 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(None), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 18_000_000, 200));
			let order1_hash = TradeModule::owned_order((bob, 0)).unwrap();
			let mut order1 = TradeModule::order(order1_hash).unwrap();
			assert_eq!(order1.sell_amount, 200);
			assert_eq!(order1.remained_sell_amount, 200);
			assert_eq!(order1.buy_amount, 36);
			assert_eq!(order1.remained_buy_amount, 36);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 10_000_000, 10));
			let order2_hash = TradeModule::owned_order((bob, 1)).unwrap();
			let mut order2 = TradeModule::order(order2_hash).unwrap();
			assert_eq!(order2.sell_amount, 10);
			assert_eq!(order2.remained_sell_amount, 10);
			assert_eq!(order2.buy_amount, 1);
			assert_eq!(order2.remained_buy_amount, 1);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 11_000_000, 100));
			let order3_hash = TradeModule::owned_order((bob, 2)).unwrap();
			let mut order3 = TradeModule::order(order3_hash).unwrap();
			assert_eq!(order3.sell_amount, 100);
			assert_eq!(order3.remained_sell_amount, 100);
			assert_eq!(order3.buy_amount, 11);
			assert_eq!(order3.remained_buy_amount, 11);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 11_000_000, 10000));
			let order4_hash = TradeModule::owned_order((bob, 3)).unwrap();
			let mut order4 = TradeModule::order(order4_hash).unwrap();
			assert_eq!(order4.sell_amount, 10000);
			assert_eq!(order4.remained_sell_amount, 10000);
			assert_eq!(order4.buy_amount, 1100);
			assert_eq!(order4.remained_buy_amount, 1100);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 6_000_000, 24));
			let order101_hash = TradeModule::owned_order((alice, 0)).unwrap();
			let mut order101 = TradeModule::order(order101_hash).unwrap();
			assert_eq!(order101.sell_amount, 24);
			assert_eq!(order101.remained_sell_amount, 24);
			assert_eq!(order101.buy_amount, 400);
			assert_eq!(order101.remained_buy_amount, 400);

			// bottom
			let mut item = OrderLinkedItem::<Test> {
				next: Some(6000000),
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();
			v.push(order101_hash);

			item = OrderLinkedItem::<Test> {
				next: None,
				prev: Some(0),
				price: Some(6000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();

			item = OrderLinkedItem::<Test> {
				next: Some(10000000),
				prev: Some(6000000),
				price: None,
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item3
			curr = item.next;
			
			v = Vec::new();
			v.push(order2_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(11000000),
				prev: None,
				price: Some(10000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item4
			curr = item.next;

			v = Vec::new();
			v.push(order3_hash);
			v.push(order4_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(18000000),
				prev: Some(10000000),
				price: Some(11000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item5
			curr = item.next;

			v = Vec::new();
			v.push(order1_hash);

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(11000000),
				price: Some(18000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// top
			item = OrderLinkedItem::<Test> {
				next: min,
				prev: Some(18000000),
				price: max,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_top(tp_hash), item);


			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(Some(6000000)), Prev(Some(18446744073709551615)), Orders(0): 
			// Price(Some(6000000)), Next(None), Prev(Some(0)), Orders(1): (0x39ea…f183@[Created]: Sell[24, 24], Buy[400, 400]), 
			// Head ==> Price(None), Next(Some(10000000)), Prev(Some(6000000)), Orders(0): 
			// Price(Some(10000000)), Next(Some(11000000)), Prev(None), Orders(1): (0x2491…b0ff@[Created]: Sell[10, 10], Buy[1, 1]), 
			// Price(Some(11000000)), Next(Some(18000000)), Prev(Some(10000000)), Orders(2): (0xdf61…f22c@[Created]: Sell[100, 100], Buy[11, 11]), (0xe7d6…07ae@[Created]: Sell[10000, 10000], Buy[1100, 1100]), 
			// Price(Some(18000000)), Next(Some(18446744073709551615)), Prev(Some(11000000)), Orders(1): (0xe7da…ee16@[Created]: Sell[200, 200], Buy[36, 36]), 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(Some(18000000)), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);
	
			order101.status = OrderStatus::Filled;
			let tmp_amount = order101.remained_buy_amount;
			order101.remained_buy_amount = Zero::zero();
			<Orders<Test>>::insert(order101.hash, order101.clone());
			assert_err!(TradeModule::cancel_limit_order(Origin::signed(alice), order101.hash), "can not cancel finished order");

			assert_err!(TradeModule::cancel_limit_order(Origin::signed(alice), order1.hash), "can only cancel your owned order");

			assert_err!(TradeModule::cancel_limit_order(Origin::signed(alice), H256::from_low_u64_be(0)), "can not get order");

			order101.status = OrderStatus::Created;
			order101.remained_buy_amount = tmp_amount;
			<Orders<Test>>::insert(order101.hash, order101.clone());
			assert_ok!(TradeModule::cancel_limit_order(Origin::signed(alice), order101.hash));
			
			order3.status = OrderStatus::PartialFilled;
			order3.remained_buy_amount = order3.remained_buy_amount.checked_sub(1).unwrap();
			<Orders<Test>>::insert(order3.hash, order3.clone());
			assert_ok!(TradeModule::cancel_limit_order(Origin::signed(bob), order3.hash));
			assert_ok!(TradeModule::cancel_limit_order(Origin::signed(bob), order4.hash));

			// bottom
			let mut item = OrderLinkedItem::<Test> {
				next: None,
				prev: max,
				price: min,
				orders: Vec::new(),
			};
			assert_eq!(OrderLinkedItemList::<Test>::read_bottom(tp_hash), item);

			// item1
			let mut curr = item.next;

			let mut v = Vec::new();

			item = OrderLinkedItem::<Test> {
				next: Some(10000000),
				prev: Some(0),
				price: None,
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item2
			curr = item.next;

			v = Vec::new();
			v.push(order2_hash);

			item = OrderLinkedItem::<Test> {
				next: Some(18000000),
				prev: None,
				price: Some(10000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item3
			curr = item.next;
			
			v = Vec::new();
			v.push(order1_hash);

			item = OrderLinkedItem::<Test> {
				next: max,
				prev: Some(10000000),
				price: Some(18000000),
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// item4
			curr = item.next;

			v = Vec::new();

			item = OrderLinkedItem::<Test> {
				next: min,
				prev: Some(18000000),
				price: max,
				orders: v,
			};
			assert_eq!(OrderLinkedItemList::<Test>::read(tp_hash, curr), item);

			// [Market Orders]
			// Bottom ==> Price(Some(0)), Next(None), Prev(Some(18446744073709551615)), Orders(0): 
			// Head ==> Price(None), Next(Some(10000000)), Prev(Some(0)), Orders(0): 
			// Price(Some(10000000)), Next(Some(18000000)), Prev(None), Orders(1): (0x2491…b0ff@[Created]: Sell[10, 10], Buy[1, 1]), 
			// Price(Some(18000000)), Next(Some(18446744073709551615)), Prev(Some(10000000)), Orders(1): (0xe7da…ee16@[Created]: Sell[200, 200], Buy[36, 36]), 
			// Top ==> Price(Some(18446744073709551615)), Next(Some(0)), Prev(Some(18000000)), Orders(0): 
			// [Market Trades]
			output_order(tp_hash);
		});
	}

	#[test]
	fn order_match_test_case() {
		with_externalities(&mut new_test_ext(), || {
			let alice = 10;
			let bob = 20;

			// token1
			assert_ok!(TokenModule::issue(Origin::signed(alice), b"66".to_vec(), 21000000));
			let token1_hash = TokenModule::owned_token((alice, 0)).unwrap();
			let token1 = TokenModule::token(token1_hash).unwrap();

			// token2
			assert_ok!(TokenModule::issue(Origin::signed(bob), b"77".to_vec(), 10000000));
			let token2_hash = TokenModule::owned_token((bob, 0)).unwrap();
			let token2 = TokenModule::token(token2_hash).unwrap();

			// tradepair
			let base = token1.hash;
			let quote = token2.hash;
			assert_ok!(TradeModule::create_trade_pair(Origin::signed(alice), base, quote));

			assert_ok!(TradeModule::create_limit_order(Origin::signed(alice), base, quote, OrderType::Buy, 25_010_000, 2501));
			let order101_hash = TradeModule::owned_order((alice, 0)).unwrap();
			let order101 = TradeModule::order(order101_hash).unwrap();
			assert_eq!(order101.sell_amount, 2501);
			assert_eq!(order101.remained_sell_amount, 2501);
			assert_eq!(order101.buy_amount, 10000);
			assert_eq!(order101.remained_buy_amount, 10000);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 25_000_000, 4));
			let order1_hash = TradeModule::owned_order((bob, 0)).unwrap();
			let order1 = TradeModule::order(order1_hash).unwrap();
			assert_eq!(order1.sell_amount, 4);
			assert_eq!(order1.remained_sell_amount, 0);
			assert_eq!(order1.buy_amount, 1);
			assert_eq!(order1.remained_buy_amount, 0);

			assert_eq!(TokenModule::balance_of((alice, quote)), 4);
			assert_eq!(TokenModule::balance_of((alice, base)), 21000000 - 1);
			assert_eq!(TokenModule::balance_of((bob, base)), 1);
			assert_eq!(TokenModule::balance_of((bob, quote)), 10000000 - 4);

			assert_ok!(TradeModule::create_limit_order(Origin::signed(bob), base, quote, OrderType::Sell, 25_000_000, 9996));
			let order1_hash = TradeModule::owned_order((bob, 1)).unwrap();
			let order1 = TradeModule::order(order1_hash).unwrap();
			assert_eq!(order1.sell_amount, 9996);
			assert_eq!(order1.remained_sell_amount, 0);
			assert_eq!(order1.buy_amount, 2499);
			assert_eq!(order1.remained_buy_amount, 0);

			let order101_hash = TradeModule::owned_order((alice, 0)).unwrap();
			let order101 = TradeModule::order(order101_hash).unwrap();
			assert_eq!(order101.sell_amount, 2501);
			assert_eq!(order101.remained_sell_amount, 1);
			assert_eq!(order101.buy_amount, 10000);
			assert_eq!(order101.remained_buy_amount, 3);

			assert_eq!(TokenModule::balance_of((alice, quote)), 4 + 9993);
			assert_eq!(TokenModule::balance_of((alice, base)), 21000000 - 1 - 2499);
			assert_eq!(TokenModule::freezed_balance_of((alice, base)), 1);
			assert_eq!(TokenModule::balance_of((bob, base)), 1 + 2499);
			assert_eq!(TokenModule::balance_of((bob, quote)), 10000000 - 4 - 9993);
			assert_eq!(TokenModule::freezed_balance_of((bob, base)), 0);
		});
	}

	#[test]
	fn calculate_ex_amount() {
		with_externalities(&mut new_test_ext(), || {
			let alice = 10;
			let bob = 20;

			let order1 = LimitOrder::<Test> {
				hash: H256::from_low_u64_be(0),
				base: H256::from_low_u64_be(0),
				quote: H256::from_low_u64_be(0),
				owner: alice,
				price: <Test as Trait>::Price::sa(25010000),
				sell_amount: 2501,
				remained_sell_amount: 2501,
				buy_amount: 10000,
				remained_buy_amount: 10000,
				otype: OrderType::Buy,
				status: OrderStatus::Created,
			};

			let mut order2 = LimitOrder::<Test> {
				hash: H256::from_low_u64_be(0),
				base: H256::from_low_u64_be(0),
				quote: H256::from_low_u64_be(0),
				owner: bob,
				price: <Test as Trait>::Price::sa(25000000),
				sell_amount: 4,
				remained_sell_amount: 4,
				buy_amount: 1,
				remained_buy_amount: 1,
				otype: OrderType::Sell,
				status: OrderStatus::Created,
			};

			let result = TradeModule::calculate_ex_amount(&order1, &order2).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			let result = TradeModule::calculate_ex_amount(&order2, &order1).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			order2.price = <Test as Trait>::Price::sa(25_010_000);
			let result = TradeModule::calculate_ex_amount(&order1, &order2).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			let result = TradeModule::calculate_ex_amount(&order2, &order1).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			order2.price = <Test as Trait>::Price::sa(33_000_000);
			let result = TradeModule::calculate_ex_amount(&order1, &order2).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			let result = TradeModule::calculate_ex_amount(&order2, &order1).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			order2.price = <Test as Trait>::Price::sa(35_000_000);
			let result = TradeModule::calculate_ex_amount(&order1, &order2).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 4);

			let result = TradeModule::calculate_ex_amount(&order2, &order1).unwrap();
			assert_eq!(result.0, 1);
			assert_eq!(result.1, 3);
		});
	}

	#[test]
	fn ensure_amount_zero_digits_test_case() {
		with_externalities(&mut new_test_ext(), || {
			assert_eq!(1, 1);

			let price = <Test as Trait>::Price::sa(25010000); // 0.2501
			let amount = <Test as balances::Trait>::Balance::sa(2501);
			let otype = OrderType::Buy;
			assert_ok!(TradeModule::ensure_counterparty_amount_bounds(otype, price, amount), 10000u128);
			
			let price = <Test as Trait>::Price::sa(25010000); // 0.2501
			let amount = <Test as balances::Trait>::Balance::sa(2500);
			let otype = OrderType::Buy;
			assert_err!(TradeModule::ensure_counterparty_amount_bounds(otype, price, amount), "amount have digits parts");

			let price = <Test as Trait>::Price::sa(25000000); // 0.25
			let amount = <Test as balances::Trait>::Balance::sa(24);
			let otype = OrderType::Sell;
			assert_ok!(TradeModule::ensure_counterparty_amount_bounds(otype, price, amount), 6u128);
			
			let price = <Test as Trait>::Price::sa(25000000); // 0.25
			let amount = <Test as balances::Trait>::Balance::sa(21);
			let otype = OrderType::Sell;
			assert_err!(TradeModule::ensure_counterparty_amount_bounds(otype, price, amount), "amount have digits parts");

			let price = <Test as Trait>::Price::sa(200000000); // 2.0
			let amount = <Test as balances::Trait>::Balance::sa(u128::max_value() - 1);
			let otype = OrderType::Sell;
			assert_err!(TradeModule::ensure_counterparty_amount_bounds(otype, price, amount), "counterparty bound check failed");
		});
	}
}