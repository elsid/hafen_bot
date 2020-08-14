use crate::bot::protocol::Message;
use crate::bot::Update;
use crate::bot::world::PlayerWorld;

pub trait Bot: Send {
    fn name(&self) -> &'static str;

    fn get_next_message(&mut self, world: &PlayerWorld) -> Option<Message>;

    fn update(&mut self, update: &Update);
}
