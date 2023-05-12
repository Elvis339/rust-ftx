use sled::{Db, IVec};

#[derive(Debug, Clone)]
pub struct Database {
    inner: Db,
}

impl Database {
    pub fn new(name: Option<String>) -> Self {
        match name {
            Some(name) => Self {
                inner: sled::open(name.clone())
                    .expect(format!("Failed to connect to {}", name).as_str()),
            },
            None => Self {
                inner: sled::open("order_book.db").expect("Failed to connect to order_book.db"),
            },
        }
    }

    pub fn set<T>(&self, key: &String, value: &T) -> sled::Result<Option<IVec>>
    where
        T: Sized + serde::Serialize,
    {
        let stringify = serde_json::to_string(&value).expect("Failed to stringify");
        self.inner.insert(key, stringify.as_bytes())
    }

    pub fn get(&self, key: &String) -> String {
        let key = self
            .inner
            .get(&key)
            .expect(format!("Failed to get {}", key).as_str())
            .expect(format!("{} does not exist", key).as_str());
        String::from_utf8(key.to_vec()).expect("Could not convert Vec<u8> to String")
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;
    use rand::prelude::*;
    use serde::{Deserialize, Serialize};
    use std::fs;
    use std::path::Path;

    #[derive(Debug, Serialize, Deserialize)]
    struct Complex {
        id: String,
        active_orders: Vec<u32>,
        fulfilled_orders: Vec<u32>,
    }

    fn create_mock_db() -> Database {
        Database::new(Some("mock.db".to_string()))
    }

    fn cleanup() {
        if Path::new("mock.db").exists() {
            fs::remove_dir_all("mock.db").expect("could not delete mock.db")
        }
    }

    fn gen_rnd_complex_obj(num: usize) -> Vec<Complex> {
        let mut rng = thread_rng();
        let mut objs: Vec<Complex> = Vec::with_capacity(num);

        while objs.len() < num {
            let rand_active_order: u32 = rng.gen();
            let rand_filfilled_order: u32 = rng.gen();

            objs.push(Complex {
                id: "btc/usdc".to_string(),
                active_orders: vec![rand_active_order; 5],
                fulfilled_orders: vec![rand_filfilled_order; 5],
            });
        }

        objs
    }

    #[test]
    fn set_test() {
        let complex = Complex {
            id: "Hello".to_string(),
            active_orders: vec![1, 2, 3],
            fulfilled_orders: vec![],
        };
        let db = create_mock_db();
        let key = "BTC/USD".to_string();
        db.set(&key, &complex).expect("failed to insert");

        let stringified = db.get(&key);
        let converted: Complex =
            serde_json::from_str(&*stringified).expect("failed to deserialize");

        assert_eq!(&complex.id, &converted.id);
        assert_eq!(&complex.fulfilled_orders, &converted.fulfilled_orders);
        assert_eq!(&complex.active_orders, &converted.active_orders);
        cleanup();
    }

    #[test]
    fn multiple_set_latest_get() {
        let db = create_mock_db();
        let btc_usdc: Vec<Complex> = gen_rnd_complex_obj(10);

        for index in 0..10 {
            db.set(&"btc/usdc".to_string(), &btc_usdc[index]).unwrap();
        }

        assert_eq!(
            db.get(&"btc/usdc".to_string()),
            serde_json::to_string(&btc_usdc[9]).unwrap()
        );
        cleanup();
    }
}
