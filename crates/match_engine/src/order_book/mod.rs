use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::anyhow;
use db::Database;
use sorted_insert::SortedInsertByKey;

use crate::order::{Order, OrderStatus, OrderType};

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
    pub active_orders: Vec<Order>,
    pub fulfilled_orders: Vec<Order>,
}

#[derive(Default)]
pub struct OrderBook {
    pair: Option<String>,
    db: Option<Arc<Mutex<Database>>>,
    buy_orders: Arc<Mutex<Vec<Order>>>,
    sell_orders: Arc<Mutex<Vec<Order>>>,
}

impl OrderBook {
    pub fn set_pair(&mut self, pair: String) {
        self.pair = Some(pair)
    }

    pub fn set_db(&mut self, db: Arc<Mutex<Database>>) {
        self.db = Some(db);
    }

    pub fn get_pair(&self) -> &String {
        self.pair.as_ref().expect("Pair is not set!")
    }

    pub fn load(&mut self) {
        let binding = self.db.clone().expect("Database is required!");
        let guard = &binding.lock().unwrap();

        match guard.get(&self.pair.clone().expect("Pair is required!")) {
            Ok(value) => match value {
                Some(item) => {
                    let item_from_db: Item =
                        serde_json::from_str(item.as_str()).expect("Failed to deserialize!");
                    item_from_db
                        .active_orders
                        .clone()
                        .into_iter()
                        .filter(|o| o.order_type == OrderType::Buy)
                        .for_each(|o| {
                            self.buy_orders
                                .clone()
                                .lock()
                                .expect("Failed to get buy orders lock")
                                .push(o)
                        });

                    item_from_db
                        .active_orders
                        .clone()
                        .into_iter()
                        .filter(|o| o.order_type == OrderType::Sell)
                        .for_each(|o| {
                            self.sell_orders
                                .clone()
                                .lock()
                                .expect("Failed to get sell orders lock")
                                .push(o)
                        });
                }
                None => {}
            },
            Err(_) => {}
        }
    }

    pub fn build(self) -> Self {
        Self {
            pair: self.pair.map(Some).expect("Pair is required!"),
            db: self.db.map(Some).expect("Db is required!"),
            buy_orders: Arc::new(Mutex::new(Vec::new())),
            sell_orders: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_buy_orders(&self) -> Vec<Order> {
        let buy_orders = Arc::clone(&self.buy_orders);
        let orders_vec = buy_orders.lock().unwrap().to_owned();
        return orders_vec;
    }

    pub fn get_sell_orders(&self) -> Vec<Order> {
        let sell_orders = Arc::clone(&self.sell_orders);
        let orders_vec = sell_orders.lock().unwrap().to_owned();
        return orders_vec;
    }

    pub fn get_filled_buy_orders(&self) -> Vec<Order> {
        let orders: Vec<Order> = self
            .get_buy_orders()
            .into_iter()
            .filter(|o| o.order_status == OrderStatus::Filled)
            .collect();
        return orders;
    }

    pub fn get_filled_sell_orders(&self) -> Vec<Order> {
        let orders: Vec<Order> = self
            .get_sell_orders()
            .into_iter()
            .filter(|o| o.order_status == OrderStatus::Filled)
            .collect();
        return orders;
    }

    pub fn get_active_buy_orders(&self) -> Vec<Order> {
        let orders: Vec<Order> = self
            .get_buy_orders()
            .into_iter()
            .filter(|o| o.order_status == OrderStatus::Active)
            .collect();
        return orders;
    }

    pub fn get_active_sell_orders(&self) -> Vec<Order> {
        let orders: Vec<Order> = self
            .get_sell_orders()
            .into_iter()
            .filter(|o| o.order_status == OrderStatus::Active)
            .collect();
        return orders;
    }

    pub fn join_active_orders(&self) -> Vec<Order> {
        self.get_active_buy_orders()
            .into_iter()
            .chain(self.get_active_sell_orders())
            .collect::<Vec<Order>>()
    }

    pub fn join_filled_orders(&self) -> Vec<Order> {
        self.get_filled_buy_orders()
            .into_iter()
            .chain(self.get_filled_sell_orders())
            .collect::<Vec<Order>>()
    }

    pub fn append_buy_order(&mut self, order: Order) -> anyhow::Result<()> {
        match order.order_type {
            OrderType::Buy => {
                let mut buy_orders = self.buy_orders.lock().unwrap();
                buy_orders.sorted_insert_desc_by_key(order, |o| &o.price);
                drop(buy_orders);

                let db_mutex_guard = self
                    .db
                    .as_ref()
                    .expect("Database is not set!")
                    .lock()
                    .expect("could not get db lock");
                db_mutex_guard
                    .set(
                        &self.get_pair(),
                        &Item {
                            active_orders: self.join_active_orders(),
                            fulfilled_orders: self.join_filled_orders(),
                        },
                    )
                    .expect("sam bankman fried");
                drop(db_mutex_guard);

                self.match_orders();
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid order type, expected Buy order type but Sell provided"
            )),
        }
    }

    pub fn append_sell_order(&mut self, order: Order) -> anyhow::Result<()> {
        match order.order_type {
            OrderType::Sell => {
                let mut sell_orders = self.sell_orders.lock().unwrap();
                sell_orders.sorted_insert_asc_by_key(order, |o| &o.price);
                drop(sell_orders);

                let db_mutex_guard = self
                    .db
                    .as_ref()
                    .expect("Database is not set!")
                    .lock()
                    .expect("could not get db lock");
                db_mutex_guard
                    .set(
                        &self.get_pair(),
                        &Item {
                            active_orders: self.join_active_orders(),
                            fulfilled_orders: self.join_filled_orders(),
                        },
                    )
                    .expect("sam bankman fried");
                drop(db_mutex_guard);

                self.match_orders();
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid order type, expected Sell order type but Buy provided"
            )),
        }
    }

    fn match_orders(&self) {
        let stop = AtomicBool::new(false);

        let buy_orders = Arc::clone(&self.buy_orders);
        let sell_orders = Arc::clone(&self.sell_orders);

        thread::spawn(move || {
            let mut index = 0;
            while !stop.load(Ordering::Relaxed) {
                let index_len = index + 1;
                let mut buy_orders = buy_orders.lock().unwrap();
                let mut sell_orders = sell_orders.lock().unwrap();

                if index_len > buy_orders.len() || index_len > sell_orders.len() {
                    stop.store(true, Ordering::Relaxed);
                }

                if let Some(max_buy_order) = buy_orders.get_mut(index) {
                    if let Some(min_sell_order) = sell_orders.get_mut(index) {
                        if max_buy_order.price >= min_sell_order.price
                            && max_buy_order.order_status == OrderStatus::Active
                            && min_sell_order.order_status == OrderStatus::Active
                        {
                            max_buy_order.update_order_status(OrderStatus::Filled);
                            min_sell_order.update_order_status(OrderStatus::Filled);
                        }
                    }
                }
                index += 1;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazy_static::lazy_static;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;

    lazy_static! {
        static ref PAIR: String = "BTC/ETH".to_string();
    }

    fn cleanup() {
        if Path::new("mock.db").exists() {
            fs::remove_dir_all("mock.db").expect("could not delete mock.db")
        }
    }

    #[test]
    fn it_should_load_orders_from_db() {
        let db = Arc::new(Mutex::new(Database::new(Some("mock.db".to_string()))));
        let mut order_book_builder = OrderBook::default();
        order_book_builder.set_pair(PAIR.clone());
        order_book_builder.set_db(db.clone());

        let buy = Order::new(1, 10, OrderType::Buy);
        let sell = Order::new(1, 20, OrderType::Sell);

        let binding = db.clone();
        let db_guard = binding.lock().unwrap();

        db_guard
            .set(
                &PAIR.clone(),
                &Item {
                    active_orders: vec![buy.clone(), sell.clone()],
                    fulfilled_orders: vec![],
                },
            )
            .unwrap();
        drop(db_guard);

        let mut order_book = order_book_builder.build();
        order_book.load();

        let binding_buy_order = order_book.buy_orders.clone();
        let buy_orders_guard = binding_buy_order.lock().unwrap();

        let binding_sell_order = order_book.sell_orders.clone();
        let sell_order_guard = binding_sell_order.lock().unwrap();

        assert_eq!(*buy_orders_guard, vec![buy]);
        assert_eq!(*sell_order_guard, vec![sell]);

        cleanup();
    }

    #[test]
    // Buy | Sell
    //  5 | 4
    //  4 | 3
    //  3 | 9
    fn match_orders_test() {
        let db = Arc::new(Mutex::new(Database::new(Some("mock.db".to_string()))));
        let mut order_book_builder = OrderBook::default();
        order_book_builder.set_pair(PAIR.clone());
        order_book_builder.set_db(db.clone());

        let mut order_book = order_book_builder.build();

        let orders: [Order; 6] = [
            Order::new(1, 4, OrderType::Sell),
            Order::new(1, 3, OrderType::Sell),
            Order::new(1, 9, OrderType::Sell),
            //
            Order::new(1, 5, OrderType::Buy),
            Order::new(1, 4, OrderType::Buy),
            Order::new(1, 3, OrderType::Buy),
        ];

        for order in orders {
            if order.order_type == OrderType::Buy {
                order_book
                    .append_buy_order(order)
                    .expect("could not append buy order");
            } else {
                order_book
                    .append_sell_order(order)
                    .expect("could not append sell order");
            }
        }

        thread::sleep(Duration::from_secs(10));

        let filled_buy_orders: Vec<i32> = order_book
            .get_filled_buy_orders()
            .into_iter()
            .map(|o| o.price)
            .collect();
        let filled_sell_orders: Vec<i32> = order_book
            .get_filled_sell_orders()
            .into_iter()
            .map(|o| o.price)
            .collect();

        assert_eq!(filled_buy_orders, vec![5, 4]);
        assert_eq!(filled_sell_orders, vec![3, 4]);

        cleanup();
    }
}
