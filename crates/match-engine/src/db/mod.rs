use std::collections::HashMap;

use lazy_static::lazy_static;

use crate::order::Order;

lazy_static! {
    pub static ref DB: HashMap<String, Order> = HashMap::new();
}
