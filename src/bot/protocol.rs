use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::bot::map::GridNeighbour;
use crate::bot::session::SessionData;
use crate::bot::vec2::{Vec2f, Vec2i};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Update {
    pub session: i64,
    pub number: i64,
    pub event: Event,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Event {
    NewWidget {
        id: i32,
        kind: String,
        parent: i32,
        pargs: Vec<Value>,
        cargs: Vec<Value>,
    },
    UIMessage {
        id: i32,
        msg: String,
        args: Vec<Value>,
    },
    Destroy {
        id: i32,
    },
    AddWidget {
        id: i32,
        parent: i32,
        pargs: Vec<Value>,
    },
    MapTile {
        id: i32,
        version: i32,
        name: String,
        color: i32,
    },
    MapGridAdd {
        grid: MapGrid,
        neighbours: Vec<GridNeighbour>,
    },
    MapGridUpdate {
        grid: MapGrid,
    },
    MapGridRemove {
        id: i64,
    },
    GobAdd {
        id: i64,
        position: Vec2f,
        angle: f64,
        name: Option<String>,
    },
    GobRemove {
        id: i64,
    },
    GobMove {
        id: i64,
        position: Vec2f,
        angle: f64,
    },
    ResourceAdd {
        id: i32,
        version: i32,
        name: String,
    },
    WidgetMessage {
        id: i32,
        msg: String,
        args: Vec<Value>,
    },
    Close,
    TaskAdd {
        name: String,
        params: Vec<u8>,
    },
    TaskRemove {
        id: i64,
    },
    VisualizationAdd,
    SessionData { value: Option<String> },
    GetSessionData,
    Cancel,
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq, Clone)]
#[serde(tag = "type")]
pub enum Value {
    Nil,
    Int { value: i32 },
    Long { value: i64 },
    Str { value: String },
    Coord { value: Vec2i },
    Bytes { value: Vec<u8> },
    Color { value: Color },
    Float32 { value: f32 },
    Float64 { value: f64 },
    FCoord64 { value: Vec2f },
    List { value: Vec<Value> },
}

macro_rules! value_from_impl {
    ($type: ty, $variant: tt) => {
        impl From<$type> for Value {
            fn from(value: $type) -> Self {
                Value::$variant { value: value as $type }
            }
        }
    }
}

macro_rules! value_from_to_impl {
    ($from: ty, $to: ty, $variant: tt) => {
        impl From<$from> for Value {
            fn from(value: $from) -> Self {
                Value::$variant { value: value as $to }
            }
        }
    }
}

value_from_impl! { i32, Int }
value_from_impl! { i64, Long }
value_from_impl! { String, Str }
value_from_impl! { Vec2i, Coord }
value_from_impl! { Vec<u8>, Bytes }
value_from_impl! { Color, Color }
value_from_impl! { f32, Float32 }
value_from_impl! { f64, Float64 }
value_from_impl! { Vec2f, FCoord64 }
value_from_impl! { Vec<Value>, List }
value_from_to_impl! { Button, i32, Int }
value_from_to_impl! { Modifier, i32, Int }

impl PartialEq<String> for Value {
    fn eq(&self, other: &String) -> bool {
        if let Value::Str { value } = self {
            value == other
        } else {
            false
        }
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        if let Value::Str { value } = self {
            value == other
        } else {
            false
        }
    }
}

impl PartialEq<i32> for Value {
    fn eq(&self, other: &i32) -> bool {
        if let Value::Int { value } = self {
            value == other
        } else {
            false
        }
    }
}

impl PartialOrd<i32> for Value {
    fn partial_cmp(&self, other: &i32) -> Option<Ordering> {
        if let Value::Int { value } = self {
            value.partial_cmp(other)
        } else {
            None
        }
    }
}

impl PartialEq<&[&str]> for Value {
    fn eq(&self, other: &&[&str]) -> bool {
        if let Value::List { value } = self {
            value == other
        } else {
            false
        }
    }
}

impl PartialEq<Vec2i> for Value {
    fn eq(&self, other: &Vec2i) -> bool {
        if let Value::Coord { value } = self {
            value == other
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum Message {
    Ok,
    Error { message: String },
    Sessions { value: Vec<SessionInfo> },
    WidgetMessage {
        sender: i32,
        kind: String,
        arguments: Vec<Value>,
    },
    UIMessage {
        id: i32,
        kind: String,
        arguments: Vec<Value>,
    },
    Done { task: String },
    Session { value: SessionData },
    SessionData { value: String },
    GetSessionData,
    LockWidget { value: String },
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq, Clone)]
pub struct Color {
    pub r: i32,
    pub g: i32,
    pub b: i32,
    pub a: i32,
}

#[allow(dead_code)]
pub enum Button {
    LeftClick = 1,
    RightClick = 3,
}

#[allow(dead_code)]
pub enum Modifier {
    None = 0,
    Shift = 1,
    Ctrl = 2,
    Alt = 4,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SessionInfo {
    pub id: i64,
    pub tasks: Vec<String>,
    pub updates: usize,
    pub messages: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapGrid {
    pub id: i64,
    pub position: Vec2i,
    pub heights: Vec<f32>,
    pub tiles: Vec<i32>,
}
