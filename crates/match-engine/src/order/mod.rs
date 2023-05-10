use std::cmp::Ordering;

use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OrderStatus {
    Filled,
    Active,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Order {
    pub id: Uuid,
    pub price: i32,
    pub quantity: i32,
    pub order_type: OrderType,
    pub order_status: OrderStatus,
}

impl Order {
    pub fn new(quantity: i32, price: i32, order_type: OrderType) -> Self {
        Self {
            id: Uuid::new_v4(),
            quantity,
            price,
            order_type,
            order_status: OrderStatus::Active,
        }
    }

    pub fn update_order_type(&mut self, new_order_type: OrderType) {
        self.order_type = new_order_type;
    }

    pub fn update_order_status(&mut self, new_order_status: OrderStatus) {
        self.order_status = new_order_status;
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.order_type {
            OrderType::Buy => self.price.cmp(&other.price),
            _ => other.price.cmp(&self.price),
        }
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.order_type {
            OrderType::Buy => Some(self.price.cmp(&other.price)),
            _ => Some(other.price.cmp(&other.price)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_order_should_have_active_status() {
        let order = Order::new(10, 30, OrderType::Buy);
        assert_eq!(order.order_status, OrderStatus::Active);
    }

    #[test]
    fn update_order_type_test() {
        let mut order = Order::new(10, 30, OrderType::Buy);
        order.update_order_type(OrderType::Sell);

        assert_eq!(order.order_type, OrderType::Sell);
    }

    #[test]
    fn update_order_status_test() {
        let mut order = Order::new(10, 30, OrderType::Sell);
        order.update_order_status(OrderStatus::Filled);

        assert_eq!(order.order_status, OrderStatus::Filled);
    }
}
