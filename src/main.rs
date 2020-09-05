#[macro_use]
extern crate hexf;
#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use actix_web::{Error, HttpResponse, web};
use futures::StreamExt;
use serde::Deserialize;

use crate::bot::{Message, Session, SessionData, SessionInfo, Update};

mod bot;

#[derive(Clone)]
struct State {
    sessions: Arc<Mutex<HashMap<i64, Arc<Mutex<Session>>>>>,
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    use actix_web::{middleware, App, HttpServer};

    env_logger::init();

    let state = State {
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    HttpServer::new(move || {
        App::new()
            .data(state.clone())
            .wrap(middleware::Logger::default()) // <- limit size of the payload (global configuration)
            .service(web::resource("/ping").route(web::get().to(ping)))
            .service(web::resource("/push").route(web::put().to(push)))
            .service(web::resource("/poll").route(web::get().to(poll)))
            .service(web::resource("/add_bot").route(web::post().to(add_bot)))
            .service(web::resource("/clear_bots").route(web::get().to(clear_bots)))
            .service(web::resource("/sessions").route(web::get().to(sessions)))
            .default_service(web::resource("").to(HttpResponse::NotFound))
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
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
    let session_path = format!("sessions/{}.json", update.session);
    let session_id = update.session;
    if let Some(session) = state.sessions.lock().unwrap().get(&session_id).map(Arc::clone) {
        update_session(session_id, session, update);
        return Ok(HttpResponse::Ok().json(&Message::Ok));
    }
    let new_session = match SessionData::read_from_file(session_path.as_str()) {
        Ok(v) => {
            match Session::from_session_data(v) {
                Ok(v) => {
                    info!("Use saved session: {}", session_id);
                    v
                }
                Err(e) => {
                    error!("Failed to use saved session {}: {}", session_id, e);
                    info!("Create new session: {}", session_id);
                    Session::new(session_id)
                }
            }
        }
        Err(e) => {
            error!("Failed to read session {}: {}", session_id, e);
            info!("Create new session: {}", session_id);
            Session::new(session_id)
        }
    };
    let session = state.sessions.lock().unwrap()
        .entry(session_id)
        .or_insert_with(|| Arc::new(Mutex::new(new_session)))
        .clone();
    update_session(session_id, session, update);
    Ok(HttpResponse::Ok().json(&Message::Ok))
}

#[derive(Deserialize)]
struct Poll {
    session: i64,
}

async fn poll(state: web::Data<State>, query: web::Query<Poll>) -> HttpResponse {
    HttpResponse::Ok().json(
        state.sessions.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|session| session.lock().unwrap().get_next_message().unwrap_or(Message::Ok))
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}

#[derive(Deserialize)]
struct RunBot {
    session: i64,
    bot_name: String,
}

async fn add_bot(state: web::Data<State>, query: web::Query<RunBot>, payload: web::Payload) -> Result<HttpResponse, Error> {
    let body = collect(payload).await?;
    Ok(HttpResponse::Ok().json(
        state.sessions.lock().unwrap()
            .get(&query.session)
            .map(Arc::clone)
            .map(|session| {
                match session.lock().unwrap().add_bot(query.bot_name.as_str(), &body) {
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
                session.lock().unwrap().clear_bots();
                Message::Ok
            })
            .unwrap_or_else(|| Message::Error { message: String::from("Session is not found") })
    )
}

async fn sessions(state: web::Data<State>) -> HttpResponse {
    HttpResponse::Ok().json(&Message::Sessions {
        value: state.sessions.lock().unwrap().iter()
            .map(|(id, session)| SessionInfo {
                id: *id,
                bots: session.lock().unwrap().get_bots(),
            })
            .collect()
    })
}

async fn collect(mut payload: web::Payload) -> Result<web::BytesMut, Error> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn update_session(session_id: i64, session: Arc<Mutex<Session>>, update: Update) {
    let session_data = {
        let mut locked_session = session.lock().unwrap();
        if locked_session.update(update) {
            Some(locked_session.as_session_data())
        } else {
            None
        }
    };
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
