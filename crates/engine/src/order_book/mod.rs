use std::collections::BinaryHeap;

use anyhow::anyhow;
use sorted_insert::SortedInsertByKey;

use crate::order::{Order, OrderType};

#[derive(Debug)]
pub struct OrderBook {
    buy_orders: BinaryHeap<Order>,
    sell_orders: Vec<Order>,
}

impl<'a> OrderBook {
    pub fn new() -> Self {
        Self {
            buy_orders: BinaryHeap::new(),
            sell_orders: Vec::new(),
        }
    }

    pub fn get_buy_orders(&self) -> &BinaryHeap<Order> {
        &self.buy_orders
    }

    pub fn get_sell_orders(&self) -> &Vec<Order> {
        &self.sell_orders
    }

    pub fn append_buy_order(&mut self, order: Order) -> anyhow::Result<()> {
        match order.order_type {
            OrderType::Buy => {
                self.buy_orders.push(order);
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
                self.sell_orders
                    .sorted_insert_asc_by_key(order, |o| &o.price);
                Ok(())
            }
            _ => Err(anyhow!(
                "Invalid order type, expected Sell order type but Buy provided"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_book_should_be_empty() {
        let order_book = OrderBook::new();
        assert_eq!(order_book.get_sell_orders().len(), 0);
        assert_eq!(order_book.get_buy_orders().len(), 0);
    }

    #[test]
    fn should_not_add_sell_order_to_buy_order() {
        let mut order_book = OrderBook::new();
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
        let mut order_book = OrderBook::new();
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
    fn buy_order_should_be_sorted_desc() {
        let mut order_book = OrderBook::new();
        let orders: [Order; 3] = [
            Order::new(1, 5, OrderType::Buy),
            Order::new(1, 1, OrderType::Buy),
            Order::new(1, 10, OrderType::Buy),
        ];

        for order in orders {
            order_book
                .append_buy_order(order)
                .expect("failed to append buy order");
        }

        assert_eq!(order_book.get_buy_orders().peek(), Some(&orders[2]));
    }

    #[test]
    fn sell_order_should_be_sorted_asc() {
        let mut order_book = OrderBook::new();
        let orders: [Order; 3] = [
            Order::new(1, 5, OrderType::Sell),
            Order::new(1, 1, OrderType::Sell),
            Order::new(1, 10, OrderType::Sell),
        ];

        for order in orders {
            order_book
                .append_sell_order(order)
                .expect("failed to append buy order");
        }

        assert_eq!(order_book.sell_orders[0], orders[1]);
    }
}
