#[macro_export]
macro_rules! bind {
    ($type:ty) => {
        use $crate::arcadeum;
        arcadeum::bind!($crate::GameState<$type>);
    };
}
