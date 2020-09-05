use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::bot::vec2::Vec2f;

pub struct Objects {
    objects: BTreeMap<i64, VecDeque<Object>>,
    objects_by_name: BTreeMap<String, i64>,
}

impl Objects {
    pub fn new() -> Self {
        Self {
            objects: BTreeMap::new(),
            objects_by_name: BTreeMap::new(),
        }
    }

    pub fn from_objects_data(data: ObjectsData) -> Self {
        Self {
            objects_by_name: data.objects.iter()
                .filter_map(|v| v.name.as_ref().map(|name| (name.clone(), v.id)))
                .collect(),
            objects: data.objects.into_iter()
                .map(|object| {
                    (object.id, {
                        let mut values = VecDeque::new();
                        values.push_back(object);
                        values
                    })
                })
                .collect(),
        }
    }

    pub fn as_objects_data(&self) -> ObjectsData {
        ObjectsData {
            objects: self.objects.values().filter_map(|v| v.back()).cloned().collect(),
        }
    }

    pub fn add(&mut self, object: Object) {
        if let Some(name) = object.name.as_ref() {
            self.objects_by_name.insert(name.clone(), object.id);
        }
        self.objects.entry(object.id).or_insert_with(|| VecDeque::new()).push_back(object);
    }

    pub fn get_by_id(&self, object_id: i64) -> Option<&Object> {
        self.objects.get(&object_id).and_then(|v| v.back())
    }

    pub fn get_by_name(&self, name: &String) -> Option<&Object> {
        self.objects_by_name.get(name).and_then(|id| self.get_by_id(*id))
    }

    pub fn remove(&mut self, object_id: i64) -> bool {
        let mut remove = None;
        if let Some(values) = self.objects.get_mut(&object_id) {
            if let Some(removed) = values.pop_front().map(|v| v.name) {
                if values.is_empty() {
                    remove = Some(removed);
                }
            }
        }
        if let Some(name_opt) = remove {
            if let Some(name) = name_opt {
                self.objects_by_name.remove(&name);
            }
            self.objects.remove(&object_id);
            return true;
        }
        false
    }

    pub fn update(&mut self, object_id: i64, position: Vec2f, angle: f64) -> bool {
        if let Some(values) = self.objects.get_mut(&object_id) {
            if let Some(object) = values.back_mut() {
                object.position = position;
                object.angle = angle;
                return true;
            }
        }
        false
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn iter(&self) -> impl Iterator<Item=&Object> {
        self.objects.values().filter_map(|v| v.back())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ObjectsData {
    objects: Vec<Object>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Object {
    pub id: i64,
    pub position: Vec2f,
    pub angle: f64,
    pub name: Option<String>,
}
