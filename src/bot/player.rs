use std::collections::BTreeMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::bot::map::pos_to_grid_pos;
use crate::bot::protocol::{Event, Update, Value};
use crate::bot::stuck_detector::StuckDetector;
use crate::bot::vec2::{Vec2f, Vec2i};
use crate::bot::world::World;

#[derive(Clone, Deserialize)]
pub struct PlayerConfig {
    pub meters: MetersConfig,
    pub equipment: EquipmentConfig,
    pub items: ItemsConfig,
}

#[derive(Clone, Deserialize)]
pub struct ItemsConfig {
    pub content: String,
    pub content_name: String,
    pub quality: String,
}

#[derive(Clone, Deserialize)]
pub struct MetersConfig {
    pub stamina: String,
}

#[derive(Clone, Deserialize)]
pub struct EquipmentConfig {
    pub belt: usize,
}

pub struct PlayerEquipment<'a> {
    config: &'a EquipmentConfig,
    slots: &'a BTreeMap<usize, Option<i32>>,
}

impl<'a> PlayerEquipment<'a> {
    pub fn belt(&self) -> Option<i32> {
        self.slots.get(&self.config.belt).and_then(|v| *v)
    }
}

pub struct Player {
    map_view_id: Option<i32>,
    game_ui_id: Option<i32>,
    belt_id: Option<i32>,
    name: Option<String>,
    object_id: Option<i64>,
    grid_id: Option<i64>,
    position: Option<Vec2f>,
    widgets: BTreeMap<i32, Widget>,
    map_grids: Vec<MapGrid>,
    resources: BTreeMap<i32, Resource>,
    stuck_detector: StuckDetector,
    is_stuck: bool,
    meters: Meters,
    items: Items,
    stamina: Stamina,
    equipment: Equipment,
    inventory: Inventory,
    belt: Inventory,
}

impl Player {
    pub fn new(config: PlayerConfig) -> Self {
        Self {
            map_view_id: None,
            game_ui_id: None,
            belt_id: None,
            name: None,
            object_id: None,
            grid_id: None,
            position: None,
            widgets: BTreeMap::new(),
            map_grids: Vec::new(),
            resources: BTreeMap::new(),
            stuck_detector: StuckDetector::new(),
            is_stuck: false,
            meters: Meters::new(config.meters.clone()),
            items: Items::new(config.items.clone()),
            stamina: Stamina::default(),
            equipment: Equipment::new(config.equipment.clone()),
            inventory: Inventory::default(),
            belt: Inventory::default(),
        }
    }

    pub fn map_view_id(&self) -> Option<i32> {
        self.map_view_id
    }

    pub fn game_ui_id(&self) -> Option<i32> {
        self.game_ui_id
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn object_id(&self) -> Option<i64> {
        self.object_id
    }

    pub fn grid_id(&self) -> Option<i64> {
        self.grid_id
    }

    pub fn widgets(&self) -> &BTreeMap<i32, Widget> {
        &self.widgets
    }

    pub fn resources(&self) -> &BTreeMap<i32, Resource> {
        &self.resources
    }

    pub fn is_stuck(&self) -> bool {
        self.is_stuck
    }

    pub fn stamina(&self) -> Option<i32> {
        self.stamina.value
    }

    pub fn equipment(&self) -> Option<PlayerEquipment> {
        self.equipment.widget_id.map(|_| PlayerEquipment {
            config: &self.equipment.config,
            slots: &self.equipment.slots,
        })
    }

    pub fn inventory_items(&self) -> &BTreeMap<i32, Item> {
        &self.inventory.items
    }

    pub fn belt_items(&self) -> &BTreeMap<i32, Item> {
        &self.belt.items
    }

    pub fn from_player_data(data: PlayerData, config: PlayerConfig) -> Self {
        let widgets = data.widgets.into_iter().map(|v| (v.id, v)).collect();
        let resources = data.resources.into_iter().map(|v| (v.id, v)).collect();
        let items = data.items.iter().map(|v| (v.id, v)).collect();
        let meters = Meters::from_resources(&resources, config.meters.clone());
        Self {
            map_view_id: data.map_view_id,
            game_ui_id: data.game_ui_id,
            name: data.name,
            object_id: data.object_id,
            grid_id: data.grid_id,
            position: data.position,
            items: Items::from_resources(&resources, config.items.clone()),
            stamina: Stamina::new(data.stamina, &widgets, meters.stamina),
            meters,
            equipment: Equipment::from_widgets(&widgets, config.equipment.clone()),
            inventory: Inventory::inventory(data.game_ui_id, &widgets, &items),
            belt_id: data.belt_id,
            belt: Inventory::belt(data.belt_id, &widgets, &items),
            widgets,
            map_grids: data.map_grids,
            resources,
            stuck_detector: StuckDetector::new(),
            is_stuck: false,
        }
    }

    pub fn as_player_data(&self) -> PlayerData {
        let mut items = Vec::new();
        clone_items(&mut items, &self.inventory.items);
        clone_items(&mut items, &self.belt.items);
        PlayerData {
            map_view_id: self.map_view_id,
            game_ui_id: self.game_ui_id,
            belt_id: self.belt_id,
            name: self.name.clone(),
            object_id: self.object_id,
            grid_id: self.grid_id,
            position: self.position,
            widgets: self.widgets.values().cloned().collect(),
            map_grids: self.map_grids.clone(),
            resources: self.resources.values().cloned().collect(),
            stamina: self.stamina.value,
            items,
        }
    }

    pub fn update(&mut self, world: &World, update: &Update) -> bool {
        match &update.event {
            Event::NewWidget { id, kind, parent, pargs, cargs } => {
                match kind.as_str() {
                    "gameui" => {
                        self.game_ui_id = Some(*id);
                        if cargs.len() >= 2 {
                            if let Value::Str { value } = &cargs[0] {
                                self.name = Some(value.clone());
                            }
                            if let Value::Int { value } = &cargs[1] {
                                self.object_id = Some(*value as i64);
                                if let Some(object) = world.objects().get_by_id(*value as i64) {
                                    self.update_player(object.id, object.position);
                                }
                            }
                        }
                    }
                    "mapview" => {
                        self.map_view_id = Some(*id);
                    }
                    "im" => {
                        if let Some(resource) = self.meters.stamina {
                            self.stamina.update_widget_id(resource, *id, cargs);
                        }
                    }
                    "epry" => {
                        self.equipment.widget_id = Some(*id);
                    }
                    "item" => {
                        if Some(*parent) == self.equipment.widget_id {
                            self.equipment.add_item(*id, pargs);
                        } else if Some(*parent) == self.inventory.widget_id {
                            self.inventory.add_item(*id, cargs);
                        } else if Some(*parent) == self.belt.widget_id {
                            self.belt.add_item(*id, cargs);
                        }
                    }
                    "wnd" => {
                        if cargs.len() >= 2 && cargs[1] == "Belt" {
                            self.belt_id = Some(*id);
                        }
                    }
                    "inv" => {
                        if Some(*parent) == self.game_ui_id && pargs.len() >= 1 && pargs[0] == "inv" {
                            self.inventory.widget_id = Some(*id);
                        } else if Some(*parent) == self.belt_id {
                            self.belt.widget_id = Some(*id);
                        }
                    }
                    _ => (),
                }
                self.widgets.insert(*id, Widget {
                    id: *id,
                    parent: *parent,
                    kind: kind.clone(),
                    pargs: pargs.clone(),
                    cargs: cargs.clone(),
                });
                true
            }
            Event::UIMessage { id, msg, args } => {
                match msg.as_str() {
                    "plob" => {
                        if Some(*id) == self.map_view_id && args.len() > 0 {
                            match &args[0] {
                                Value::Nil => {
                                    self.object_id = None;
                                    true
                                }
                                Value::Int { value } => {
                                    self.object_id = Some((*value).into());
                                    true
                                }
                                _ => false,
                            }
                        } else {
                            false
                        }
                    }
                    "set" => {
                        self.stamina.update_value(*id, args)
                    }
                    "tt" => {
                        self.inventory.update_item(*id, args, &self.items)
                            || self.belt.update_item(*id, args, &self.items)
                    }
                    _ => false,
                }
            }
            Event::Destroy { id } => {
                if Some(*id) == self.game_ui_id {
                    self.game_ui_id = None;
                } else if Some(*id) == self.map_view_id {
                    self.map_view_id = None;
                } else if Some(*id) == self.belt_id {
                    self.belt_id = None;
                } else if Some(*id) == self.equipment.widget_id {
                    self.equipment.widget_id = None;
                } else if Some(*id) == self.belt.widget_id {
                    self.belt.widget_id = None;
                }
                if let Some(widget) = self.widgets.remove(id) {
                    if Some(widget.parent) == self.inventory.widget_id {
                        self.inventory.remove_item(*id).is_some()
                    } else if Some(widget.parent) == self.belt.widget_id {
                        self.belt.remove_item(*id).is_some()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            Event::MapGridAdd { grid, neighbours: _ } => {
                self.map_grids.push(MapGrid { id: grid.id, position: grid.position });
                if Some(grid.position) == self.position.map(|v| pos_to_grid_pos(v)) {
                    self.grid_id = Some(grid.id);
                    debug!("Player: set grid: {}", grid.id);
                }
                true
            }
            Event::MapGridRemove { id } => {
                if self.grid_id == Some(*id) {
                    self.grid_id = None;
                    debug!("Player: reset grid");
                }
                self.map_grids.retain(|grid| grid.id != *id);
                true
            }
            Event::GobAdd { id, position, angle: _, name: _ } => {
                self.update_player(*id, *position)
            }
            Event::GobRemove { id } => {
                if Some(*id) == self.object_id {
                    self.position = None;
                    self.grid_id = None;
                    self.stuck_detector = StuckDetector::new();
                    self.is_stuck = false;
                    debug!("Player: reset");
                    true
                } else {
                    false
                }
            }
            Event::GobMove { id, position, angle: _ } => {
                self.update_player(*id, *position)
            }
            Event::ResourceAdd { id, version, name } => {
                let resource = Resource { id: *id, version: *version, name: name.clone() };
                self.meters.update(&resource);
                self.items.update(&resource);
                self.resources.insert(*id, resource);
                true
            }
            _ => false,
        }
    }

    fn update_player(&mut self, object_id: i64, object_position: Vec2f) -> bool {
        if self.object_id == Some(object_id) {
            self.position = Some(object_position);
            let grid_position = pos_to_grid_pos(object_position);
            if let Some(grid) = self.map_grids.iter().find(|v| v.position == grid_position) {
                self.grid_id = Some(grid.id);
            }
            let now = Instant::now();
            self.is_stuck = self.stuck_detector.check(object_position, now);
            self.stuck_detector.update(object_position, now);
            if self.is_stuck {
                debug!("Player is stuck at {:?}", object_position);
            }
            true
        } else {
            false
        }
    }
}

struct Meters {
    config: MetersConfig,
    stamina: Option<i32>,
}

impl Meters {
    fn new(config: MetersConfig) -> Self {
        Self {
            config,
            stamina: None,
        }
    }

    fn from_resources(resources: &BTreeMap<i32, Resource>, config: MetersConfig) -> Self {
        let mut result = Self::new(config);
        for resource in resources.values() {
            result.update(resource);
        }
        result
    }

    fn update(&mut self, resource: &Resource) {
        if resource.name == self.config.stamina {
            self.stamina = Some(resource.id);
        }
    }
}

struct Items {
    config: ItemsConfig,
    content: Option<i32>,
    content_name: Option<i32>,
    quality: Option<i32>,
}

impl Items {
    fn new(config: ItemsConfig) -> Self {
        Self {
            config,
            content: None,
            content_name: None,
            quality: None,
        }
    }

    fn from_resources(resources: &BTreeMap<i32, Resource>, config: ItemsConfig) -> Self {
        let mut result = Self::new(config);
        for resource in resources.values() {
            result.update(resource);
        }
        result
    }

    fn update(&mut self, resource: &Resource) {
        if resource.name == self.config.content {
            self.content = Some(resource.id);
        } else if resource.name == self.config.content_name {
            self.content_name = Some(resource.id);
        } else if resource.name == self.config.quality {
            self.quality = Some(resource.id);
        }
    }
}

#[derive(Default)]
struct Stamina {
    widget_id: Option<i32>,
    value: Option<i32>,
}

impl Stamina {
    fn new(value: Option<i32>, widgets: &BTreeMap<i32, Widget>, resource: Option<i32>) -> Self {
        let mut result = Self { widget_id: None, value };
        if let (Some(resource), Some((_, widget))) = (resource, widgets.iter().find(|(_, v)| v.kind == "im")) {
            result.update_widget_id(resource, widget.id, &widget.cargs);
        }
        result
    }

    fn update_widget_id(&mut self, resource: i32, id: i32, cargs: &Vec<Value>) {
        if cargs.len() >= 1 && cargs[0] == resource {
            self.widget_id = Some(id);
        }
    }

    fn update_value(&mut self, id: i32, args: &Vec<Value>) -> bool {
        if self.widget_id == Some(id) && args.len() >= 2 {
            if let Value::Int { value } = &args[1] {
                self.value = Some(*value);
                return true;
            }
        }
        false
    }
}

struct Equipment {
    config: EquipmentConfig,
    widget_id: Option<i32>,
    slots: BTreeMap<usize, Option<i32>>,
}

impl Equipment {
    fn new(config: EquipmentConfig) -> Self {
        Self { config, widget_id: None, slots: BTreeMap::new() }
    }

    fn from_widgets(widgets: &BTreeMap<i32, Widget>, config: EquipmentConfig) -> Self {
        let widget_id = widgets.iter().find(|(_, v)| v.kind == "epry").map(|(_, v)| v.id);
        let mut result = Self { config, widget_id, slots: BTreeMap::new() };
        for widget in widgets.values() {
            if Some(widget.parent) == widget_id {
                result.add_item(widget.id, &widget.pargs);
            }
        }
        result
    }

    fn add_item(&mut self, id: i32, pargs: &Vec<Value>) {
        if pargs.len() >= 1 {
            if let Value::Int { value: index } = &pargs[0] {
                self.slots.insert(*index as usize, Some(id));
            }
        }
    }
}

#[derive(Default)]
struct Inventory {
    widget_id: Option<i32>,
    items: BTreeMap<i32, Item>,
}

impl Inventory {
    fn inventory(game_ui_id: Option<i32>, widgets: &BTreeMap<i32, Widget>, items: &BTreeMap<i32, &Item>) -> Self {
        let widget_id = widgets.iter()
            .find(|(_, v)| v.kind == "inv" && Some(v.parent) == game_ui_id && v.pargs.len() >= 1 && v.pargs[0] == "inv")
            .map(|(_, v)| v.id);
        Self::new(widget_id, widgets, items)
    }

    fn belt(window_id: Option<i32>, widgets: &BTreeMap<i32, Widget>, items: &BTreeMap<i32, &Item>) -> Self {
        let widget_id = widgets.iter()
            .find(|(_, v)| v.kind == "inv" && Some(v.parent) == window_id)
            .map(|(_, v)| v.id);
        Self::new(widget_id, widgets, items)
    }

    fn new(widget_id: Option<i32>, widgets: &BTreeMap<i32, Widget>, items: &BTreeMap<i32, &Item>) -> Self {
        let mut result = Self { widget_id, items: BTreeMap::new() };
        for widget in widgets.values() {
            if Some(widget.parent) == widget_id {
                if let Some(item) = items.get(&widget.id) {
                    result.items.insert(item.id, Item {
                        id: item.id,
                        resource: item.resource,
                        content: item.content.clone(),
                    });
                }
            }
        }
        result
    }

    fn add_item(&mut self, id: i32, cargs: &Vec<Value>) {
        if cargs.len() >= 1 {
            if let Value::Int { value } = &cargs[0] {
                self.items.insert(id, Item {
                    id,
                    resource: *value,
                    content: None,
                });
            }
        }
    }

    fn remove_item(&mut self, id: i32) -> Option<Item> {
        self.items.remove(&id)
    }

    fn update_item(&mut self, id: i32, args: &Vec<Value>, items: &Items) -> bool {
        if let Some(item) = self.items.get_mut(&id) {
            if args.len() >= 3 {
                if let Value::List { value: content } = &args[2] {
                    if let (Some(content_res), Some(content_name_res), Some(quality_res)) = (items.content, items.content_name, items.quality) {
                        if content.len() >= 2 && content[0] == content_res {
                            if let Value::List { value: parameters } = &content[1] {
                                if let (Some(name), Some(quality)) = (get_string(parameters, content_name_res), get_float32(parameters, quality_res)) {
                                    item.content = Some(Content { name: name.clone(), quality });
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
            if item.content.is_some() {
                item.content = None;
                return true;
            }
        }
        false
    }
}

fn get_float32(values: &Vec<Value>, resource: i32) -> Option<f32> {
    values.iter()
        .find_map(|v| {
            if let Value::List { value: key_value } = v {
                if key_value.len() >= 2 {
                    if key_value[0] == resource {
                        if let Value::Float32 { value } = key_value[1] {
                            return Some(value);
                        }
                    }
                }
            }
            None
        })
}

fn get_string(values: &Vec<Value>, resource: i32) -> Option<&String> {
    values.iter()
        .find_map(|v| {
            if let Value::List { value: key_value } = v {
                if key_value.len() >= 2 {
                    if key_value[0] == resource {
                        if let Value::Str { value } = &key_value[1] {
                            return Some(value);
                        }
                    }
                }
            }
            None
        })
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerData {
    map_view_id: Option<i32>,
    game_ui_id: Option<i32>,
    belt_id: Option<i32>,
    name: Option<String>,
    object_id: Option<i64>,
    grid_id: Option<i64>,
    position: Option<Vec2f>,
    widgets: Vec<Widget>,
    map_grids: Vec<MapGrid>,
    resources: Vec<Resource>,
    stamina: Option<i32>,
    items: Vec<Item>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Widget {
    pub id: i32,
    pub parent: i32,
    pub kind: String,
    pub pargs: Vec<Value>,
    pub cargs: Vec<Value>,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct MapGrid {
    id: i64,
    position: Vec2i,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Resource {
    pub id: i32,
    pub version: i32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub id: i32,
    pub resource: i32,
    pub content: Option<Content>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Content {
    pub name: String,
    pub quality: f32,
}

fn clone_items(items: &mut Vec<Item>, src: &BTreeMap<i32, Item>) {
    for item in src.values() {
        items.push(item.clone());
    }
}
