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
    info!("Start process session {}", session_id);
    messages.lock().unwrap().push_back(Message::GetSessionData);
    loop {
        let update = poll_update(&updates);
        if let &Event::Close = &update.event {
            break;
        }
        if session.write().unwrap().update(update) {
            debug!("Session {} is updated", session_id);
        }
        if let Some(next_message) = session.read().unwrap().get_next_message() {
            let mut locked_messages = messages.lock().unwrap();
            if locked_messages.is_empty() || *locked_messages.back().unwrap() != next_message {
                debug!("Add next message for session {}: {:?}", session_id, next_message);
                locked_messages.push_back(next_message);
            }
        }
    }
    info!("Stop process session {}", session_id);
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
