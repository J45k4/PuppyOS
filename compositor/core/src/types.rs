#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn offset(self, delta: Vector) -> Self {
        Self {
            x: self.x + delta.dx,
            y: self.y + delta.dy,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vector {
    pub dx: f32,
    pub dy: f32,
}

impl Vector {
    pub const fn new(dx: f32, dy: f32) -> Self {
        Self { dx, dy }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x < self.origin.x + self.size.width
            && point.y < self.origin.y + self.size.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub translation: Point,
    pub scale: Point,
    pub rotation: f32,
}

impl Transform {
    pub const fn identity() -> Self {
        Self {
            translation: Point::new(0.0, 0.0),
            scale: Point::new(1.0, 1.0),
            rotation: 0.0,
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub roots: Vec<NodeId>,
    pub nodes: Vec<Node>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, mut node: Node) -> NodeId {
        let id = NodeId(self.nodes.len());
        node.id = id;
        self.nodes.push(node);
        id
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct NodeId(pub usize);

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub transform: Transform,
    pub visible: bool,
    pub opacity: f32,
    pub clip: Option<Rect>,
    pub kind: NodeKind,
}

impl Node {
    pub fn new(kind: NodeKind) -> Self {
        Self {
            id: NodeId::default(),
            transform: Transform::identity(),
            visible: true,
            opacity: 1.0,
            clip: None,
            kind,
        }
    }

    pub fn group(children: Vec<NodeId>) -> Self {
        Self::new(NodeKind::Group { children })
    }

    pub fn rect(style: RectStyle) -> Self {
        Self::new(NodeKind::Rect(style))
    }
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    Group { children: Vec<NodeId> },
    Rect(RectStyle),
    Text(TextRun),
    Image(ImageRef),
    Surface(SurfaceRef),
}

#[derive(Clone, Debug)]
pub struct RectStyle {
    pub bounds: Rect,
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub text: String,
    pub position: Point,
    pub color: Color,
    pub size: f32,
    pub font: String,
}

#[derive(Clone, Debug)]
pub struct ImageRef {
    pub id: ResourceId,
    pub bounds: Rect,
}

#[derive(Clone, Debug)]
pub struct SurfaceRef {
    pub id: ResourceId,
    pub bounds: Rect,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceId(pub u64);
