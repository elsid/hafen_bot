use serde::{Deserialize, Serialize};

use crate::bot::bot::Bot;
use crate::bot::exp_wnd_closer::ExpWndCloser;
use crate::bot::explorer::Explorer;
use crate::bot::player::{Player, PlayerData};
use crate::bot::protocol::{Message, Update};
use crate::bot::world::{World, WorldData};

pub struct Session {
    id: i64,
    last_update: i64,
    world: World,
    player: Player,
    bots: Vec<Box<dyn Bot>>,
}

impl Session {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            last_update: 0,
            world: World::new(),
            player: Player::default(),
            bots: Vec::new(),
        }
    }

    pub fn from_session_data(session_data: SessionData) -> Self {
        Self {
            id: session_data.id,
            last_update: 0,
            world: World::from_world_data(session_data.world),
            player: Player::from_player_data(session_data.player),
            bots: Vec::new(),
        }
    }

    pub fn as_session_data(&self) -> SessionData {
        SessionData {
            id: self.id,
            last_update: self.last_update,
            world: self.world.as_world_data(),
            player: self.player.as_player_data(),
        }
    }

    pub fn get_bots(&self) -> Vec<String> {
        self.bots.iter().map(|v| String::from(v.name())).collect()
    }

    pub fn add_bot(&mut self, name: &str, params: &[u8]) -> Result<(), String> {
        self.bots.push(make_bot(name, params)?);
        Ok(())
    }

    pub fn clear_bots(&mut self) {
        self.bots.clear();
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
        if let Some(world) = self.world.for_player(&self.player) {
            for bot in self.bots.iter_mut() {
                bot.update(&world, &update);
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

    pub fn get_next_message(&mut self) -> Option<Message> {
        if let Some(world) = self.world.for_player(&self.player) {
            for bot in self.bots.iter_mut() {
                if let Some(v) = bot.get_next_message(&world) {
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
}

fn make_bot(name: &str, _params: &[u8]) -> Result<Box<dyn Bot>, String> {
    match name {
        "Explorer" => Ok(Box::new(Explorer::new())),
        "ExpWndCloser" => Ok(Box::new(ExpWndCloser::new())),
        _ => Err(String::from("Bot is not found")),
    }
}

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    id: i64,
    last_update: i64,
    world: WorldData,
    player: PlayerData,
}

impl SessionData {
    pub fn read_from_file(path: &str) -> Result<Self, String> {
        let data = match std::fs::read(path) {
            Ok(v) => v,
            Err(e) => return Err(format!("Session read file \"{}\" error: {}", path, e)),
        };
        match serde_json::from_slice::<SessionData>(data.as_slice()) {
            Ok(v) => Ok(v),
            Err(e) => return Err(format!("Session deserialization error: {}", e)),
        }
    }

    pub fn write_to_file(&self, path: &str) -> Result<(), String> {
        let data = match serde_json::to_vec(self) {
            Ok(v) => v,
            Err(e) => return Err(format!("Session write to file \"{}\" error: {}", path, e)),
        };
        match std::fs::write(path, data.as_slice()) {
            Ok(_) => Ok(()),
            Err(e) => return Err(format!("Session serialization error: {}", e)),
        }
    }
}
