use serde::{Deserialize, Serialize};

use crate::bot::player::{Player, PlayerData};
use crate::bot::protocol::{Message, Update};
use crate::bot::tasks::exp_wnd_closer::ExpWndCloser;
use crate::bot::tasks::explorer::Explorer;
use crate::bot::tasks::new_character::{NewCharacter, NewCharacterParams};
use crate::bot::tasks::path_finder::PathFinder;
use crate::bot::tasks::task::Task;
use crate::bot::world::{World, WorldData};

pub struct Session {
    id: i64,
    last_update: i64,
    world: World,
    player: Player,
    tasks: Vec<TaskWithParams>,
}

struct TaskWithParams {
    name: String,
    params: Vec<u8>,
    value: Box<dyn Task>,
}

impl Session {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            last_update: 0,
            world: World::new(),
            player: Player::default(),
            tasks: Vec::new(),
        }
    }

    pub fn from_session_data(session_data: SessionData) -> Result<Self, String> {
        Ok(Self {
            id: session_data.id,
            last_update: 0,
            world: World::from_world_data(session_data.world),
            player: Player::from_player_data(session_data.player),
            tasks: {
                let mut tasks = Vec::new();
                for task in session_data.tasks.into_iter() {
                    tasks.push(TaskWithParams {
                        value: make_task(task.name.as_str(), task.params.as_slice())?,
                        name: task.name,
                        params: task.params,
                    });
                }
                tasks
            },
        })
    }

    pub fn as_session_data(&self) -> SessionData {
        SessionData {
            id: self.id,
            last_update: self.last_update,
            world: self.world.as_world_data(),
            player: self.player.as_player_data(),
            tasks: self.tasks.iter().map(|v| BotParams { name: v.name.clone(), params: v.params.clone() }).collect(),
        }
    }

    pub fn get_tasks(&self) -> Vec<String> {
        self.tasks.iter().map(|v| v.name.clone()).collect()
    }

    pub fn add_task(&mut self, name: &str, params: &[u8]) -> Result<(), String> {
        self.tasks.push(TaskWithParams {
            name: String::from(name),
            params: Vec::from(params),
            value: make_task(name, params)?,
        });
        Ok(())
    }

    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
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
            for task in self.tasks.iter_mut() {
                task.value.update(&world, &update);
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
            let mut message = None;
            for task in self.tasks.iter_mut() {
                if let Some(v) = task.value.get_next_message(&world) {
                    if !matches!(v, Message::Done { .. }) {
                        message = Some(v);
                        break;
                    }
                    message = Some(v);
                }
            }
            debug!("Next message for session {}: {:?}", self.id, message);
            message
        } else {
            debug!("World is not configured for session {}", self.id);
            None
        }
    }
}

fn make_task(name: &str, params: &[u8]) -> Result<Box<dyn Task>, String> {
    match name {
        "Explorer" => Ok(Box::new(Explorer::new())),
        "ExpWndCloser" => Ok(Box::new(ExpWndCloser::new())),
        "NewCharacter" => {
            match serde_json::from_slice::<NewCharacterParams>(params) {
                Ok(parsed) => Ok(Box::new(NewCharacter::new(parsed))),
                Err(e) => Err(format!("Failed to parse {} task params: {}", name, e)),
            }
        }
        "PathFinder" => Ok(Box::new(PathFinder::new())),
        _ => Err(String::from("Task is not found")),
    }
}

#[derive(Serialize, Deserialize)]
pub struct SessionData {
    id: i64,
    last_update: i64,
    world: WorldData,
    player: PlayerData,
    tasks: Vec<BotParams>,
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
