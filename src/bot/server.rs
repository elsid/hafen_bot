use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use actix_web::{Error, HttpResponse, web};
use actix_web::dev::Server;
use futures::StreamExt;
use rusqlite::Connection;
use serde::Deserialize;

use crate::bot::map_db::MapDb;
use crate::bot::process::{
    add_session_visualization,
    count_updates,
    push_update,
    start_process_session,
    UpdatesQueue,
};
use crate::bot::protocol::{Event, Message, SessionInfo, Update};
use crate::bot::session::{Session, SessionConfig, SessionData};
use crate::bot::sqlite_map_db::SqliteMapDb;

#[derive(Clone)]
struct State {
    updates: Arc<Mutex<HashMap<i64, Arc<UpdatesQueue>>>>,
    messages: Arc<Mutex<HashMap<i64, Arc<Mutex<VecDeque<Message>>>>>>,
    sessions: Arc<Mutex<HashMap<i64, Arc<RwLock<Session>>>>>,
    processors: Arc<Mutex<HashMap<i64, JoinHandle<()>>>>,
    visualizers: Arc<Mutex<HashMap<i64, Arc<Mutex<Vec<JoinHandle<()>>>>>>>,
    map_db: Arc<Mutex<dyn MapDb + Send>>,
    session_config: SessionConfig,
}

pub fn run_server(config: ServerConfig) -> std::io::Result<Server> {
    use actix_web::{middleware, App, HttpServer};

    let state = State {
        updates: Arc::new(Mutex::new(HashMap::new())),
        messages: Arc::new(Mutex::new(HashMap::new())),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        processors: Arc::new(Mutex::new(HashMap::new())),
        visualizers: Arc::new(Mutex::new(HashMap::new())),
        map_db: Arc::new(Mutex::new(SqliteMapDb::new(
            Connection::open(config.map_db_path).unwrap(),
            Duration::from_secs_f64(config.map_cache_ttl),
        ))),
        session_config: config.session,
    };

    Ok(HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource("/ping").route(web::get().to(ping)))
            .service(web::resource("/push").route(web::put().to(push)))
            .service(web::resource("/poll").route(web::get().to(poll)))
            .service(web::resource("/add_bot").route(web::post().to(add_bot)))
            .service(web::resource("/remove_bot").route(web::post().to(remove_bot)))
            .service(web::resource("/clear_bots").route(web::get().to(clear_bots)))
            .service(web::resource("/sessions").route(web::get().to(sessions)))
            .service(web::resource("/set_session").route(web::get().to(set_session)))
            .service(web::resource("/get_session").route(web::get().to(get_session)))
            .service(web::resource("/add_visualization").route(web::get().to(add_visualization)))
            .default_service(web::resource("").to(HttpResponse::NotFound))
    })
        .bind(config.bind_addr)?
        .run())
}

#[derive(Deserialize)]
pub struct ServerConfig {
    bind_addr: String,
    map_db_path: String,
    map_cache_ttl: f64,
    session: SessionConfig,
}

pub fn read_config<T: AsRef<Path>>(path: T) -> std::io::Result<ServerConfig> {
    match serde_yaml::from_reader(std::fs::File::open(path)?) {
        Ok(v) => Ok(v),
        Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to parse config: {}", e))),
    }
}

async fn ping() -> HttpResponse {
    HttpResponse::Ok().json(&Message::Ok)
}

async fn push(state: web::Data<State>, payload: web::Payload) -> Result<HttpResponse, Error> {
    let body = collect(payload).await?;
    let update = match serde_json::from_slice::<Update>(&body) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse update {}: {}", std::str::from_utf8(&body).unwrap(), e);
            return Ok(HttpResponse::Ok().json(&Message::Error { message: String::from("Failed to parse update") }));
        }
    };
    let session_id = update.session;
    let new_session = match &update.event {
        Event::SessionData { value: Some(value) } => {
            match serde_json::from_str(&value) {
                Ok(v) => {
                    match Session::from_session_data(v, state.map_db.clone(), &state.session_config) {
                        Ok(v) => {
                            if let Some(session) = state.sessions.lock().unwrap().get(&session_id).map(Arc::clone) {
                                info!("Set session data {}", session_id);
                                *session.write().unwrap() = v;
                                return Ok(HttpResponse::Ok().json(&Message::Ok));
                            } else {
                                info!("Use session data {}", session_id);
                                v
                            }
                        }
                        Err(e) => {
                            error!("Failed to create session from data: {}", e);
                            return Ok(HttpResponse::Ok().json(Message::Error { message: String::from("Failed to create session from data") }));
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to parse session data {}: {}", value, e);
                    return Ok(HttpResponse::Ok().json(Message::Error { message: String::from("Failed to parse session data") }));
                }
            }
        }
        _ => if let Some(updates) = state.updates.lock().unwrap().get(&session_id).map(Arc::clone) {
            push_update(&updates, update);
            return Ok(HttpResponse::Ok().json(&Message::Ok));
        } else {
            info!("Create new session {}", session_id);
            Session::new(session_id, state.map_db.clone(), &state.session_config)
        },
    };
    let session = state.sessions.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| Arc::new(RwLock::new(new_session)))
        .clone();
    let updates = state.updates.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| Arc::new(UpdatesQueue::new()))
        .clone();
    let messages = state.messages.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| Arc::new(Mutex::new(VecDeque::new())))
        .clone();
    let visualizers = state.visualizers.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
        .clone();
    if !matches!(update.event, Event::SessionData { .. }) {
        push_update(&updates, update);
    }
    state.processors.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| {
            start_process_session(session_id, session, updates, messages, visualizers,
                                  state.map_db.clone())
        });
    Ok(HttpResponse::Ok().json(&Message::Ok))
}

#[derive(Deserialize)]
struct Poll {
    session: i64,
}

async fn poll(state: web::Data<State>, query: web::Query<Poll>) -> HttpResponse {
    HttpResponse::Ok().json(
        state.messages.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|messages| messages.lock().unwrap().pop_front().unwrap_or(Message::Ok))
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}

#[derive(Deserialize)]
struct AddBot {
    session: i64,
    bot_name: String,
}

async fn add_bot(state: web::Data<State>, query: web::Query<AddBot>, payload: web::Payload) -> Result<HttpResponse, Error> {
    let body = collect(payload).await?;
    Ok(HttpResponse::Ok().json(
        state.sessions.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|session| {
                match session.write().unwrap().add_bot(query.bot_name.as_str(), &body) {
                    Ok(_) => Message::Ok,
                    Err(e) => Message::Error { message: e },
                }
            })
            .unwrap_or_else(|| {
                let session_id = query.session;
                let new_session = Session::new(session_id, state.map_db.clone(), &state.session_config);
                let session = state.sessions.lock().unwrap()
                    .entry(session_id)
                    .or_insert_with(|| Arc::new(RwLock::new(new_session)))
                    .clone();
                let updates = state.updates.lock().unwrap()
                    .entry(session_id)
                    .or_insert_with(|| Arc::new(UpdatesQueue::new()))
                    .clone();
                let messages = state.messages.lock().unwrap()
                    .entry(session_id)
                    .or_insert_with(|| Arc::new(Mutex::new(VecDeque::new())))
                    .clone();
                let visualizers = state.visualizers.lock().unwrap()
                    .entry(session_id)
                    .or_insert_with(|| Arc::new(Mutex::new(Vec::new())))
                    .clone();
                state.processors.lock().unwrap()
                    .entry(session_id)
                    .or_insert_with(|| {
                        start_process_session(session_id, session, updates, messages, visualizers,
                                              state.map_db.clone())
                    });
                Message::Ok
            })
    ))
}

#[derive(Deserialize)]
struct RemoveBot {
    session: i64,
    bot_id: i64,
}

async fn remove_bot(state: web::Data<State>, query: web::Query<RemoveBot>) -> HttpResponse {
    HttpResponse::Ok().json(
        state.sessions.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|session| {
                session.write().unwrap().remove_bot(query.bot_id);
                Message::Ok
            })
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}

#[derive(Deserialize)]
struct ClearBots {
    session: i64,
}

async fn clear_bots(state: web::Data<State>, query: web::Query<ClearBots>) -> HttpResponse {
    HttpResponse::Ok().json(
        state.sessions.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|session| {
                session.write().unwrap().clear_bots();
                Message::Ok
            })
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}

async fn sessions(state: web::Data<State>) -> HttpResponse {
    let session_ids = state.sessions.lock().unwrap().keys().cloned().collect::<Vec<_>>();
    HttpResponse::Ok().json(&Message::Sessions {
        value: session_ids.iter()
            .map(|session_id| SessionInfo {
                id: *session_id,
                bots: state.sessions.lock().unwrap()
                    .get(session_id)
                    .map(Arc::clone)
                    .map(|session| session.read().unwrap().get_bots())
                    .unwrap_or_else(Vec::new),
                updates: state.updates.lock().unwrap()
                    .get(session_id)
                    .map(Arc::clone)
                    .map(|updates| count_updates(&updates))
                    .unwrap_or(0),
                messages: state.messages.lock().unwrap()
                    .get(session_id)
                    .map(Arc::clone)
                    .map(|messages| messages.lock().unwrap().len())
                    .unwrap_or(0),
            })
            .collect()
    })
}

#[derive(Deserialize)]
struct SetSession {
    session: i64,
}

async fn set_session(state: web::Data<State>, query: web::Query<SetSession>, payload: web::Payload) -> Result<HttpResponse, Error> {
    let body = collect(payload).await?;
    let session_data = match serde_json::from_slice::<SessionData>(&body) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse session data {}: {}", std::str::from_utf8(&body).unwrap(), e);
            return Ok(HttpResponse::Ok().json(&Message::Error { message: String::from("Failed to parse session data") }));
        }
    };
    let session = match Session::from_session_data(session_data, state.map_db.clone(), &state.session_config) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to create session from data: {}", e);
            return Ok(HttpResponse::Ok().json(&Message::Error { message: String::from("Failed to create session from data") }));
        }
    };
    state.sessions.lock().unwrap().insert(query.session, Arc::new(RwLock::new(session)));
    Ok(HttpResponse::Ok().json(Message::Ok))
}

#[derive(Deserialize)]
struct GetSession {
    session: i64,
}

async fn get_session(state: web::Data<State>, query: web::Query<GetSession>) -> HttpResponse {
    HttpResponse::Ok().json(
        state.sessions.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|session| Message::Session {
                value: session.read().unwrap().as_session_data(),
            })
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}

async fn collect(mut payload: web::Payload) -> Result<web::BytesMut, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Deserialize)]
struct AddVisualization {
    session: i64,
}

async fn add_visualization(state: web::Data<State>, query: web::Query<AddVisualization>) -> HttpResponse {
    let session_id = query.session;
    HttpResponse::Ok().json(
        &state.sessions.lock().unwrap()
            .get(&session_id)
            .map(Arc::clone)
            .and_then(|session| {
                state.updates.lock().unwrap().get(&session_id)
                    .map(Arc::clone)
                    .map(|v| (session, v))
            })
            .and_then(|(session, updates)| {
                state.messages.lock().unwrap().get(&session_id)
                    .map(Arc::clone)
                    .map(|v| (session, updates, v))
            })
            .and_then(|(session, updates, messages)| {
                state.visualizers.lock().unwrap().get(&session_id)
                    .map(Arc::clone)
                    .map(|v| (session, updates, messages, v))
            })
            .map(|(session, updates, messages, visualizers)| {
                add_session_visualization(session_id, &session, &updates, &messages, &visualizers,
                                          state.map_db.clone());
                Message::Ok
            })
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}
