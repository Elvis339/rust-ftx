use db::Database;
use match_engine::order::{Order, OrderStatus, OrderType};
use match_engine::order_book::{Item, OrderBook};
use std::env;
use std::sync::{Arc, Mutex};

fn main() {
    let print = env::args()
        .nth(2)
        .map(|arg| arg == "print".to_string())
        .unwrap_or(false);
    let commands: [String; 3] = [
        "print".to_string(),
        "create_order".to_string(),
        "list_order".to_string(),
    ];
    let db = Arc::new(Mutex::new(Database::new(Some("order_book.db".to_string()))));
    let mut order_book_builder = OrderBook::default();
    order_book_builder.set_db(db.clone());

    match env::args().nth(2) {
        Some(arg) => match arg.as_str() {
            "print" => {
                let pair = env::args()
                    .nth(3)
                    .expect("Pair is required. Example: print btc/usd");
                let json = db.clone().lock().expect("could not get db lock").get(&pair);
                let item: Item = serde_json::from_str(
                    &json
                        .expect("could not get fetch orders")
                        .expect("sam bankman took the money"),
                )
                .expect(format!("Could not deserialize {}", pair).as_str());

                println!("Active orders={:?}", item.active_orders);
                println!("Fulfilled orders={:?}", item.fulfilled_orders);
            }
            "order" => {
                let err_msg = "Invalid usage! Example: order btc/usd [[represents pair]] buy [[or sell]] 10 [[price]] 3 [[quantity]] (default: 1)";
                let pair = env::args().nth(3).expect(err_msg);
                let order_type = env::args()
                    .nth(4)
                    .map(|a| {
                        if a == "sell" {
                            OrderType::Sell
                        } else {
                            OrderType::Buy
                        }
                    })
                    .expect(err_msg);
                let price = env::args()
                    .nth(5)
                    .map(|p| p.parse::<i32>().expect("Please provide a number"))
                    .expect(err_msg);
                let quantity = env::args()
                    .nth(6)
                    .map(|q| q.parse::<i32>().expect("Please provide a number"))
                    .unwrap_or(1);
                order_book_builder.set_pair(pair.clone());
                let mut order_book = order_book_builder.build();
                order_book.load();

                if order_type == OrderType::Buy {
                    order_book
                        .append_buy_order(Order::new(quantity, price, order_type))
                        .expect("Invalid Order arguments");
                } else {
                    order_book
                        .append_sell_order(Order::new(quantity, price, order_type))
                        .expect("Invalid Order arguments");
                }
                println!("Orders={:?}", order_book.join_active_orders());
            }
            _ => {}
        },
        None => {
            for cmd in commands {
                println!("Command={cmd}");
            }
        }
    }
}
