use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{JoinHandle, spawn};

use crate::bot::protocol::{Event, Message, Update};
use crate::bot::session::Session;

pub fn start_process_session(session_id: i64, session: Arc<RwLock<Session>>, updates: Arc<UpdatesQueue>,
                             messages: Arc<Mutex<VecDeque<Message>>>) -> JoinHandle<()> {
    spawn(move || process_session(session_id, session, updates, messages))
}

fn process_session(session_id: i64, session: Arc<RwLock<Session>>, updates: Arc<UpdatesQueue>,
                   messages: Arc<Mutex<VecDeque<Message>>>) {
    loop {
        let update = poll_update(&updates);
        if let &Event::Close = &update.event {
            info!("Session {} is closed", session_id);
            break;
        }
        let session_data = {
            let mut locked_session = session.write().unwrap();
            let session_data = if locked_session.update(update) {
                info!("Session {} is updated", session_id);
                Some(locked_session.as_session_data())
            } else {
                None
            };
            session_data
        };
        if let Some(next_message) = session.read().unwrap().get_next_message() {
            let mut locked_messages = messages.lock().unwrap();
            if locked_messages.is_empty() || *locked_messages.back().unwrap() != next_message {
                info!("Add next message for session {}: {:?}", session_id, next_message);
                locked_messages.push_back(next_message);
            }
        }
        let session_path = format!("sessions/{}.json", session_id);
        let new_path = format!("sessions/{}.new.json", session_id);
        if let Some(data) = session_data {
            match data.write_to_file(new_path.as_str()) {
                Ok(_) => match std::fs::rename(new_path, session_path) {
                    Ok(_) => info!("Session is saved: {}", session_id),
                    Err(e) => error!("Failed to rename new session file {}: {}", session_id, e),
                },
                Err(e) => error!("Failed to write session {}: {}", session_id, e),
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
    values.lock().unwrap().iter().count()
}
