use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::thread::{JoinHandle, spawn};

use serde::Deserialize;

use crate::bot::map_db::MapDb;
use crate::bot::protocol::{Event, Message, Update};
use crate::bot::session::Session;
use crate::bot::visualization::start_visualize_session;

#[derive(Clone, Deserialize)]
pub struct ProcessConfig {
    pub sessions_path: String,
    pub write_updates_log: bool,
}

pub fn start_process_session(session_id: i64, session: Arc<RwLock<Session>>, updates: Arc<UpdatesQueue>,
                             messages: Arc<Mutex<VecDeque<Message>>>,
                             visualizers: Arc<Mutex<Vec<JoinHandle<()>>>>, map_db: Arc<Mutex<dyn MapDb + Send>>,
                             cancel: Arc<AtomicBool>, config: ProcessConfig) -> JoinHandle<()> {
    spawn(move || process_session(session_id, session, updates, messages, visualizers, map_db, cancel, config))
}

fn process_session(session_id: i64, session: Arc<RwLock<Session>>, updates: Arc<UpdatesQueue>,
                   messages: Arc<Mutex<VecDeque<Message>>>, visualizers: Arc<Mutex<Vec<JoinHandle<()>>>>,
                   map_db: Arc<Mutex<dyn MapDb + Send>>, cancel: Arc<AtomicBool>, config: ProcessConfig) {
    info!("Start process session {}", session_id);
    messages.lock().unwrap().push_back(Message::GetSessionData);
    let (updates_sender, updates_writer) = if config.write_updates_log {
        let (sender, receiver) = channel();
        let sessions_path = config.sessions_path.clone();
        (Some(sender), Some(spawn(move || write_updates(session_id, receiver, sessions_path))))
    } else {
        (None, None)
    };
    loop {
        let update = poll_update(&updates);
        if let Some(sender) = updates_sender.as_ref() {
            sender.send(Some(update.clone())).unwrap();
        }
        match &update.event {
            Event::Close => break,
            Event::VisualizationAdd => {
                add_session_visualization(session_id, &session, &updates, &messages, &visualizers, map_db.clone());
            }
            Event::GetSessionData => {
                let session_data = session.read().unwrap().as_session_data();
                let value = serde_json::to_string(&session_data).unwrap();
                messages.lock().unwrap().push_back(Message::SessionData { value });
            }
            _ => (),
        }
        if session.write().unwrap().update(update) {
            debug!("Session {} is updated", session_id);
        }
        while let Some(message) = session.read().unwrap().get_existing_message() {
            let mut locked_messages = messages.lock().unwrap();
            if locked_messages.is_empty() || *locked_messages.back().unwrap() != message {
                debug!("Add next message for session {}: {:?}", session_id, message);
                locked_messages.push_back(message);
            }
        }
        if let Some(message) = session.read().unwrap().get_next_message() {
            let mut locked_messages = messages.lock().unwrap();
            if locked_messages.is_empty() || *locked_messages.back().unwrap() != message {
                debug!("Add next message for session {}: {:?}", session_id, message);
                locked_messages.push_back(message);
            }
        }
        cancel.store(false, Ordering::Relaxed);
    }
    if let Some(sender) = updates_sender.as_ref() {
        sender.send(None).unwrap();
    }
    if let Some(writer) = updates_writer {
        writer.join().unwrap();
    }
    info!("Stop process session {}", session_id);
}

fn write_updates(session_id: i64, receiver: Receiver<Option<Update>>, path: String) {
    match std::fs::create_dir_all(&path) {
        Ok(_) => (),
        Err(e) => {
            error!("Failed to create dir {}: {}", path, e);
            return;
        }
    }
    let mut file = match OpenOptions::new().create(true).append(true).open(format!("{}/{}.json", path, session_id)) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed open updates log for session {}: {}", session_id, e);
            return;
        }
    };
    while let Some(update) = receiver.recv().unwrap() {
        match file.write(&serde_json::to_vec(&update).unwrap()) {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to write update for session {}: {}", session_id, e);
                break;
            }
        }
        match file.write(b"\n") {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to write update for session {}: {}", session_id, e);
                break;
            }
        }
    }
}

pub struct UpdatesQueue {
    has_value: Condvar,
    values: Mutex<VecDeque<Update>>,
}

impl UpdatesQueue {
    pub fn new() -> Self {
        Self {
            has_value: Condvar::new(),
            values: Mutex::new(VecDeque::new()),
        }
    }
}

pub fn push_update(updates: &Arc<UpdatesQueue>, update: Update) {
    let UpdatesQueue { has_value, values } = &**updates;
    values.lock().unwrap().push_back(update);
    has_value.notify_one();
}

fn poll_update(updates: &Arc<UpdatesQueue>) -> Update {
    let UpdatesQueue { has_value, values } = &**updates;
    let mut locked_values = has_value
        .wait_while(values.lock().unwrap(), |values| values.is_empty())
        .unwrap();
    locked_values.pop_front().unwrap()
}

pub fn count_updates(updates: &Arc<UpdatesQueue>) -> usize {
    let UpdatesQueue { has_value: _, values } = &**updates;
    values.lock().unwrap().len()
}

pub fn add_session_visualization(session_id: i64, session: &Arc<RwLock<Session>>, updates: &Arc<UpdatesQueue>,
                                 messages: &Arc<Mutex<VecDeque<Message>>>,
                                 visualizers: &Arc<Mutex<Vec<JoinHandle<()>>>>,
                                 map_db: Arc<Mutex<dyn MapDb + Send>>) {
    let scene = session.read().unwrap().scene().clone();
    visualizers.lock().unwrap()
        .push(start_visualize_session(session_id, session.clone(), scene, updates.clone(), messages.clone(), map_db));
}
