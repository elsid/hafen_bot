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
    inventory_id: Option<i32>,
    belt_id: Option<i32>,
    belt_inventory_id: Option<i32>,
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
    widget_inventories: BTreeMap<i32, BTreeMap<i32, Item>>,
    hand: Option<Item>,
}

impl Player {
    pub fn new(config: PlayerConfig) -> Self {
        Self {
            map_view_id: None,
            game_ui_id: None,
            inventory_id: None,
            belt_id: None,
            belt_inventory_id: None,
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
            widget_inventories: BTreeMap::new(),
            hand: None,
        }
    }

    pub fn map_view_id(&self) -> Option<i32> {
        self.map_view_id
    }

    pub fn game_ui_id(&self) -> Option<i32> {
        self.game_ui_id
    }

    pub fn inventory_id(&self) -> Option<i32> {
        self.inventory_id
    }

    pub fn belt_inventory_id(&self) -> Option<i32> {
        self.belt_inventory_id
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

    pub fn widget_inventories(&self) -> &BTreeMap<i32, BTreeMap<i32, Item>> {
        &self.widget_inventories
    }

    pub fn hand(&self) -> &Option<Item> {
        &self.hand
    }

    pub fn from_player_data(data: PlayerData, config: PlayerConfig) -> Self {
        let belt_inventory_id = data.widgets.iter()
            .find(|v| v.kind == "inv" && Some(v.parent) == data.belt_id)
            .map(|v| v.id);
        let inventory_id = data.widgets.iter()
            .find(|v| v.kind == "inv" && Some(v.parent) == data.game_ui_id && v.pargs.len() >= 1 && v.pargs[0] == "inv")
            .map(|v| v.id);
        let widgets = data.widgets.into_iter().map(|v| (v.id, v)).collect();
        let resources = data.resources.into_iter().map(|v| (v.id, v)).collect();
        let items: BTreeMap<i32, Item> = data.items.into_iter().map(|v| (v.id, v)).collect();
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
            inventory_id,
            belt_id: data.belt_id,
            belt_inventory_id,
            hand: data.game_ui_id
                .and_then(|game_ui| {
                    widgets.values()
                        .find(|widget| widget.parent == game_ui && widget.kind == "item")
                        .and_then(|widget| items.get(&widget.id).cloned())
                }),
            widget_inventories: widgets.values()
                .filter(|v| v.kind == "inv")
                .map(|v| (v.id, make_inventory(Some(v.id), &widgets, &items)))
                .collect(),
            widgets,
            map_grids: data.map_grids,
            resources,
            stuck_detector: StuckDetector::new(),
            is_stuck: false,
        }
    }

    pub fn as_player_data(&self) -> PlayerData {
        let mut items = Vec::new();
        for inventory in self.widget_inventories.values() {
            clone_items(&mut items, &inventory);
        }
        if let Some(item) = self.hand.clone() {
            items.push(item);
        }
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
                        } else if Some(*parent) == self.game_ui_id {
                            self.hand = make_item(*id, cargs, None);
                        } else if let Some(inventory) = self.widget_inventories.get_mut(parent) {
                            debug!("Player: add inventory item id={} parent={}", id, parent);
                            add_inventory_item(*id, pargs, cargs, inventory);
                        }
                    }
                    "wnd" => {
                        if cargs.len() >= 2 && cargs[1] == "Belt" {
                            self.belt_id = Some(*id);
                        }
                    }
                    "inv" => {
                        debug!("Player: add widget inventory id={} parent={} pargs={:?}", id, parent, pargs);
                        if Some(*parent) == self.game_ui_id && pargs.len() >= 1 && pargs[0] == "inv" {
                            debug!("Player: set inventory id");
                            self.inventory_id = Some(*id);
                        } else if Some(*parent) == self.belt_id {
                            debug!("Player: set belt inventory id");
                            self.belt_inventory_id = Some(*id);
                        }
                        self.widget_inventories.insert(*id, BTreeMap::new());
                    }
                    _ => (),
                }
                self.widgets.insert(*id, Widget {
                    id: *id,
                    parent: *parent,
                    kind: kind.clone(),
                    pargs: pargs.clone(),
                    cargs: cargs.clone(),
                    pargs_add: Vec::new(),
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
                        let items = &self.items;
                        self.hand.as_mut().map(|item| item.id == *id && update_item(args, items, item)).unwrap_or(false)
                            || self.widget_inventories.values_mut()
                            .any(|v| update_inventory_item(*id, args, items, v))
                    }
                    _ => false,
                }
            }
            Event::AddWidget { id, parent, pargs } => {
                let widget = self.widgets.get_mut(id).unwrap();
                widget.parent = *parent;
                widget.pargs_add = pargs.clone();
                true
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
                } else if Some(*id) == self.belt_inventory_id {
                    self.belt_inventory_id = None;
                }
                if let Some(widget) = self.widgets.remove(id) {
                    if self.hand.as_ref().map(|item| item.id == widget.id).unwrap_or(false) {
                        self.hand = None;
                    } else if self.widget_inventories.remove(&widget.id).is_none() {
                        self.widget_inventories.get_mut(&widget.parent)
                            .map(|v| v.remove(id));
                    }
                    true
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
        Self {
            widget_id: resource.and_then(|resource| {
                widgets.values()
                    .find(|widget| widget.kind == "im" && widget.cargs.len() >= 1 && widget.cargs[0] == resource)
                    .map(|widget| widget.id)
            }),
            value,
        }
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

fn make_inventory(widget_id: Option<i32>, widgets: &BTreeMap<i32, Widget>, items: &BTreeMap<i32, Item>) -> BTreeMap<i32, Item> {
    let mut result = BTreeMap::new();
    for widget in widgets.values() {
        if Some(widget.parent) == widget_id {
            if let Some(item) = items.get(&widget.id) {
                result.insert(item.id, Item {
                    id: item.id,
                    resource: item.resource,
                    content: item.content.clone(),
                    position: item.position,
                });
            }
        }
    }
    result
}

fn add_inventory_item(id: i32, pargs: &Vec<Value>, cargs: &Vec<Value>, inventory: &mut BTreeMap<i32, Item>) {
    let position = pargs.iter().find_map(|v| {
        if let Value::Coord { value } = v {
            return Some(*value);
        }
        return None;
    });
    if cargs.len() >= 1 {
        if let Some(item) = make_item(id, cargs, position) {
            inventory.insert(id, item);
        }
    }
}

fn update_inventory_item(id: i32, args: &Vec<Value>, items: &Items, inventory: &mut BTreeMap<i32, Item>) -> bool {
    if let Some(item) = inventory.get_mut(&id) {
        return update_item(args, items, item);
    }
    false
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
    pub pargs_add: Vec<Value>,
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
    pub position: Option<Vec2i>,
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

fn make_item(id: i32, cargs: &Vec<Value>, position: Option<Vec2i>) -> Option<Item> {
    if cargs.len() >= 1 {
        if let Value::Int { value } = &cargs[0] {
            return Some(Item {
                id,
                resource: *value,
                content: None,
                position,
            });
        }
    }
    None
}

fn update_item(args: &Vec<Value>, items: &Items, item: &mut Item) -> bool {
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
    false
}
