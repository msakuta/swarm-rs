#[macro_export]
macro_rules! unwrap_return {
    {$result:expr} => {
        match $result {
            Ok(res) => res,
            Err(err) => {
                println!("Oops: {:?}", err);
                return;
            }
        }
    }
}
