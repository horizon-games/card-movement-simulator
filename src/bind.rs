#[macro_export]
macro_rules! bind {
    ($type:ty) => {
        use $crate::arcadeum;
        arcadeum::bind!(card_movement_simulator::GameState<$type>);
    };
}
