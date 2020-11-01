use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use graphics::{Ellipse, Image, Line, Polygon, Rectangle, Transformed};
use graphics::character::CharacterCache;
use graphics::math::{Matrix2d, Vec2d};
use graphics::rectangle::rectangle_by_corners;
use graphics::text::Text;
use graphics::types;
use opengl_graphics::{GlGraphics, GlyphCache, Texture};

use crate::bot::math::as_score;
use crate::bot::vec2::Vec2f;

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

    pub fn nodes(&self) -> Arc<Mutex<BTreeMap<usize, Arc<Mutex<Node>>>>> {
        self.nodes.clone()
    }
}

pub struct Layer {
    id: usize,
    scene: Scene,
}

impl Layer {
    pub fn new(scene: Scene, node: Arc<Mutex<Node>>) -> Self {
        Self {
            id: scene.add_node(node),
            scene,
        }
    }
}

impl Drop for Layer {
    fn drop(&mut self) {
        self.scene.remove_node(self.id);
    }
}

pub struct Context<'a> {
    pub base: &'a graphics::Context,
    pub scale: f64,
    pub shift: Vec2f,
}

pub enum Node {
    Empty,
    CompositeVec(CompositeVecNode),
    CompositeBTreeMap(CompositeBTreeMapNode),
    MapTransformBox(MapTransformBoxNode),
    MapTransformArc(MapTransformArcNode),
    MapTransformVec(MapTransformVecNode),
    DebugText(DebugTextNode),
    Rectangle(RectangleNode),
    Ellipse(EllipseNode),
    Text(TextNode),
    FixedScaleLine(FixedScaleLineNode),
    Arrow(ArrowNode),
    Image(ImageNode),
    Polygon(PolygonNode),
    Triangle(TriangleNode),
}

impl Node {
    pub fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
        match self {
            Node::Empty => 0,
            Node::CompositeVec(v) => v.draw(context, transform, cache, g),
            Node::CompositeBTreeMap(v) => v.draw(context, transform, cache, g),
            Node::MapTransformBox(v) => v.draw(context, transform, cache, g),
            Node::MapTransformArc(v) => v.draw(context, transform, cache, g),
            Node::MapTransformVec(v) => v.draw(context, transform, cache, g),
            Node::DebugText(v) => v.draw(context, transform, cache, g),
            Node::Rectangle(v) => v.draw(context, transform, g),
            Node::Ellipse(v) => v.draw(context, transform, g),
            Node::Text(v) => v.draw(context, transform, cache, g),
            Node::FixedScaleLine(v) => v.draw(context, transform, g),
            Node::Arrow(v) => v.draw(context, transform, g),
            Node::Image(v) => v.draw(context, transform, g),
            Node::Polygon(v) => v.draw(context, transform, g),
            Node::Triangle(v) => v.draw(context, transform, g),
        }
    }
}

macro_rules! node_from_impl {
    ($type: ty, $variant: tt) => {
        impl From<$type> for Node {
            fn from(value: $type) -> Self {
                Node::$variant(value)
            }
        }
    }
}

node_from_impl! { CompositeVecNode, CompositeVec }
node_from_impl! { CompositeBTreeMapNode, CompositeBTreeMap }
node_from_impl! { MapTransformBoxNode, MapTransformBox }
node_from_impl! { MapTransformArcNode, MapTransformArc }
node_from_impl! { MapTransformVecNode, MapTransformVec }
node_from_impl! { DebugTextNode, DebugText }
node_from_impl! { RectangleNode, Rectangle }
node_from_impl! { EllipseNode, Ellipse }
node_from_impl! { TextNode, Text }
node_from_impl! { FixedScaleLineNode, FixedScaleLine }
node_from_impl! { ArrowNode, Arrow }
node_from_impl! { ImageNode, Image }
node_from_impl! { PolygonNode, Polygon }
node_from_impl! { TriangleNode, Triangle }

#[derive(Default)]
pub struct CompositeVecNode {
    pub nodes: Vec<Node>,
}

impl CompositeVecNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
        self.nodes.iter().map(|node| node.draw(context, transform, cache, g)).sum()
    }
}

#[derive(Default)]
pub struct CompositeBTreeMapNode {
    pub nodes: BTreeMap<usize, Node>,
}

impl CompositeBTreeMapNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
        self.nodes.values().map(|node| node.draw(context, transform, cache, g)).sum()
    }
}

pub struct MapTransformBoxNode {
    pub node: Box<Node>,
}

pub struct MapTransformArcNode {
    pub node: Arc<Mutex<Node>>,
}

pub struct MapTransformVecNode {
    pub nodes: Vec<Node>,
}

trait AsMapTransformNode {
    fn with_node<F: FnMut(&Node) -> usize>(&self, f: F) -> usize;

    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
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
                )
            })
        } else {
            0
        }
    }
}

impl AsMapTransformNode for MapTransformBoxNode {
    fn with_node<F: FnMut(&Node) -> usize>(&self, mut f: F) -> usize {
        f(self.node.deref())
    }
}

impl AsMapTransformNode for MapTransformArcNode {
    fn with_node<F: FnMut(&Node) -> usize>(&self, mut f: F) -> usize {
        f(self.node.lock().unwrap().deref())
    }
}

impl AsMapTransformNode for MapTransformVecNode {
    fn with_node<F: FnMut(&Node) -> usize>(&self, f: F) -> usize {
        self.nodes.iter().map(f).sum()
    }
}

impl MapTransformBoxNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
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
            )
        } else {
            0
        }
    }
}

pub struct DebugTextNode {
    pub value: Text,
    pub background: Rectangle,
    pub lines: Vec<String>,
    pub transform: Matrix2d,
    pub margin: u32,
}

impl DebugTextNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
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
            let mut count = 1;
            for (n, line) in self.lines.iter().enumerate() {
                self.value.draw(
                    line.as_str(),
                    cache,
                    &context.base.draw_state,
                    transform.trans(0.0, ((self.value.font_size + self.margin) * (n + 1) as u32) as f64),
                    g,
                ).unwrap();
                count += 1;
            }
            count
        } else {
            0
        }
    }
}

pub struct RectangleNode {
    pub value: Rectangle,
    pub rectangle: types::Rectangle,
    pub transform: Matrix2d,
}

impl RectangleNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        self.value.draw(
            self.rectangle,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub struct EllipseNode {
    pub value: Ellipse,
    pub rectangle: types::Rectangle,
    pub transform: Matrix2d,
}

impl EllipseNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        self.value.draw(
            self.rectangle,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub struct TextNode {
    pub value: Text,
    pub text: String,
    pub transform: Matrix2d,
}

impl TextNode {
    fn draw(&self, context: &Context, transform: Matrix2d, cache: &mut GlyphCache, g: &mut GlGraphics) -> usize {
        self.value.draw(
            self.text.as_str(),
            cache,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        ).unwrap();
        1
    }
}

pub struct FixedScaleLineNode {
    pub value: Line,
    pub line: types::Line,
    pub transform: Matrix2d,
}

impl FixedScaleLineNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        Line {
            color: self.value.color,
            radius: self.value.radius / context.scale,
            shape: self.value.shape,
        }.draw(
            self.line,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub struct ArrowNode {
    pub value: Line,
    pub line: types::Line,
    pub head_size: f64,
    pub transform: Matrix2d,
}

impl ArrowNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        self.value.draw_arrow(
            self.line,
            self.head_size,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub struct ImageNode {
    pub value: Image,
    pub texture: Arc<Mutex<Texture>>,
    pub transform: Matrix2d,
}

impl ImageNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        self.value.draw(
            self.texture.lock().unwrap().deref(),
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub struct PolygonNode {
    pub value: Polygon,
    pub polygon: Vec<Vec2d>,
    pub transform: Matrix2d,
}

impl PolygonNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        self.value.draw(
            self.polygon.as_slice(),
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub struct TriangleNode {
    pub value: Polygon,
    pub triangle: types::Triangle,
    pub transform: Matrix2d,
}

impl TriangleNode {
    fn draw(&self, context: &Context, transform: Matrix2d, g: &mut GlGraphics) -> usize {
        self.value.draw(
            &self.triangle,
            &context.base.draw_state,
            transform.append_transform(self.transform),
            g,
        );
        1
    }
}

pub fn insert_to_composite_node_btree_map(target: &Arc<Mutex<Node>>, key: usize, node: Node) {
    match target.lock().unwrap().deref_mut() {
        Node::CompositeBTreeMap(ref mut v) => {
            v.nodes.insert(key, node);
        }
        _ => (),
    }
}

pub fn remove_from_composite_node_btree_map(target: &Arc<Mutex<Node>>, key: usize) {
    match target.lock().unwrap().deref_mut() {
        Node::CompositeBTreeMap(ref mut v) => {
            v.nodes.remove(&key);
        }
        _ => (),
    }
}
