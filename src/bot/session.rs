use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};

use crate::bot::map_db::MapDb;
use crate::bot::player::{Player, PlayerData};
use crate::bot::protocol::{Event, Message, Update, Value};
use crate::bot::scene::Scene;
use crate::bot::tasks::exp_wnd_closer::ExpWndCloser;
use crate::bot::tasks::explorer::{Explorer, ExplorerConfig};
use crate::bot::tasks::new_character::{NewCharacter, NewCharacterParams};
use crate::bot::tasks::path_finder::{PathFinder, PathFinderConfig};
use crate::bot::tasks::task::Task;
use crate::bot::world::{PlayerWorld, World, WorldConfig, WorldData};

#[derive(Clone, Deserialize)]
pub struct SessionConfig {
    world: WorldConfig,
    tasks: TaskConfigs,
}

#[derive(Clone, Deserialize)]
pub struct TaskConfigs {
    path_finder: PathFinderConfig,
    explorer: ExplorerConfig,
}

pub struct Session {
    id: i64,
    last_update: i64,
    world: World,
    player: Player,
    task_id_counter: i64,
    tasks: Arc<RwLock<Vec<Arc<RwLock<TaskWithParams>>>>>,
    scene: Scene,
    messages: Arc<Mutex<VecDeque<Message>>>,
    task_configs: TaskConfigs,
}

struct TaskWithParams {
    id: i64,
    name: String,
    params: Vec<u8>,
    value: Arc<Mutex<dyn Task>>,
}

impl Session {
    pub fn new(id: i64, map_db: Arc<Mutex<dyn MapDb + Send>>, config: &SessionConfig) -> Self {
        Self {
            id,
            last_update: 0,
            world: World::new(config.world.clone(), map_db),
            player: Player::default(),
            task_id_counter: 0,
            tasks: Arc::new(RwLock::new(Vec::new())),
            scene: Scene::new(),
            messages: Arc::new(Mutex::new(VecDeque::new())),
            task_configs: config.tasks.clone(),
        }
    }

    pub fn from_session_data(session_data: SessionData, map_db: Arc<Mutex<dyn MapDb + Send>>,
                             config: &SessionConfig) -> Result<Self, String> {
        Ok(Self {
            id: session_data.id,
            last_update: 0,
            player: Player::from_player_data(session_data.player),
            task_id_counter: session_data.task_id_counter,
            tasks: {
                let mut tasks = Vec::new();
                for task in session_data.tasks.into_iter() {
                    tasks.push(Arc::new(RwLock::new(TaskWithParams {
                        id: task.id,
                        value: make_task(task.name.as_str(), task.params.as_slice(), &config.tasks)?,
                        name: task.name,
                        params: task.params,
                    })));
                }
                Arc::new(RwLock::new(tasks))
            },
            world: World::from_world_data(session_data.world, config.world.clone(), map_db),
            scene: Scene::new(),
            messages: Arc::new(Mutex::new(VecDeque::new())),
            task_configs: config.tasks.clone(),
        })
    }

    pub fn as_session_data(&self) -> SessionData {
        SessionData {
            id: self.id,
            last_update: self.last_update,
            world: self.world.as_world_data(),
            player: self.player.as_player_data(),
            task_id_counter: self.task_id_counter,
            tasks: self.tasks.read().unwrap().iter()
                .map(Arc::clone)
                .map(|v| {
                    let locked = v.read().unwrap();
                    TaskParams {
                        id: locked.id,
                        name: locked.name.clone(),
                        params: locked.params.clone(),
                    }
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
        self.task_id_counter += 1;
        let id = self.task_id_counter;
        self.tasks.write().unwrap().push(Arc::new(RwLock::new(TaskWithParams {
            id,
            name: String::from(name),
            params: Vec::from(params),
            value: make_task(name, params, &self.task_configs)?,
        })));
        if let Some(game_ui_id) = self.player.game_ui_id() {
            self.messages.lock().unwrap().push_back(Message::UIMessage {
                id: game_ui_id,
                kind: String::from("add-task"),
                arguments: vec![
                    Value::from(id),
                    Value::from(String::from(name)),
                    Value::from(Vec::from(params)),
                ],
            });
        }
        Ok(())
    }

    pub fn remove_task(&mut self, id: i64) {
        let mut removed = false;
        self.tasks.write().unwrap().retain(|task| {
            if task.read().unwrap().id == id {
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
                    kind: String::from("remove-task"),
                    arguments: vec![Value::from(id)],
                });
            }
        }
    }

    pub fn clear_tasks(&self) {
        let mut locked = self.tasks.write().unwrap();
        if let Some(world) = self.world.for_player(&self.player) {
            for task in locked.iter() {
                self.messages.lock().unwrap().push_back(Message::UIMessage {
                    id: world.game_ui_id(),
                    kind: String::from("remove-task"),
                    arguments: vec![Value::from(task.read().unwrap().id)],
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
            Event::TaskAdd { name, params } => {
                match self.add_task(name, params) {
                    Ok(_) => (),
                    Err(e) => error!("Failed to add task: {:?}", e),
                }
            }
            Event::TaskRemove { id } => {
                self.remove_task(*id);
            }
            _ => (),
        }
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
        if let Some(message) = self.messages.lock().unwrap().pop_front() {
            return Some(message);
        }
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

fn make_task(name: &str, params: &[u8], task_configs: &TaskConfigs) -> Result<Arc<Mutex<dyn Task>>, String> {
    match name {
        "Explorer" => Ok(Arc::new(Mutex::new(Explorer::new(task_configs.explorer.clone())))),
        "ExpWndCloser" => Ok(Arc::new(Mutex::new(ExpWndCloser::new()))),
        "NewCharacter" => {
            match serde_json::from_slice::<NewCharacterParams>(params) {
                Ok(parsed) => Ok(Arc::new(Mutex::new(NewCharacter::new(parsed)))),
                Err(e) => Err(format!("Failed to parse {} bot params: {}", name, e)),
            }
        }
        "PathFinder" => Ok(Arc::new(Mutex::new(PathFinder::new(task_configs.path_finder.clone())))),
        _ => Err(String::from("Task is not found")),
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SessionData {
    id: i64,
    last_update: i64,
    world: WorldData,
    player: PlayerData,
    task_id_counter: i64,
    tasks: Vec<TaskParams>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct TaskParams {
    id: i64,
    name: String,
    params: Vec<u8>,
}
