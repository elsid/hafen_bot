use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{JoinHandle, spawn};
use std::time::{Duration, Instant};

use glutin_window::GlutinWindow;
use graphics::{clear, Ellipse, Image, Rectangle, Transformed, Line};
use graphics::math::identity;
use graphics::rectangle::{centered_square, square};
use graphics::text::Text;
use image::{Rgba, RgbaImage};
use opengl_graphics::{Filter, GlGraphics, GlyphCache, OpenGL, Texture, TextureSettings};
use piston::{EventLoop, RenderArgs, RenderEvent, UpdateArgs, UpdateEvent, Window};
use piston::event_loop::{Events, EventSettings};
use piston::input::{
    Button,
    MouseButton,
    MouseRelativeEvent,
    MouseScrollEvent,
    PressEvent,
    ReleaseEvent,
};
use piston::window::WindowSettings;
use sdl2_window::Sdl2Window;
use serde::Deserialize;

use crate::bot::map::{Grid, grid_pos_to_pos, GRID_SIZE, tile_index_to_tile_pos, TILE_SIZE};
use crate::bot::map_db::MapDb;
use crate::bot::process::{count_updates, UpdatesQueue};
use crate::bot::protocol::Message;
use crate::bot::scene::{CompositeVecNode, Context, DebugTextNode, EllipseNode, ImageNode, MapTransformBoxNode, Node, Scene, TextNode, FixedScaleLineNode};
use crate::bot::session::Session;
use crate::bot::vec2::{Vec2f, Vec2i};
use crate::bot::world::PlayerWorld;
use crate::bot::navigator::make_areas;

#[derive(Clone, Deserialize)]
pub enum WindowType {
    Glutin,
    SDL2,
}

#[derive(Clone, Deserialize)]
pub struct VisualizationConfig {
    window_type: WindowType,
}

pub fn start_visualize_session(session_id: i64, session: Arc<RwLock<Session>>, scene: Scene,
                               updates: Arc<UpdatesQueue>, messages: Arc<Mutex<VecDeque<Message>>>,
                               map_db: Arc<Mutex<dyn MapDb + Send>>, config: VisualizationConfig) -> JoinHandle<()> {
    spawn(move || visualize_session(session_id, session, scene.nodes(), updates, messages, map_db, config))
}

fn visualize_session(session_id: i64, session: Arc<RwLock<Session>>,
                     layers: Arc<Mutex<BTreeMap<usize, Arc<Mutex<Node>>>>>,
                     updates: Arc<UpdatesQueue>, messages: Arc<Mutex<VecDeque<Message>>>,
                     map_db: Arc<Mutex<dyn MapDb + Send>>, config: VisualizationConfig) {
    let opengl = OpenGL::V4_5;
    let settings = WindowSettings::new(format!("Session {}", session_id), [1920, 1080])
        .graphics_api(opengl)
        .exit_on_esc(true);
    match config.window_type {
        WindowType::Glutin => match settings.build::<GlutinWindow>() {
            Ok(window) => visualize_loop(window, opengl, session_id, session, layers, updates, messages, map_db),
            Err(e) => error!("Failed to create visualization glutin window: {}", e),
        }
        WindowType::SDL2 => match settings.build::<Sdl2Window>() {
            Ok(window) => visualize_loop(window, opengl, session_id, session, layers, updates, messages, map_db),
            Err(e) => error!("Failed to create visualization SDL2 window: {}", e),
        }
    }
}

fn visualize_loop<W>(mut window: W, opengl: OpenGL, session_id: i64, session: Arc<RwLock<Session>>,
                     layers: Arc<Mutex<BTreeMap<usize, Arc<Mutex<Node>>>>>,
                     updates: Arc<UpdatesQueue>, messages: Arc<Mutex<VecDeque<Message>>>,
                     map_db: Arc<Mutex<dyn MapDb + Send>>) where W: Window {
    let mut events = Events::new(EventSettings::new().ups(60));
    let mut visualizer = Visualizer::new(opengl, session_id, session, updates, messages, map_db);

    while let Some(e) = events.next(&mut window) {
        if let Some(args) = e.render_args() {
            visualizer.render(args, &layers);
        }

        if let Some(args) = e.update_args() {
            visualizer.update(args);
        }

        if let Some(args) = e.press_args() {
            visualizer.press(args);
        }

        if let Some(args) = e.release_args() {
            visualizer.release(args);
        }

        if let Some(args) = e.mouse_scroll_args() {
            visualizer.mouse_scroll(args);
        }

        if let Some(args) = e.mouse_relative_args() {
            visualizer.mouse_relative(args);
        }
    }
}

struct Visualizer<'a> {
    gl: GlGraphics,
    glyphs: RefCell<GlyphCache<'a>>,
    session_id: i64,
    session: Arc<RwLock<Session>>,
    updates: Arc<UpdatesQueue>,
    messages: Arc<Mutex<VecDeque<Message>>>,
    map_db: Arc<Mutex<dyn MapDb + Send>>,
    frame_number: usize,
    fps: FpsMovingAverage,
    render_duration: DurationMovingAverage,
    update_duration: DurationMovingAverage,
    nodes: usize,
    scale: f64,
    shift: Vec2f,
    left_mouse_button_pushed: bool,
    last_player_segment_id: Option<i64>,
    last_world_revision: Option<u64>,
    world_scene: WorldScene,
    map_db_scene: MapDbScene,
    world_node: RefCell<Node>,
    debug_node: RefCell<Node>,
    map_db_node: RefCell<Node>,
}

impl Visualizer<'_> {
    fn new(opengl: OpenGL, session_id: i64, session: Arc<RwLock<Session>>,
           updates: Arc<UpdatesQueue>, messages: Arc<Mutex<VecDeque<Message>>>,
           map_db: Arc<Mutex<dyn MapDb + Send>>) -> Self {
        Self {
            gl: GlGraphics::new(opengl),
            glyphs: RefCell::new(GlyphCache::new(
                "fonts/UbuntuMono-R.ttf",
                (),
                TextureSettings::new().filter(Filter::Linear),
            ).expect("Could not load font")),
            session_id,
            session,
            updates,
            messages,
            map_db,
            frame_number: 0,
            fps: FpsMovingAverage::new(100, Duration::from_secs(1)),
            render_duration: DurationMovingAverage::new(100, Duration::from_secs(1)),
            update_duration: DurationMovingAverage::new(100, Duration::from_secs(1)),
            nodes: 0,
            scale: 1.0,
            shift: Vec2f::zero(),
            left_mouse_button_pushed: false,
            last_player_segment_id: None,
            last_world_revision: None,
            world_scene: WorldScene::default(),
            map_db_scene: MapDbScene::default(),
            world_node: RefCell::new(Node::Empty),
            debug_node: RefCell::new(Node::Empty),
            map_db_node: RefCell::new(Node::Empty),
        }
    }

    fn press(&mut self, args: Button) {
        if let Button::Mouse(MouseButton::Left) = args {
            self.left_mouse_button_pushed = true;
        }
    }

    fn release(&mut self, args: Button) {
        if let Button::Mouse(MouseButton::Left) = args {
            self.left_mouse_button_pushed = false;
        }
    }

    fn mouse_scroll(&mut self, args: [f64; 2]) {
        self.scale *= 1.0 + args[1] * 0.1;
    }

    fn mouse_relative(&mut self, args: [f64; 2]) {
        if self.left_mouse_button_pushed {
            self.shift += Vec2f::new(args[0], args[1]) / self.scale;
        }
    }

    fn render(&mut self, args: RenderArgs, nodes: &Arc<Mutex<BTreeMap<usize, Arc<Mutex<Node>>>>>) {
        let start = Instant::now();
        let world_node = self.world_node.borrow();
        let debug_node = self.debug_node.borrow();
        let map_db_node = self.map_db_node.borrow();
        let scale = self.scale;
        let shift = self.shift;
        let mut glyphs = self.glyphs.borrow_mut();
        let mut nodes_count = 0;
        self.gl.draw(args.viewport(), |base_context, g| {
            clear([0.0, 0.0, 0.0, 1.0], g);
            let context = &Context { base: &base_context, scale, shift };
            nodes_count += map_db_node.draw(&context, base_context.transform, glyphs.deref_mut(), g);
            nodes_count += world_node.draw(&context, base_context.transform, glyphs.deref_mut(), g);
            for layer in nodes.lock().unwrap().values() {
                nodes_count += layer.lock().unwrap().draw(&context, base_context.transform, glyphs.deref_mut(), g);
            }
            nodes_count += debug_node.draw(&context, base_context.transform, glyphs.deref_mut(), g);
        });
        let finish = Instant::now();
        self.render_duration.add(finish - start);
        self.fps.add(finish);
        self.nodes = nodes_count;
    }

    fn update(&mut self, _args: UpdateArgs) {
        let start = Instant::now();
        let mut debug_text = Vec::new();
        self.frame_number += 1;
        debug_text.push(format!("session: {}", self.session_id));
        debug_text.push(format!("frame: {}", self.frame_number));
        debug_text.push(format!("fps: {}", self.fps.get()));
        debug_text.push(format!("render duration: {}", self.render_duration.get()));
        debug_text.push(format!("update duration: {}", self.update_duration.get()));
        debug_text.push(format!("nodes: {}", self.nodes));
        debug_text.push(format!("updates: {}", count_updates(&self.updates)));
        debug_text.push(format!("messages: {}", self.messages.lock().unwrap().len()));
        if let Some(world) = self.session.read().unwrap().get_player_world() {
            if self.last_player_segment_id != Some(world.player_segment_id()) {
                self.shift = -world.player_position();
                self.last_player_segment_id = Some(world.player_segment_id());
            }
            if self.last_world_revision != Some(world.revision()) {
                self.world_node = RefCell::new(self.world_scene.make_node(&world));
                self.last_world_revision = Some(world.revision());
            }
            self.map_db_node = RefCell::new(self.map_db_scene.make_node(&self.map_db, &world));
            debug_text.push(format!("revision: {}", world.revision()));
            debug_text.push(format!("local grids: {}", self.world_scene.grids.len()));
            debug_text.push(format!("db grids: {}", self.map_db_scene.grids.len()));
            debug_text.push(format!("objects: {}", world.objects_len()));
            debug_text.push(format!("player segment id: {}", world.player_segment_id()));
            debug_text.push(format!("player grid id: {:?}", world.player_grid_id()));
            debug_text.push(format!("player position: {:?}", world.player_position()));
            debug_text.push(format!("player object id: {:?}", world.player_object_id()));
            debug_text.push(format!("player stuck: {:?}", world.is_player_stuck()));
        } else {
            debug_text.push(format!("world is not configured"));
            self.last_player_segment_id = None;
            self.last_world_revision = None;
            self.shift = Vec2f::zero();
        }
        self.debug_node = RefCell::new(Node::from(DebugTextNode {
            value: Text::new_color([1.0, 0.9, 0.9, 1.0], 14),
            background: Rectangle::new([0.2, 0.2, 0.8, 0.6]),
            lines: debug_text,
            transform: identity(),
            margin: 4,
        }));
        let finish = Instant::now();
        self.update_duration.add(finish - start);
    }
}

fn make_rgba_color_u8(value: i32) -> [u8; 4] {
    [
        get_color_component(value, 2),
        get_color_component(value, 1),
        get_color_component(value, 0),
        get_color_component(value, 3),
    ]
}

fn make_rgba_color_f32(value: i32) -> [f32; 4] {
    [
        get_color_component(value, 2) as f32 / 255.0,
        get_color_component(value, 1) as f32 / 255.0,
        get_color_component(value, 0) as f32 / 255.0,
        get_color_component(value, 3) as f32 / 255.0,
    ]
}

fn get_color_component(value: i32, number: i32) -> u8 {
    ((value >> (8 * number)) & std::u8::MAX as i32) as u8
}

#[derive(Default)]
struct WorldScene {
    grids: HashMap<i64, GridTexture>,
}

struct GridTexture {
    revision: i64,
    value: Arc<Mutex<Texture>>,
}

impl WorldScene {
    fn make_node(&mut self, world: &PlayerWorld) -> Node {
        let mut nodes: Vec<Node> = Vec::new();
        for grid in world.iter_grids().filter(|grid| grid.segment_id == world.player_segment_id()) {
            add_grid_node(grid, Vec2i::zero(), world, &mut self.grids, &mut nodes);
        }
        for object in world.iter_objects() {
            nodes.push(Node::from(EllipseNode {
                value: Ellipse::new([0.1, 0.1, 0.1, 0.9]),
                rectangle: centered_square(0.0, 0.0, TILE_SIZE),
                transform: identity().trans(object.position.x(), object.position.y()),
            }));
            let font_size = 14;
            let text_position = object.position + Vec2f::new(TILE_SIZE, -TILE_SIZE) / 2.0;
            nodes.push(Node::from(TextNode {
                value: Text::new_color([0.0, 0.0, 0.0, 1.0], font_size),
                text: format!("{}", object.id),
                transform: identity()
                    .trans(text_position.x(), text_position.y())
                    .scale(0.5, 0.5),
            }));
            if let Some(name) = object.name.as_ref() {
                let name_position = text_position - Vec2f::only_y(font_size as f64 / 2.0 + 2.0);
                nodes.push(Node::from(TextNode {
                    value: Text::new_color([0.0, 0.0, 0.0, 1.0], font_size),
                    text: name.clone(),
                    transform: identity()
                        .trans(name_position.x(), name_position.y())
                        .scale(0.5, 0.5),
                }));
            }
        }
        Node::from(MapTransformBoxNode {
            node: Box::new(Node::from(CompositeVecNode { nodes })),
        })
    }
}

#[derive(Default)]
struct MapDbScene {
    grids: HashMap<i64, GridTexture>,
}

impl MapDbScene {
    fn make_node(&mut self, map_db: &Arc<Mutex<dyn MapDb + Send>>, world: &PlayerWorld) -> Node {
        let mut nodes: Vec<Node> = Vec::new();
        let locked_map_db = map_db.lock().unwrap();
        if let Some((shift, grid_ids)) = locked_map_db.get_grid_by_id(world.player_segment_id())
            .and_then(|grid| {
                let locked_grid = grid.lock().unwrap();
                world.get_grid_by_id(locked_grid.id)
                    .map(|world_grid| (
                        world_grid.position - locked_grid.position,
                        locked_map_db.get_grid_ids_by_segment_id(locked_grid.segment_id),
                    ))
            }) {
            for grid_id in grid_ids.into_iter() {
                if world.get_grid_by_id(grid_id).is_none() {
                    if let Some(grid) = locked_map_db.get_grid_by_id(grid_id) {
                        let locked = grid.lock().unwrap();
                        add_grid_node(locked.deref(), shift, world, &mut self.grids, &mut nodes);
                    }
                }
            }
        }
        Node::from(MapTransformBoxNode {
            node: Box::new(Node::from(CompositeVecNode { nodes })),
        })
    }
}

fn add_grid_node(grid: &Grid, shift: Vec2i, world: &PlayerWorld, grids: &mut HashMap<i64, GridTexture>,
                 nodes: &mut Vec<Node>) {
    let cached = grids.entry(grid.id)
        .or_insert_with(|| make_grid_texture(grid, world));
    if cached.revision != grid.revision {
        *cached = make_grid_texture(grid, world);
    }
    let grid_position = grid_pos_to_pos(grid.position + shift);
    nodes.push(Node::from(ImageNode {
        value: Image::new().rect(square(0.0, 0.0, GRID_SIZE as f64 * TILE_SIZE)),
        texture: cached.value.clone(),
        transform: identity().trans(grid_position.x(), grid_position.y()),
    }));
    let areas = make_areas(grid);
    for area in areas.into_iter() {
        let color: [f32; 4] = world.get_tile_by_id(area.tile)
            .map(|tile| make_rgba_color_f32(tile.color))
            .map(|color| [color[0] / 2.0, color[1] / 2.0, color[2] / 2.0, 0.9])
            .unwrap_or([0.5, 0.5, 0.5, 0.9]);
        for i in 0..area.border.len() {
            let current = Vec2f::from(area.border[i]) * TILE_SIZE;
            let next = Vec2f::from(area.border[(i + 1) % area.border.len()]) * TILE_SIZE;
            nodes.push(Node::from(FixedScaleLineNode {
                value: Line::new(color, 3.0),
                line: [current.x() as f64, current.y() as f64, next.x() as f64, next.y() as f64],
                transform: identity().trans(grid_position.x(), grid_position.y()),
            }))
        }
    }
}

fn make_grid_texture(grid: &Grid, world: &PlayerWorld) -> GridTexture {
    let mut image = RgbaImage::new(GRID_SIZE as u32, GRID_SIZE as u32);
    for (index, tile_id) in grid.tiles.iter().enumerate() {
        let position = tile_index_to_tile_pos(index);
        let color = world.get_tile_by_id(*tile_id)
            .map(|tile| make_rgba_color_u8(tile.color))
            .unwrap_or([255, 255, 255, 255]);
        image.put_pixel(position.x() as u32, position.y() as u32, Rgba(color));
    }
    GridTexture {
        revision: grid.revision,
        value: Arc::new(Mutex::new(Texture::from_image(&image, &TextureSettings::new().filter(Filter::Nearest)))),
    }
}

pub struct FpsMovingAverage {
    max_frames: usize,
    max_interval: Duration,
    times: VecDeque<Instant>,
    sum_duration: Duration,
}

impl FpsMovingAverage {
    pub fn new(max_frames: usize, max_interval: Duration) -> Self {
        assert!(max_frames >= 3);
        Self {
            max_frames,
            max_interval,
            times: VecDeque::new(),
            sum_duration: Duration::new(0, 0),
        }
    }

    pub fn add(&mut self, time: Instant) {
        if self.times.len() >= self.max_frames
            || (self.times.len() >= 3 && self.sum_duration >= self.max_interval) {
            if let Some(removed) = self.times.pop_front() {
                if let Some(first) = self.times.front() {
                    self.sum_duration -= *first - removed;
                }
            }
        }
        if let Some(last) = self.times.back() {
            self.sum_duration += time - *last;
        }
        self.times.push_back(time);
    }

    pub fn get(&self) -> f64 {
        if self.times.len() >= 2 {
            (self.times.len() - 1) as f64 / self.sum_duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

pub struct DurationMovingAverage {
    max_frames: usize,
    max_interval: Duration,
    durations: VecDeque<Duration>,
    sum_duration: Duration,
}

impl DurationMovingAverage {
    pub fn new(max_frames: usize, max_interval: Duration) -> Self {
        assert!(max_frames >= 2);
        Self {
            max_frames,
            max_interval,
            durations: VecDeque::new(),
            sum_duration: Duration::new(0, 0),
        }
    }

    pub fn add(&mut self, duration: Duration) {
        if self.durations.len() >= self.max_frames
            || (self.durations.len() >= 2 && self.sum_duration >= self.max_interval) {
            if let Some(removed) = self.durations.pop_front() {
                self.sum_duration -= removed;
            }
        }
        self.durations.push_back(duration);
        self.sum_duration += duration;
    }

    pub fn get(&self) -> f64 {
        if self.durations.len() >= 1 {
            self.sum_duration.as_secs_f64() / self.durations.len() as f64
        } else {
            0.0
        }
    }
}
