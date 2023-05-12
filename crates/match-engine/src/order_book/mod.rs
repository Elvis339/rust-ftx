use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::anyhow;
use db::Database;
use sorted_insert::SortedInsertByKey;

use crate::order::{Order, OrderStatus, OrderType};

pub struct OrderBook {
    pub pair: String,
    db: Arc<Mutex<Database>>,
    buy_orders: Arc<Mutex<Vec<Order>>>,
    sell_orders: Arc<Mutex<Vec<Order>>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Item {
    active_orders: Vec<Order>,
    fulfilled_orders: Vec<Order>,
}

impl OrderBook {
    pub fn new(pair: String, db: Arc<Mutex<Database>>) -> Self {
        Self {
            pair,
            buy_orders: Arc::new(Mutex::new(Vec::new())),
            sell_orders: Arc::new(Mutex::new(Vec::new())),
            db,
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

    fn join_active_orders(&self) -> Vec<Order> {
        self.get_active_buy_orders()
            .into_iter()
            .chain(self.get_active_sell_orders())
            .collect::<Vec<Order>>()
    }

    fn join_filled_orders(&self) -> Vec<Order> {
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

                let db_mutex_guard = self.db.lock().expect("could not get db lock");
                db_mutex_guard
                    .set(
                        &self.pair,
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

                let db_mutex_guard = self.db.lock().expect("could not get db lock");
                db_mutex_guard
                    .set(
                        &self.pair,
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
                    let min_sell_order = sell_orders.get_mut(index).expect("No sell orders");

                    if max_buy_order.price >= min_sell_order.price
                        && max_buy_order.order_status == OrderStatus::Active
                        && min_sell_order.order_status == OrderStatus::Active
                    {
                        max_buy_order.update_order_status(OrderStatus::Filled);
                        min_sell_order.update_order_status(OrderStatus::Filled);
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
    // Buy | Sell
    //  5 | 4
    //  4 | 3
    //  3 | 9
    fn match_orders_test() {
        let db = Arc::new(Mutex::new(Database::new(Some("mock.db".to_string()))));
        let mut order_book = OrderBook::new(PAIR.clone(), db.clone());

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
