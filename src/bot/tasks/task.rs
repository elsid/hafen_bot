use crate::bot::protocol::{Message, Update};
use crate::bot::scene::Scene;
use crate::bot::world::PlayerWorld;

pub trait Task: Send {
    fn name(&self) -> &'static str;

    fn get_next_message(&mut self, world: &PlayerWorld, scene: &Scene) -> Option<Message>;

    fn update(&mut self, world: &PlayerWorld, update: &Update);

    fn restore(&mut self, world: &PlayerWorld);
}
