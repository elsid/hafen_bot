use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;

use actix_web::{Error, HttpResponse, web};
use actix_web::dev::Server;
use futures::StreamExt;
use serde::Deserialize;

use crate::bot::process::{count_updates, push_update, start_process_session, UpdatesQueue};
use crate::bot::protocol::{Message, SessionInfo, Update};
use crate::bot::session::{Session, SessionData};

#[derive(Clone)]
struct State {
    updates: Arc<Mutex<HashMap<i64, Arc<UpdatesQueue>>>>,
    messages: Arc<Mutex<HashMap<i64, Arc<Mutex<VecDeque<Message>>>>>>,
    sessions: Arc<Mutex<HashMap<i64, Arc<RwLock<Session>>>>>,
    processors: Arc<Mutex<HashMap<i64, JoinHandle<()>>>>,
}

pub fn run_server() -> std::io::Result<Server> {
    use actix_web::{middleware, App, HttpServer};

    let state = State {
        updates: Arc::new(Mutex::new(HashMap::new())),
        messages: Arc::new(Mutex::new(HashMap::new())),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        processors: Arc::new(Mutex::new(HashMap::new())),
    };

    Ok(HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default())
            .service(web::resource("/ping").route(web::get().to(ping)))
            .service(web::resource("/push").route(web::put().to(push)))
            .service(web::resource("/poll").route(web::get().to(poll)))
            .service(web::resource("/add_bot").route(web::post().to(add_bot)))
            .service(web::resource("/clear_bots").route(web::get().to(clear_bots)))
            .service(web::resource("/sessions").route(web::get().to(sessions)))
            .service(web::resource("/set_session").route(web::get().to(set_session)))
            .service(web::resource("/get_session").route(web::get().to(get_session)))
            .default_service(web::resource("").to(HttpResponse::NotFound))
    })
        .bind("127.0.0.1:8080")?
        .run())
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
    match std::fs::create_dir_all("sessions") {
        Ok(_) => (),
        Err(e) => {
            error!("Failed create sessions directory: {}", e);
            return Ok(HttpResponse::Ok().json(&Message::Error { message: String::from("Failed create sessions directory") }));
        }
    }
    let session_id = update.session;
    if let Some(updates) = state.updates.lock().unwrap().get(&session_id).map(Arc::clone) {
        push_update(&updates, update);
        return Ok(HttpResponse::Ok().json(&Message::Ok));
    }
    info!("Create new session: {}", session_id);
    let new_session = Session::new(session_id);
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
    push_update(&updates, update);
    state.processors.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| start_process_session(session_id, session, updates, messages));
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
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    ))
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
    let session = match Session::from_session_data(session_data) {
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
            .map(|session| Message::SessionData {
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