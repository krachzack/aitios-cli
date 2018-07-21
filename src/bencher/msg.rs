use std::time::Duration;

pub enum Msg {
    Done,
    Persist(Duration),
}
