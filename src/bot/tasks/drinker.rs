use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::bot::actions::open_belt::OpenBelt;
use crate::bot::actions::use_item::UseItem;
use crate::bot::player::Item;
use crate::bot::protocol::{Message, Update};
use crate::bot::scene::Scene;
use crate::bot::tasks::task::Task;
use crate::bot::world::PlayerWorld;

#[derive(Clone, Deserialize)]
pub struct DrinkerConfig {
    pub open_belt_timeout: f64,
    pub sip_timeout: f64,
    pub max_stamina: i32,
    pub stamina_threshold: i32,
    pub liquid_containers: BTreeSet<String>,
    pub contents: Vec<ContentConfig>,
}

#[derive(Clone, Deserialize)]
pub struct ContentConfig {
    pub name: String,
    pub action: String,
    pub wait_interval: f64,
}

pub struct Drinker {
    open_belt: OpenBelt,
    sip: Option<UseItem>,
    wait_interval: Option<Duration>,
    last_sip: Option<Instant>,
    config: DrinkerConfig,
}

impl Drinker {
    pub fn new(config: DrinkerConfig) -> Self {
        Self {
            open_belt: OpenBelt::new(Duration::from_secs_f64(config.open_belt_timeout)),
            sip: None,
            wait_interval: None,
            last_sip: None,
            config,
        }
    }
}

impl Task for Drinker {
    fn name(&self) -> &'static str {
        "Drinker"
    }

    fn get_next_message(&mut self, world: &PlayerWorld, _: &Scene) -> Option<Message> {
        if world.player_stamina() >= self.config.max_stamina {
            debug!("Drinker: max stamina");
            self.sip = None;
            return Some(Message::Done { task: String::from("Drinker") });
        }
        let mut reset_sip = false;
        if let Some(sip) = self.sip.as_mut() {
            let sip_item_id = sip.item_id();
            if find_container_with_content(world, &self.config.liquid_containers, &self.config.contents)
                .map(|(v, _, _)| v == sip_item_id).unwrap_or(false) {
                match sip.get_next_message() {
                    Some(Message::Done { .. }) => (),
                    v => return v,
                }
            }
            reset_sip = true;
        }
        if reset_sip || world.player_stamina() > self.config.stamina_threshold {
            debug!("Drinker: reset sip");
            self.sip = None;
            self.last_sip = Some(Instant::now());
            return None;
        }
        if self.sip.is_some() {
            debug!("Drinker: sipping");
            return None;
        }
        if self.last_sip.map(|v| self.wait_interval.map(|w| Instant::now() - v < w).unwrap_or(false)).unwrap_or(false) {
            debug!("Drinker: wait");
            return None;
        }
        match self.open_belt.get_next_message(world) {
            Some(Message::Done { .. }) => (),
            Some(Message::Error { message }) => debug!("Drinker: {:?}", message),
            v => return v,
        }
        debug!("Drinker: try drink");
        let (sip, wait_interval) = {
            find_container_with_content(world, &self.config.liquid_containers, &self.config.contents)
                .map(|(id, action, wait_interval)| {
                    (
                        Some(UseItem::new(id, action.clone(), Duration::from_secs_f64(self.config.sip_timeout))),
                        Some(wait_interval)
                    )
                })
                .unwrap_or((None, None))
        };
        self.sip = sip;
        self.wait_interval = wait_interval;
        self.sip.as_mut().and_then(|v| v.get_next_message())
    }

    fn update(&mut self, _: &PlayerWorld, update: &Update) {
        if let Some(sip) = self.sip.as_mut() {
            sip.update(update);
        }
    }

    fn restore(&mut self, _: &PlayerWorld) {}
}

fn find_container_with_content<'a>(world: &PlayerWorld, liquid_containers: &BTreeSet<String>, contents: &'a Vec<ContentConfig>) -> Option<(i32, &'a String, Duration)> {
    contents.iter()
        .find_map(|config| {
            match world.player_belt_items().map(|belt_items| belt_items.iter()) {
                Some(v) => find_container_with_content_iter(v, world, liquid_containers, config),
                None => find_container_with_content_iter(std::iter::empty(), world, liquid_containers, config),
            }
        })
}

fn find_container_with_content_iter<'a, 'b, I>(iter: I, world: &'b PlayerWorld,
                                               liquid_containers: &BTreeSet<String>,
                                               config: &'a ContentConfig) -> Option<(i32, &'a String, Duration)>
    where I: Iterator<Item=(&'b i32, &'b Item)> {
    iter.chain(world.player_inventory_items().iter())
        .find_map(|(_, item)| {
            item.content.as_ref()
                .and_then(|v| {
                    if v.name.contains(&config.name) {
                        world.resources().get(&item.resource)
                    } else {
                        None
                    }
                })
                .and_then(|v| {
                    if liquid_containers.contains(&v.name) {
                        Some((item.id, &config.action, Duration::from_secs_f64(config.wait_interval)))
                    } else {
                        None
                    }
                })
        })
}
