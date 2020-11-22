#[macro_use]
extern crate hexf;
extern crate portpicker;
extern crate reqwest;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use futures::Future;
use portpicker::{pick_unused_port, Port};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use hafen_bot::bot::{run_server, ServerConfig};

#[actix_rt::test]
async fn ping() {
    with_bot_service(|bot_service| async move {
        assert_eq!(
            bot_service.ping().await, r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn no_sessions_by_default() {
    with_bot_service(|bot_service| async move {
        assert_eq!(
            bot_service.sessions().await, r#"{"type":"Sessions","value":[]}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn single_update_should_create_new_session() {
    with_bot_service(|bot_service| async move {
        for update in read_updates("tests/input/new_session.json").into_iter() {
            assert_eq!(
                bot_service.push(&update).await, r#"{"type":"Ok"}"#,
                "BotService port={}", bot_service.port
            );
        }
        assert!(
            parse_session(&bot_service.sessions().await).value.iter().find(|v| v.id == 1602331785).is_some(),
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn poll_should_fail_for_absent_session() {
    with_bot_service(|bot_service| async move {
        assert_eq!(
            bot_service.poll(1).await, r#"{"type":"Error","message":"Session is not found"}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn first_poll_should_return_get_session_data() {
    with_bot_service(|bot_service| async move {
        let mut session_id = 0;
        for update in read_updates("tests/input/new_session.json").into_iter() {
            assert_eq!(
                bot_service.push(&update).await, r#"{"type":"Ok"}"#,
                "BotService port={}", bot_service.port
            );
            session_id = update["session"].as_i64().unwrap();
        }
        assert_eq!(
            bot_service.poll(session_id).await, r#"{"type":"GetSessionData"}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn poll_should_return_session_data_after_request_once() {
    with_bot_service(|bot_service| async move {
        let mut session_id = 0;
        let mut number = 0;
        for update in read_updates("tests/input/init_session_start.json").into_iter() {
            assert_eq!(bot_service.push(&update).await, r#"{"type":"Ok"}"#);
            session_id = update["session"].as_i64().unwrap();
            number = update["number"].as_i64().unwrap();
        }
        assert_eq!(
            bot_service.poll(session_id).await, r#"{"type":"GetSessionData"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 1,
                "event": {"type": "GetSessionData"},
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        wait_updates(&bot_service, session_id).await;
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            parse_json(&bot_service.poll(session_id).await)["type"].as_str(), Some("SessionData"),
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.poll(session_id).await, r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn new_character() {
    with_bot_service(|bot_service| async move {
        let mut session_id = 0;
        let mut number = 0;
        for update in read_updates("tests/input/init_session_start.json").iter() {
            assert_eq!(
                bot_service.push(&update).await, r#"{"type":"Ok"}"#,
                "BotService port={}", bot_service.port
            );
            session_id = update["session"].as_i64().unwrap();
            number = update["number"].as_i64().unwrap();
        }
        assert_eq!(
            bot_service.poll(session_id).await, r#"{"type":"GetSessionData"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 1,
                "event": {
                    "type": "TaskAdd",
                    "name": "NewCharacter",
                    "params": serde_json::to_vec(&json!({"character_name": "Noexcept"})).unwrap(),
                },
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        wait_updates(&bot_service, session_id).await;
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"UIMessage","id":6,"kind":"add-task","arguments":[{"type":"Long","value":1},{"type":"Str","value":"NewCharacter"},{"type":"Bytes","value":[123,34,99,104,97,114,97,99,116,101,114,95,110,97,109,101,34,58,34,78,111,101,120,99,101,112,116,34,125]}]}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"WidgetMessage","sender":7,"kind":"click","arguments":[{"type":"Coord","value":{"x":0,"y":0}},{"type":"Coord","value":{"x":-924781,"y":-941823}},{"type":"Int","value":1},{"type":"Int","value":0}]}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

#[actix_rt::test]
async fn path_finder() {
    with_bot_service(|bot_service| async move {
        let mut session_id = 0;
        let mut number = 0;
        for update in read_updates("tests/input/init_session_lake.json").iter() {
            assert_eq!(
                bot_service.push(&update).await, r#"{"type":"Ok"}"#,
                "BotService port={}", bot_service.port
            );
            session_id = update["session"].as_i64().unwrap();
            number = update["number"].as_i64().unwrap();
        }
        assert_eq!(
            bot_service.poll(session_id).await, r#"{"type":"GetSessionData"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 1,
                "event": {
                    "type": "TaskAdd",
                    "name": "PathFinder",
                    "params": [],
                },
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        wait_updates(&bot_service, session_id).await;
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"UIMessage","id":6,"kind":"add-task","arguments":[{"type":"Long","value":1},{"type":"Str","value":"PathFinder"},{"type":"Bytes","value":[]}]}"#,
            "BotService port={}", bot_service.port
        );
        let dst_x = -9790.0;
        let dst_y = -10747.0;
        assert_eq!(
            bot_service.push(&make_map_click(session_id, number + 2, (dst_x / RESOLUTION).floor() as i64, (dst_y / RESOLUTION).floor() as i64)).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        for _ in 0..10usize {
            wait_for_message(&bot_service, session_id).await;
            let message = bot_service.poll(session_id).await;
            if message == r#"{"type":"Done","bot":"PathFinder"}"# {
                break;
            }
            let parsed = parse_json(&message);
            let coord = get_map_click_coord(&parsed);
            let x = coord.x as f64 * RESOLUTION;
            let y = coord.y as f64 * RESOLUTION;
            number += 1;
            assert_eq!(
                bot_service.push(&make_gob_move(session_id, number + 3, 1692553963, x, y)).await,
                r#"{"type":"Ok"}"#,
                "BotService port={}", bot_service.port
            );
        }
    }).await;
}

#[actix_rt::test]
async fn drinker() {
    with_bot_service(|bot_service| async move {
        let mut session_id = 0;
        let mut number = 0;
        for update in read_updates("tests/input/init_session_lake.json").iter() {
            assert_eq!(
                bot_service.push(&update).await, r#"{"type":"Ok"}"#,
                "BotService port={}", bot_service.port
            );
            session_id = update["session"].as_i64().unwrap();
            number = update["number"].as_i64().unwrap();
        }
        assert_eq!(
            bot_service.poll(session_id).await, r#"{"type":"GetSessionData"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&make_set_meter(session_id, number + 1, 33, 80)).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 2,
                "event": {"type": "TaskAdd", "name": "Drinker", "params": []},
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        wait_updates(&bot_service, session_id).await;
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"UIMessage","id":6,"kind":"add-task","arguments":[{"type":"Long","value":1},{"type":"Str","value":"Drinker"},{"type":"Bytes","value":[]}]}"#,
            "BotService port={}", bot_service.port
        );
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"LockWidget","value":"sm"}"#,
            "BotService port={}", bot_service.port
        );
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"WidgetMessage","sender":13,"kind":"iact","arguments":[{"type":"Coord","value":{"x":0,"y":0}},{"type":"Int","value":0}]}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 3,
                "event": {
                    "type": "NewWidget",
                    "id": 38,
                    "kind": "sm",
                    "parent": 65536,
                    "pargs": [],
                    "cargs": [
                        {"type": "Str", "value": "Drink"},
                        {"type": "Str", "value": "Sip"},
                        {"type": "Str", "value": "Empty"},
                    ],
                },
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 4,
                "event": {"type": "AddWidget", "id": 38, "parent": 0, "pargs": []},
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"WidgetMessage","sender":38,"kind":"cl","arguments":[{"type":"Int","value":0},{"type":"Int","value":0}]}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&json!({
                "session": session_id,
                "number": number + 5,
                "event": {"type": "UIMessage", "id": 38, "msg": "act", "args": []},
            })).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        assert_eq!(
            bot_service.push(&make_set_meter(session_id, number + 6, 33, 100)).await,
            r#"{"type":"Ok"}"#,
            "BotService port={}", bot_service.port
        );
        wait_for_message(&bot_service, session_id).await;
        assert_eq!(
            bot_service.poll(session_id).await,
            r#"{"type":"Done","task":"Drinker"}"#,
            "BotService port={}", bot_service.port
        );
    }).await;
}

async fn with_bot_service<R: Future<Output=()>>(mut f: impl FnMut(BotService) -> R) {
    std::env::set_var("RUST_LOG", "error");
    match env_logger::try_init() {
        _ => (),
    }
    let port = pick_unused_port().unwrap();
    match std::fs::remove_dir_all(format!("tests/var/{}", port)) {
        _ => (),
    }
    std::fs::create_dir_all(format!("tests/var/{}", port)).unwrap();
    let server = run_server(make_config(port)).unwrap();
    f(BotService { port }).await;
    server.stop(true).await;
}

struct BotService {
    port: Port,
}

impl BotService {
    async fn ping(&self) -> String {
        Client::builder().build().unwrap()
            .get(self.url("ping").as_str())
            .timeout(Duration::from_secs(5))
            .send().await.unwrap()
            .text().await.unwrap()
    }

    async fn sessions(&self) -> String {
        Client::builder().build().unwrap()
            .get(self.url("sessions").as_str())
            .timeout(Duration::from_secs(5))
            .send().await.unwrap()
            .text().await.unwrap()
    }

    async fn push(&self, update: &Value) -> String {
        Client::builder().build().unwrap()
            .put(self.url("push").as_str())
            .body(serde_json::to_string(update).unwrap())
            .timeout(Duration::from_secs(5))
            .send().await.unwrap()
            .text().await.unwrap()
    }

    async fn poll(&self, session: i64) -> String {
        Client::builder().build().unwrap()
            .get(self.url("poll").as_str())
            .query(&[("session", session)])
            .timeout(Duration::from_secs(5))
            .send().await.unwrap()
            .text().await.unwrap()
    }

    async fn add_visualization(&self, session: i64) -> String {
        Client::builder().build().unwrap()
            .get(self.url("add_visualization").as_str())
            .query(&[("session", session)])
            .timeout(Duration::from_secs(5))
            .send().await.unwrap()
            .text().await.unwrap()
    }

    fn url(&self, endpoint: &str) -> String {
        format!("http://127.0.0.1:{}/{}", self.port, endpoint)
    }
}

fn make_config(port: Port) -> ServerConfig {
    serde_yaml::from_str(format!(r"---
bind_addr: '127.0.0.1:{0}'
map_db_path: tests/var/{0}/map.db
map_cache_ttl: 1
process:
  sessions_path: tests/var/{0}/sessions
  write_updates_log: true
  poll_timeout: 0.01
session:
  world:
    report_iterations: 100000
    found_transition_color: [ 1.0, 1.0, 1.0, 0.2 ]
    path_transition_color: [ 0.6, 0.8, 0.6, 0.8 ]
    shorten_path_transition_color: [ 0.4, 0.8, 0.4, 0.9 ]
    direct_path_transition_color: [ 0.8, 0.4, 0.2, 0.9 ]
    water_tiles:
      gfx/tiles/deep: 1
      gfx/tiles/odeep: 1
      gfx/tiles/owater: 3
      gfx/tiles/water: 3
    ice_tiles:
      gfx/tiles/ice: 1
  player:
    meters:
      stamina: gfx/hud/meter/stam
    equipment:
      belt: 5
    items:
      content: ui/tt/cont
      content_name: ui/tt/cn
      quality: ui/tt/q/quality
  tasks:
    path_finder:
      find_path_max_shortcut_length: 25
      find_path_max_iterations: 100000
      max_next_point_shortcut_length: 50
    explorer:
      find_path_max_shortcut_length: 25
      find_path_max_iterations: 1000000
      max_next_point_shortcut_length: 50
    drinker:
      open_belt_timeout: 1.0
      sip_timeout: 1.0
      max_stamina: 100
      stamina_threshold: 95
      liquid_containers:
        - gfx/invobjs/kuksa
        - gfx/invobjs/kuksa-full
        - gfx/invobjs/waterskin
        - gfx/invobjs/waterflask
        - gfx/invobjs/small/waterskin
      contents:
        - name: juice
          action: Sip
          wait_interval: 1
        - name: Water
          action: Drink
          wait_interval: 3
visualization:
  window_type: SDL2
", port).as_str()).unwrap()
}

fn read_updates<P: AsRef<Path>>(path: P) -> Vec<Value> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .map(|v| serde_json::from_str::<Value>(&v.unwrap()).unwrap()).collect()
}

#[derive(Deserialize)]
struct Sessions {
    value: Vec<Session>,
}

#[derive(Deserialize)]
struct Session {
    id: i64,
    updates: i64,
    messages: i64,
}

fn parse_session(text: &String) -> Sessions {
    serde_json::from_str::<Sessions>(text).unwrap()
}

async fn wait_updates(bot_service: &BotService, session_id: i64) {
    while parse_session(&bot_service.sessions().await).value.iter()
        .find(|v| v.id == session_id)
        .unwrap()
        .updates != 0 {
        sleep(Duration::from_secs(1))
    }
}

async fn wait_for_message(bot_service: &BotService, session_id: i64) {
    while parse_session(&bot_service.sessions().await).value.iter()
        .find(|v| v.id == session_id)
        .unwrap()
        .messages == 0 {
        sleep(Duration::from_secs(1))
    }
}

fn parse_json(text: &String) -> Value {
    serde_json::from_str::<Value>(text).unwrap()
}

#[derive(Copy, Clone)]
struct Coord {
    x: i64,
    y: i64,
}

fn get_map_click_coord(value: &Value) -> Coord {
    let coord = value["arguments"].as_array().unwrap()[1]["value"].clone();
    Coord { x: coord["x"].as_i64().unwrap(), y: coord["y"].as_i64().unwrap() }
}

const TILE_SIZE: f64 = 11.0;
const RESOLUTION: f64 = hexf64!("0x1.0p-10") * TILE_SIZE;

fn make_map_click(session_id: i64, number: i64, x: i64, y: i64) -> Value {
    json!({
        "session": session_id,
        "number": number,
        "event": {
            "type": "WidgetMessage",
            "id": 7,
            "msg": "click",
            "args": [
                {
                    "type": "Coord",
                    "value": {"x": 0, "y": 0},
                },
                {
                    "type": "Coord",
                    "value": {"x": x, "y": y},
                },
                {
                    "type": "Int",
                    "value": 1,
                },
                {
                    "type": "Int",
                    "value": 4,
                },
            ],
        },
    })
}

fn make_gob_move(session_id: i64, number: i64, id: i64, x: f64, y: f64) -> Value {
    json!({
        "session": session_id,
        "number": number,
        "event": {
            "type":"GobMove",
            "id": id,
            "position": {"x": x, "y": y},
            "angle": 0.0,
        },
    })
}

fn make_set_meter(session_id: i64, number: i64, id: i64, value: i32) -> Value {
    json!({
        "session": session_id,
        "number": number,
        "event": {
            "type": "UIMessage",
            "id": id,
            "msg": "set",
            "args": [
                {"type": "Color", "value": {"r": 64, "g": 64, "b": 255, "a": 255}},
                {"type": "Int", "value": value},
            ],
        },
    })
}
