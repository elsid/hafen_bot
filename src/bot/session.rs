use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::bot::player::{Player, PlayerData};
use crate::bot::protocol::{Message, Update};
use crate::bot::tasks::exp_wnd_closer::ExpWndCloser;
use crate::bot::tasks::explorer::Explorer;
use crate::bot::tasks::new_character::{NewCharacter, NewCharacterParams};
use crate::bot::tasks::path_finder::PathFinder;
use crate::bot::tasks::task::Task;
use crate::bot::world::{PlayerWorld, World, WorldData};
use crate::bot::scene::Scene;

pub struct Session {
    id: i64,
    last_update: i64,
    world: World,
    player: Player,
    tasks: Arc<RwLock<Vec<Arc<RwLock<TaskWithParams>>>>>,
    scene: Scene,
}

struct TaskWithParams {
    name: String,
    params: Vec<u8>,
    value: Arc<Mutex<dyn Task>>,
}

impl Session {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            last_update: 0,
            world: World::new(),
            player: Player::default(),
            tasks: Arc::new(RwLock::new(Vec::new())),
            scene: Scene::new(),
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
                    tasks.push(Arc::new(RwLock::new(TaskWithParams {
                        value: make_task(task.name.as_str(), task.params.as_slice())?,
                        name: task.name,
                        params: task.params,
                    })));
                }
                Arc::new(RwLock::new(tasks))
            },
            scene: Scene::new(),
        })
    }

    pub fn as_session_data(&self) -> SessionData {
        SessionData {
            id: self.id,
            last_update: self.last_update,
            world: self.world.as_world_data(),
            player: self.player.as_player_data(),
            tasks: self.tasks.read().unwrap().iter()
                .map(Arc::clone)
                .map(|v| {
                    let locked = v.read().unwrap();
                    BotParams { name: locked.name.clone(), params: locked.params.clone() }
                })
                .collect(),
        }
    }

    pub fn get_tasks(&self) -> Vec<String> {
        self.tasks.read().unwrap().iter()
            .map(|v| v.read().unwrap().name.clone())
            .collect()
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn add_task(&mut self, name: &str, params: &[u8]) -> Result<(), String> {
        self.tasks.write().unwrap().push(Arc::new(RwLock::new(TaskWithParams {
            name: String::from(name),
            params: Vec::from(params),
            value: make_task(name, params)?,
        })));
        Ok(())
    }

    pub fn clear_tasks(&self) {
        self.tasks.write().unwrap().clear();
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
            for task in self.tasks.read().unwrap().iter().map(Arc::clone) {
                task.read().unwrap().value.lock().unwrap().update(&world, &update);
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
        if let Some(world) = self.world.for_player(&self.player) {
            let mut message = None;
            for task in self.tasks.read().unwrap().iter().map(Arc::clone) {
                if let Some(v) = task.read().unwrap().value.lock().unwrap().get_next_message(&world, &self.scene) {
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

    pub fn get_player_world(&self) -> Option<PlayerWorld> {
        self.world.for_player(&self.player)
    }
}

fn make_task(name: &str, params: &[u8]) -> Result<Arc<Mutex<dyn Task>>, String> {
    match name {
        "Explorer" => Ok(Arc::new(Mutex::new(Explorer::new()))),
        "ExpWndCloser" => Ok(Arc::new(Mutex::new(ExpWndCloser::new()))),
        "NewCharacter" => {
            match serde_json::from_slice::<NewCharacterParams>(params) {
                Ok(parsed) => Ok(Arc::new(Mutex::new(NewCharacter::new(parsed)))),
                Err(e) => Err(format!("Failed to parse {} bot params: {}", name, e)),
            }
        }
        "PathFinder" => Ok(Arc::new(Mutex::new(PathFinder::new()))),
        _ => Err(String::from("Task is not found")),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SessionData {
    id: i64,
    last_update: i64,
    world: WorldData,
    player: PlayerData,
    tasks: Vec<BotParams>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct BotParams {
    name: String,
    params: Vec<u8>,
}
