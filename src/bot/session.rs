use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::bot::bot::Bot;
use crate::bot::exp_wnd_closer::ExpWndCloser;
use crate::bot::explorer::Explorer;
use crate::bot::new_character::{NewCharacter, NewCharacterParams};
use crate::bot::player::{Player, PlayerData};
use crate::bot::protocol::{Message, Update};
use crate::bot::world::{World, WorldData};

pub struct Session {
    id: i64,
    last_update: i64,
    world: World,
    player: Player,
    bots: Arc<RwLock<Vec<Arc<RwLock<BotWithParams>>>>>,
}

struct BotWithParams {
    name: String,
    params: Vec<u8>,
    value: Arc<Mutex<dyn Bot>>,
}

impl Session {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            last_update: 0,
            world: World::new(),
            player: Player::default(),
            bots: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn from_session_data(session_data: SessionData) -> Result<Self, String> {
        Ok(Self {
            id: session_data.id,
            last_update: 0,
            world: World::from_world_data(session_data.world),
            player: Player::from_player_data(session_data.player),
            bots: {
                let mut bots = Vec::new();
                for bot in session_data.bots.into_iter() {
                    bots.push(Arc::new(RwLock::new(BotWithParams {
                        value: make_bot(bot.name.as_str(), bot.params.as_slice())?,
                        name: bot.name,
                        params: bot.params,
                    })));
                }
                Arc::new(RwLock::new(bots))
            },
        })
    }

    pub fn as_session_data(&self) -> SessionData {
        SessionData {
            id: self.id,
            last_update: self.last_update,
            world: self.world.as_world_data(),
            player: self.player.as_player_data(),
            bots: self.bots.read().unwrap().iter()
                .map(Arc::clone)
                .map(|v| {
                    let locked = v.read().unwrap();
                    BotParams { name: locked.name.clone(), params: locked.params.clone() }
                })
                .collect(),
        }
    }

    pub fn get_bots(&self) -> Vec<String> {
        self.bots.read().unwrap().iter()
            .map(|v| v.read().unwrap().name.clone())
            .collect()
    }

    pub fn add_bot(&mut self, name: &str, params: &[u8]) -> Result<(), String> {
        self.bots.write().unwrap().push(Arc::new(RwLock::new(BotWithParams {
            name: String::from(name),
            params: Vec::from(params),
            value: make_bot(name, params)?,
        })));
        Ok(())
    }

    pub fn clear_bots(&self) {
        self.bots.write().unwrap().clear();
    }

    pub fn update(&mut self, update: Update) -> bool {
        if update.number <= self.last_update {
            info!("Got stale update for session {}: number={} last_number={}", self.id, update.number, self.last_update);
            return false;
        }
        if update.number - self.last_update > 1 {
            warn!("Missed {} updates for session {}", update.number - self.last_update - 1, self.id);
        }
        self.last_update = update.number;
        info!("Got new update for session {}: {:?}", self.id, update);
        for bot in self.bots.read().unwrap().iter().map(Arc::clone) {
            bot.read().unwrap().value.lock().unwrap().update(&update);
        }
        let mut updated = false;
        if self.player.update(&update) {
            updated = true;
        }
        if self.world.update(update) {
            updated = true;
        }
        updated
    }

    pub fn get_next_message(&self) -> Option<Message> {
        if let Some(world) = self.world.for_player(&self.player) {
            for bot in self.bots.read().unwrap().iter().map(Arc::clone) {
                if let Some(v) = bot.read().unwrap().value.lock().unwrap().get_next_message(&world) {
                    info!("Next message for session {}: {:?}", self.id, v);
                    return Some(v);
                }
            }
            warn!("No messages from any bot for session {}", self.id);
            None
        } else {
            warn!("World is not configured for session {}", self.id);
            None
        }
    }
}

fn make_bot(name: &str, params: &[u8]) -> Result<Arc<Mutex<dyn Bot>>, String> {
    match name {
        "Explorer" => Ok(Arc::new(Mutex::new(Explorer::new()))),
        "ExpWndCloser" => Ok(Arc::new(Mutex::new(ExpWndCloser::new()))),
        "NewCharacter" => {
            match serde_json::from_slice::<NewCharacterParams>(params) {
                Ok(parsed) => Ok(Arc::new(Mutex::new(NewCharacter::new(parsed)))),
                Err(e) => Err(format!("Failed to parse {} bot params: {}", name, e)),
            }
        }
        _ => Err(String::from("Bot is not found")),
    }
}

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    id: i64,
    last_update: i64,
    world: WorldData,
    player: PlayerData,
    bots: Vec<BotParams>,
}

#[derive(Serialize, Deserialize)]
struct BotParams {
    name: String,
    params: Vec<u8>,
}

impl SessionData {
    pub fn read_from_file(path: &str) -> Result<Self, String> {
        let data = match std::fs::read(path) {
            Ok(v) => v,
            Err(e) => return Err(format!("Session read file error: {}", e)),
        };
        match serde_json::from_slice::<SessionData>(data.as_slice()) {
            Ok(v) => Ok(v),
            Err(e) => return Err(format!("Session deserialization error: {}", e)),
        }
    }

    pub fn write_to_file(&self, path: &str) -> Result<(), String> {
        let data = match serde_json::to_vec(self) {
            Ok(v) => v,
            Err(e) => return Err(format!("Session serialization error: {}", e)),
        };
        match std::fs::write(path, data.as_slice()) {
            Ok(_) => Ok(()),
            Err(e) => return Err(format!("Session deserialization error: {}", e)),
        }
    }
}
