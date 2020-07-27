#[macro_export]
macro_rules! bind {
    ($type:ty) => {
        arcadeum::bind!(card_movement_simulator::GameState<$type>);
    };
}
