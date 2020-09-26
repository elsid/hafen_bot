use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use crate::bot::bot::Bot;
use crate::bot::exp_wnd_closer::ExpWndCloser;
use crate::bot::explorer::{Explorer, ExplorerConfig};
use crate::bot::map_db::MapDb;
use crate::bot::new_character::{NewCharacter, NewCharacterParams};
use crate::bot::path_finder::{PathFinder, PathFinderConfig};
use crate::bot::player::{Player, PlayerData};
use crate::bot::protocol::{Event, Message, Update, Value};
use crate::bot::scene::Scene;
use crate::bot::world::{PlayerWorld, World, WorldConfig, WorldData};

#[derive(Clone, Deserialize)]
pub struct SessionConfig {
    world: WorldConfig,
    bots: BotConfigs,
}

#[derive(Clone, Deserialize)]
pub struct BotConfigs {
    path_finder: PathFinderConfig,
    explorer: ExplorerConfig,
}

pub struct Session {
    id: i64,
    last_update: i64,
    world: World,
    player: Player,
    bot_id_counter: i64,
    bots: Arc<RwLock<Vec<Arc<RwLock<BotWithParams>>>>>,
    scene: Scene,
    messages: Arc<Mutex<VecDeque<Message>>>,
    bot_configs: BotConfigs,
    cancel: Arc<AtomicBool>,
}

struct BotWithParams {
    id: i64,
    name: String,
    params: Vec<u8>,
    value: Arc<Mutex<dyn Bot>>,
}

impl Session {
    pub fn new(id: i64, map_db: Arc<Mutex<dyn MapDb + Send>>, config: &SessionConfig, cancel: Arc<AtomicBool>) -> Self {
        Self {
            id,
            last_update: 0,
            world: World::new(config.world.clone(), map_db),
            player: Player::default(),
            bot_id_counter: 0,
            bots: Arc::new(RwLock::new(Vec::new())),
            scene: Scene::new(),
            messages: Arc::new(Mutex::new(VecDeque::new())),
            bot_configs: config.bots.clone(),
            cancel,
        }
    }

    pub fn from_session_data(session_data: SessionData, map_db: Arc<Mutex<dyn MapDb + Send>>,
                             config: &SessionConfig, cancel: Arc<AtomicBool>) -> Result<Self, String> {
        Ok(Self {
            id: session_data.id,
            last_update: 0,
            player: Player::from_player_data(session_data.player),
            bot_id_counter: session_data.bot_id_counter,
            bots: {
                let mut bots = Vec::new();
                for bot in session_data.bots.into_iter() {
                    bots.push(Arc::new(RwLock::new(BotWithParams {
                        id: bot.id,
                        value: make_bot(bot.name.as_str(), bot.params.as_slice(), &config.bots, &cancel)?,
                        name: bot.name,
                        params: bot.params,
                    })));
                }
                Arc::new(RwLock::new(bots))
            },
            world: World::from_world_data(session_data.world, config.world.clone(), map_db),
            scene: Scene::new(),
            messages: Arc::new(Mutex::new(VecDeque::new())),
            bot_configs: config.bots.clone(),
            cancel,
        })
    }

    pub fn as_session_data(&self) -> SessionData {
        SessionData {
            id: self.id,
            last_update: self.last_update,
            world: self.world.as_world_data(),
            player: self.player.as_player_data(),
            bot_id_counter: self.bot_id_counter,
            bots: self.bots.read().unwrap().iter()
                .map(Arc::clone)
                .map(|v| {
                    let locked = v.read().unwrap();
                    BotParams {
                        id: locked.id,
                        name: locked.name.clone(),
                        params: locked.params.clone(),
                    }
                })
                .collect(),
        }
    }

    pub fn get_bots(&self) -> Vec<String> {
        self.bots.read().unwrap().iter()
            .map(|v| v.read().unwrap().name.clone())
            .collect()
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn add_bot(&mut self, name: &str, params: &[u8]) -> Result<(), String> {
        self.bot_id_counter += 1;
        let id = self.bot_id_counter;
        self.bots.write().unwrap().push(Arc::new(RwLock::new(BotWithParams {
            id,
            name: String::from(name),
            params: Vec::from(params),
            value: make_bot(name, params, &self.bot_configs, &self.cancel)?,
        })));
        if let Some(game_ui_id) = self.player.game_ui_id() {
            self.messages.lock().unwrap().push_back(Message::UIMessage {
                id: game_ui_id,
                kind: String::from("add-bot"),
                arguments: vec![
                    Value::from(id),
                    Value::from(String::from(name)),
                    Value::from(Vec::from(params)),
                ],
            });
        }
        Ok(())
    }

    pub fn remove_bot(&mut self, id: i64) {
        let mut removed = false;
        self.bots.write().unwrap().retain(|bot| {
            if bot.read().unwrap().id == id {
                removed = true;
                false
            } else {
                true
            }
        });
        if removed {
            if let Some(world) = self.world.for_player(&self.player) {
                self.messages.lock().unwrap().push_back(Message::UIMessage {
                    id: world.game_ui_id(),
                    kind: String::from("remove-bot"),
                    arguments: vec![Value::from(id)],
                });
            }
        }
    }

    pub fn clear_bots(&self) {
        let mut locked = self.bots.write().unwrap();
        if let Some(world) = self.world.for_player(&self.player) {
            for bot in locked.iter() {
                self.messages.lock().unwrap().push_back(Message::UIMessage {
                    id: world.game_ui_id(),
                    kind: String::from("remove-bot"),
                    arguments: vec![Value::from(bot.read().unwrap().id)],
                });
            }
        }
        locked.clear();
    }

    pub fn update(&mut self, update: Update) -> bool {
        if update.number <= self.last_update {
            warn!("Got stale update for session {}: number={} last_number={}", self.id, update.number, self.last_update);
            return false;
        }
        if update.number - self.last_update > 1 {
            warn!("Missed {} updates for session {}", update.number - self.last_update - 1, self.id);
        }
        self.last_update = update.number;
        debug!("Got new update for session {}: {:?}", self.id, update);
        match &update.event {
            Event::BotAdd { name, params } => {
                match self.add_bot(name, params) {
                    Ok(_) => (),
                    Err(e) => error!("Failed to add bot: {:?}", e),
                }
            }
            Event::BotRemove { id } => {
                self.remove_bot(*id);
            }
            _ => (),
        }
        if let Some(world) = self.world.for_player(&self.player) {
            for bot in self.bots.read().unwrap().iter().map(Arc::clone) {
                bot.read().unwrap().value.lock().unwrap().update(&world, &update);
            }
        }
        let mut updated = false;
        if self.player.update(&self.world, &update) {
            updated = true;
        }
        if self.world.update(update) {
            updated = true;
        }
        updated
    }

    pub fn get_next_message(&self) -> Option<Message> {
        if let Some(bot) = self.messages.lock().unwrap().pop_front() {
            return Some(bot);
        }
        if let Some(world) = self.world.for_player(&self.player) {
            for bot in self.bots.read().unwrap().iter().map(Arc::clone) {
                if let Some(v) = bot.read().unwrap().value.lock().unwrap().get_next_message(&world, &self.scene) {
                    debug!("Next message for session {}: {:?}", self.id, v);
                    return Some(v);
                }
            }
            debug!("No messages from any bot for session {}", self.id);
            None
        } else {
            debug!("World is not configured for session {}", self.id);
            None
        }
    }

    pub fn get_player_world(&self) -> Option<PlayerWorld> {
        self.world.for_player(&self.player)
    }
}

fn make_bot(name: &str, params: &[u8], bot_configs: &BotConfigs, cancel: &Arc<AtomicBool>) -> Result<Arc<Mutex<dyn Bot>>, String> {
    match name {
        "Explorer" => Ok(Arc::new(Mutex::new(Explorer::new(bot_configs.explorer.clone(), cancel.clone())))),
        "ExpWndCloser" => Ok(Arc::new(Mutex::new(ExpWndCloser::new()))),
        "NewCharacter" => {
            match serde_json::from_slice::<NewCharacterParams>(params) {
                Ok(parsed) => Ok(Arc::new(Mutex::new(NewCharacter::new(parsed)))),
                Err(e) => Err(format!("Failed to parse {} bot params: {}", name, e)),
            }
        }
        "PathFinder" => Ok(Arc::new(Mutex::new(PathFinder::new(bot_configs.path_finder.clone(), cancel.clone())))),
        _ => Err(String::from("Bot is not found")),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SessionData {
    id: i64,
    last_update: i64,
    world: WorldData,
    player: PlayerData,
    bot_id_counter: i64,
    bots: Vec<BotParams>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct BotParams {
    id: i64,
    name: String,
    params: Vec<u8>,
}
