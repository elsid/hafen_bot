use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{JoinHandle, spawn};

use glutin_window::GlutinWindow;
use graphics::{clear, Ellipse, Line, Rectangle, Transformed};
use graphics::character::CharacterCache;
use graphics::math::{identity, Matrix2d};
use graphics::rectangle::{centered_square, rectangle_by_corners};
use graphics::text::Text;
use graphics::types;
use graphics::types::Color;
use opengl_graphics::{Filter, GlGraphics, GlyphCache, OpenGL, TextureSettings};
use piston::{EventLoop, RenderArgs, RenderEvent, UpdateArgs, UpdateEvent};
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

use crate::bot::common::as_score;
use crate::bot::map::{grid_pos_to_tile_pos, tile_index_to_tile_pos, tile_pos_to_pos, TILE_SIZE};
use crate::bot::Session;
use crate::bot::vec2::Vec2f;
use crate::bot::world::PlayerWorld;

#[derive(Clone)]
pub struct Scene {
    id_counter: Arc<AtomicUsize>,
    nodes: Arc<Mutex<BTreeMap<usize, Arc<Mutex<Node>>>>>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            id_counter: Arc::new(AtomicUsize::new(0)),
            nodes: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn add_node(&self, node: Arc<Mutex<Node>>) -> usize {
        let id = self.id_counter.deref().fetch_add(1, Ordering::Relaxed);
        self.nodes.lock().unwrap().insert(id, node);
        id
    }

    pub fn remove_node(&self, id: usize) {
        self.nodes.lock().unwrap().remove(&id);
    }
}

pub fn start_visualize_session(session_id: i64, session: Arc<RwLock<Session>>, scene: Scene) -> JoinHandle<()> {
    spawn(move || visualize_session(session_id, session, scene.nodes.clone()))
}

pub struct Layer {
    node: Arc<Mutex<Node>>,
    id: usize,
    scene: Scene,
}

impl Layer {
    pub fn new(scene: Scene, node: Arc<Mutex<Node>>) -> Self {
        Self {
            node: node.clone(),
            id: scene.add_node(node),
            scene,
        }
    }

    pub fn node(&self) -> Arc<Mutex<Node>> {
        self.node.clone()
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        self.scene.remove_node(self.id);
    }
}

fn visualize_session(session_id: i64, session: Arc<RwLock<Session>>, layers: Arc<Mutex<BTreeMap<usize, Arc<Mutex<Node>>>>>) {
    let opengl = OpenGL::V4_5;
    let mut window: GlutinWindow = WindowSettings::new(format!("Session {}", session_id), [1920, 1080])
        .graphics_api(opengl)
        .exit_on_esc(true)
        .build()
        .unwrap();
    let mut events = Events::new(EventSettings::new().ups(60));
    let mut visualizer = Visualizer::new(opengl, session_id, session);

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
    frame_number: usize,
    scale: f64,
    shift: Vec2f,
    left_mouse_button_pushed: bool,
    last_player_segment_id: Option<i64>,
    last_world_revision: Option<u64>,
    world_node: RefCell<Node>,
    debug_node: RefCell<Node>,
}

impl Visualizer<'_> {
    fn new(opengl: OpenGL, session_id: i64, session: Arc<RwLock<Session>>) -> Self {
        Self {
            gl: GlGraphics::new(opengl),
            glyphs: RefCell::new(GlyphCache::new(
                "fonts/UbuntuMono-R.ttf",
                (),
                TextureSettings::new().filter(Filter::Linear),
            ).expect("Could not load font")),
            session_id,
            session,
            frame_number: 0,
            scale: 1.0,
            shift: Vec2f::zero(),
            left_mouse_button_pushed: false,
            last_player_segment_id: None,
            last_world_revision: None,
            world_node: RefCell::new(Node::Empty),
            debug_node: RefCell::new(Node::Empty),
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
        let world_node = self.world_node.borrow();
        let debug_node = self.debug_node.borrow();
        let scale = self.scale;
        let shift = self.shift;
        let mut glyphs = self.glyphs.borrow_mut();
        self.gl.draw(args.viewport(), |base_context, g| {
            clear([0.0, 0.0, 0.0, 1.0], g);
            let context = &Context { base: &base_context, scale, shift };
            world_node.draw(&context, base_context.transform, glyphs.deref_mut(), g);
            for layer in nodes.lock().unwrap().values() {
                layer.lock().unwrap().draw(&context, base_context.transform, glyphs.deref_mut(), g);
            }
            debug_node.draw(&context, base_context.transform, glyphs.deref_mut(), g);
        });
    }

    fn update(&mut self, _args: UpdateArgs) {
        let mut debug_text = Vec::new();
        self.frame_number += 1;
        debug_text.push(format!("session: {}", self.session_id));
        debug_text.push(format!("frame: {}", self.frame_number));
        if let Some(world) = self.session.read().unwrap().get_player_world() {
            if self.last_player_segment_id != Some(world.player_segment_id()) {
                self.shift = -world.player_position();
                self.last_player_segment_id = Some(world.player_segment_id());
            }
            if self.last_world_revision != Some(world.revision()) {
                self.world_node = RefCell::new(make_world_node(&world));
            }
            debug_text.push(format!("revision: {}", world.revision()));
            debug_text.push(format!("player segment id: {}", world.player_segment_id()));
            debug_text.push(format!("player position: {:?}", world.player_position()));
        }
        self.debug_node = RefCell::new(Node::DebugText(DebugTextNode {
            value: Text::new_color([1.0, 0.9, 0.9, 1.0], 14),
            background: Rectangle::new([0.2, 0.2, 0.8, 0.6]),
            lines: debug_text,
            transform: identity(),
            margin: 4,
        }));
    }
}

fn make_rgba_color(value: i32) -> Color {
    [
        get_color_component(value, 2),
        get_color_component(value, 1),
        get_color_component(value, 0),
        get_color_component(value, 3),
    ]
}

fn get_color_component(value: i32, number: i32) -> f32 {
    ((value >> (8 * number)) & 0xff) as f32 / 255.0
}

struct Context<'a> {
    base: &'a graphics::Context,
    scale: f64,
    shift: Vec2f,
}

#[derive(Clone)]
pub enum Node {
    Empty,
    Composite(CompositeNode),
    MapTransformBox(MapTransformNodeBox),
    MapTransformArc(MapTransformNodeArc),
    DebugText(DebugTextNode),
    Rectangle(RectangleNode),
    Ellipse(EllipseNode),
    Text(TextNode),
    Line(LineNode),
    Arrow(ArrowNode),
}

impl Node {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) {
        match self {
            Node::Empty => (),
            Node::Composite(v) => v.draw(context, transform, cache, g),
            Node::MapTransformBox(v) => v.draw(context, transform, cache, g),
            Node::MapTransformArc(v) => v.draw(context, transform, cache, g),
            Node::DebugText(v) => v.draw(context, transform, cache, g),
            Node::Rectangle(v) => v.draw(context, transform, g),
            Node::Ellipse(v) => v.draw(context, transform, g),
            Node::Text(v) => v.draw(context, transform, cache, g),
            Node::Line(v) => v.draw(context, transform, g),
            Node::Arrow(v) => v.draw(context, transform, g),
        }
    }
}

#[derive(Clone)]
pub struct CompositeNode {
    pub nodes: Vec<Node>,
}

impl CompositeNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) {
        for node in self.nodes.iter() {
            node.draw(context, transform, cache, g);
        }
    }
}

#[derive(Clone)]
pub struct MapTransformNodeBox {
    pub node: Box<Node>,
}

#[derive(Clone)]
pub struct MapTransformNodeArc {
    pub node: Arc<Mutex<Node>>,
}

trait AsMapTransformNode {
    fn with_node<F: FnMut(&Node) -> ()>(&self, f: F);

    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) {
        if let Some(viewport) = context.base.viewport.as_ref() {
            let viewport_shift = Vec2f::new(viewport.window_size[0], viewport.window_size[1]) / 2.0;
            self.with_node(|node| {
                node.draw(
                    context,
                    transform
                        .trans(viewport_shift.x(), viewport_shift.y())
                        .scale(context.scale, context.scale)
                        .trans(context.shift.x(), context.shift.y()),
                    cache,
                    g,
                );
            });
        }
    }
}

impl AsMapTransformNode for MapTransformNodeBox {
    fn with_node<F: FnMut(&Node) -> ()>(&self, mut f: F) {
        f(self.node.deref());
    }
}

impl AsMapTransformNode for MapTransformNodeArc {
    fn with_node<F: FnMut(&Node) -> ()>(&self, mut f: F) {
        f(self.node.lock().unwrap().deref());
    }
}

impl MapTransformNodeBox {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) {
        if let Some(viewport) = context.base.viewport.as_ref() {
            let viewport_shift = Vec2f::new(viewport.window_size[0], viewport.window_size[1]) / 2.0;
            self.node.draw(
                context,
                transform
                    .trans(viewport_shift.x(), viewport_shift.y())
                    .scale(context.scale, context.scale)
                    .trans(context.shift.x(), context.shift.y()),
                cache,
                g,
            );
        }
    }
}

#[derive(Clone)]
pub struct DebugTextNode {
    pub value: Text,
    pub background: Rectangle,
    pub lines: Vec<String>,
    pub transform: Matrix2d,
    pub margin: u32,
}

impl DebugTextNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) {
        let max_width = self.lines.iter()
            .map(|line| cache.width(self.value.font_size, line.as_str()).unwrap())
            .max_by_key(|v| as_score(*v));
        if let Some(width) = max_width {
            let transform = transform.append_transform(self.transform);
            self.background.draw(
                rectangle_by_corners(
                    0.0,
                    0.0,
                    width as f64,
                    (self.lines.len() as f64 + 0.5) * (self.value.font_size + self.margin) as f64,
                ),
                &context.base.draw_state,
                transform,
                g,
            );
            for (n, line) in self.lines.iter().enumerate() {
                self.value.draw(
                    line.as_str(),
                    cache,
                    &context.base.draw_state,
                    transform.trans(0.0, ((self.value.font_size + self.margin) * (n + 1) as u32) as f64),
                    g,
                ).unwrap();
            }
        }
    }
}

#[derive(Clone)]
pub struct RectangleNode {
    pub value: Rectangle,
    pub rectangle: types::Rectangle,
    pub transform: Matrix2d,
}

impl RectangleNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) {
        self.value.draw(
            self.rectangle,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
    }
}

#[derive(Clone)]
pub struct EllipseNode {
    pub value: Ellipse,
    pub rectangle: types::Rectangle,
    pub transform: Matrix2d,
}

impl EllipseNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) {
        self.value.draw(
            self.rectangle,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
    }
}

#[derive(Clone)]
pub struct TextNode {
    pub value: Text,
    pub text: String,
    pub transform: Matrix2d,
}

impl TextNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) {
        self.value.draw(
            self.text.as_str(),
            cache,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        ).unwrap();
    }
}

#[derive(Clone)]
pub struct LineNode {
    pub value: Line,
    pub line: types::Line,
    pub transform: Matrix2d,
}

impl LineNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) {
        self.value.draw(
            self.line,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
    }
}

#[derive(Clone)]
pub struct ArrowNode {
    pub value: Line,
    pub line: types::Line,
    pub head_size: f64,
    pub transform: Matrix2d,
}

impl ArrowNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) {
        self.value.draw_arrow(
            self.line,
            self.head_size,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
    }
}

fn make_world_node(world: &PlayerWorld) -> Node {
    let mut world_nodes: Vec<Node> = Vec::new();
    for grid in world.iter_grids().filter(|grid| grid.segment_id == world.player_segment_id()) {
        for (index, tile_id) in grid.tiles.iter().enumerate() {
            let tile_position = tile_pos_to_pos(
                grid_pos_to_tile_pos(grid.position)
                    + tile_index_to_tile_pos(index)
            );
            let color = world.get_tile_by_id(*tile_id)
                .map(|tile| make_rgba_color(tile.color))
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            world_nodes.push(Node::Rectangle(RectangleNode {
                value: Rectangle::new(color),
                rectangle: centered_square(0.0, 0.0, TILE_SIZE / 2.0),
                transform: identity().trans(tile_position.x(), tile_position.y()),
            }));
        }
    }
    for object in world.iter_objects() {
        world_nodes.push(Node::Ellipse(EllipseNode {
            value: Ellipse::new([0.1, 0.1, 0.1, 0.9]),
            rectangle: centered_square(0.0, 0.0, TILE_SIZE / 2.0),
            transform: identity().trans(object.position.x(), object.position.y()),
        }));
        let font_size = 14;
        let text_position = object.position + Vec2f::new(TILE_SIZE, -TILE_SIZE) / 2.0;
        world_nodes.push(Node::Text(TextNode {
            value: Text::new_color([0.0, 0.0, 0.0, 1.0], font_size),
            text: format!("{}", object.id),
            transform: identity()
                .trans(text_position.x(), text_position.y())
                .scale(0.5, 0.5),
        }));
        if let Some(name) = object.name.as_ref() {
            let name_position = text_position - Vec2f::only_y(font_size as f64 / 2.0 + 2.0);
            world_nodes.push(Node::Text(TextNode {
                value: Text::new_color([0.0, 0.0, 0.0, 1.0], font_size),
                text: name.clone(),
                transform: identity()
                    .trans(name_position.x(), name_position.y())
                    .scale(0.5, 0.5),
            }));
        }
    }
    Node::MapTransformBox(MapTransformNodeBox {
        node: Box::new(Node::Composite(CompositeNode { nodes: world_nodes })),
    })
}

pub fn add_to_composite_node(target: &Arc<Mutex<Node>>, node: Node) {
    match target.lock().unwrap().deref_mut() {
        Node::Composite(ref mut v) => {
            v.nodes.push(node);
        }
        _ => (),
    }
}
