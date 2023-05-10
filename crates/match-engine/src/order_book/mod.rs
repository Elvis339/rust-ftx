use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use sorted_insert::SortedInsertByKey;

use crate::order::{Order, OrderStatus, OrderType};

#[derive(Debug)]
pub struct OrderBook {
    buy_orders: Arc<Mutex<Vec<Order>>>,
    sell_orders: Arc<Mutex<Vec<Order>>>,
    channel: mpsc::Sender<Order>,
}

impl OrderBook {
    pub fn new(channel: mpsc::Sender<Order>) -> Self {
        Self {
            buy_orders: Arc::new(Mutex::new(Vec::new())),
            sell_orders: Arc::new(Mutex::new(Vec::new())),
            channel,
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

    pub fn append_buy_order(&mut self, order: Order) -> anyhow::Result<()> {
        match order.order_type {
            OrderType::Buy => {
                let mut buy_orders = self.buy_orders.lock().unwrap();
                buy_orders.sorted_insert_desc_by_key(order, |o| &o.price);
                self.channel.send(order).expect("could not send buy order");
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
                self.channel.send(order).expect("could not send sell order");
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid order type, expected Sell order type but Buy provided"
            )),
        }
    }

    fn match_orders(&self, rx: Arc<Mutex<Receiver<Order>>>) {
        let stop = AtomicBool::new(false);

        let buy_orders = Arc::clone(&self.buy_orders);
        let sell_orders = Arc::clone(&self.sell_orders);
        let rx_mutex = Arc::clone(&rx);

        thread::spawn(move || {
            let rx_mutex_guard = rx_mutex.lock().unwrap();
            match rx_mutex_guard.recv_timeout(Duration::from_secs(5)) {
                Ok(_) => {
                    let mut index = 0;
                    while !stop.load(Ordering::Relaxed) {
                        let index_len = index + 1;
                        let mut buy_orders = buy_orders.lock().unwrap();
                        let mut sell_orders = sell_orders.lock().unwrap();

                        if index_len > buy_orders.len() || index_len > sell_orders.len() {
                            stop.store(true, Ordering::Relaxed);
                        }

                        if let Some(max_buy_order) = buy_orders.get_mut(index) {
                            let min_sell_order =
                                sell_orders.get_mut(index).expect("No sell orders");

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
                }
                Err(_) => {
                    drop(rx_mutex_guard);
                    stop.store(true, Ordering::SeqCst)
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn order_book_should_be_empty() {
        let (tx, _rx) = mpsc::channel();
        let order_book = OrderBook::new(tx);
        assert_eq!(order_book.get_sell_orders().len(), 0);
        assert_eq!(order_book.get_buy_orders().len(), 0);
    }

    #[test]
    fn should_not_add_sell_order_to_buy_order() {
        let (tx, _rx) = mpsc::channel();
        let mut order_book = OrderBook::new(tx);
        let sell = Order::new(1, 8, OrderType::Sell);
        let error = order_book.append_buy_order(sell).unwrap_err();

        assert_eq!(
            format!("{}", error),
            "Invalid order type, expected Buy order type but Sell provided"
        );
        assert_eq!(order_book.get_buy_orders().len(), 0);
        assert_eq!(order_book.get_sell_orders().len(), 0);
    }

    #[test]
    fn should_not_add_buy_order_to_sell_order() {
        let (tx, _rx) = mpsc::channel();
        let mut order_book = OrderBook::new(tx);
        let buy = Order::new(1, 8, OrderType::Buy);
        let error = order_book.append_sell_order(buy).unwrap_err();

        assert_eq!(
            format!("{}", error),
            "Invalid order type, expected Sell order type but Buy provided"
        );
        assert_eq!(order_book.get_buy_orders().len(), 0);
        assert_eq!(order_book.get_sell_orders().len(), 0);
    }

    #[test]
    fn buy_order_test() {
        let (tx, rx) = mpsc::channel();
        let mut order_book = OrderBook::new(tx);
        let orders: [Order; 3] = [
            Order::new(1, 5, OrderType::Buy),
            Order::new(1, 1, OrderType::Buy),
            Order::new(1, 10, OrderType::Buy),
        ];

        for order in orders {
            order_book
                .append_buy_order(order.clone())
                .expect("failed to append buy order");
            assert_eq!(rx.try_recv().unwrap(), order);
        }

        assert_eq!(order_book.get_buy_orders().first(), Some(&orders[2]));
    }

    #[test]
    fn sell_order_should_be_sorted_asc() {
        let (tx, rx) = mpsc::channel();
        let mut order_book = OrderBook::new(tx);
        let orders: [Order; 3] = [
            Order::new(1, 5, OrderType::Sell),
            Order::new(1, 1, OrderType::Sell),
            Order::new(1, 10, OrderType::Sell),
        ];

        for order in orders {
            order_book
                .append_sell_order(order.clone())
                .expect("failed to append buy order");
            assert_eq!(rx.try_recv().unwrap(), order)
        }

        assert_eq!(order_book.get_sell_orders().first(), Some(&orders[1]));
    }

    #[test]
    // Buy | Sell
    //  3 | 2
    //  2 | 2
    //  1 | 9
    fn test() {
        let (tx, rx) = mpsc::channel();
        let mut order_book = OrderBook::new(tx);

        let orders: [Order; 6] = [
            Order::new(1, 2, OrderType::Sell),
            Order::new(1, 9, OrderType::Sell),
            Order::new(1, 2, OrderType::Sell),
            //
            Order::new(1, 1, OrderType::Buy),
            Order::new(1, 3, OrderType::Buy),
            Order::new(1, 2, OrderType::Buy),
        ];

        for order in orders {
            if order.order_type == OrderType::Buy {
                order_book
                    .append_buy_order(order.clone())
                    .expect("could not append buy order");
            } else {
                order_book
                    .append_sell_order(order.clone())
                    .expect("could not append sell order");
            }
        }

        order_book.match_orders(Arc::new(Mutex::new(rx)));
        thread::sleep(Duration::from_secs(10));

        println!("BUY_ORDERS: {:?}", order_book.get_filled_buy_orders());
        println!("SELL_ORDERS: {:?}", order_book.get_filled_sell_orders());

        assert_eq!(order_book.get_active_buy_orders().first().unwrap().price, 1);
        assert_eq!(
            order_book.get_active_sell_orders().first().unwrap().price,
            9
        );
    }
}
