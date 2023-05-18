#[derive(Clone)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone)]
pub enum Primitive {
    Line(Point, Point),
    Quad(Point, Point),
    Disk(Point, usize),
    Circle(Point, usize),
    Ellipse(Point, Point),
    BezierQuad(Point, Point, Point),
    BezierCubic(Point, Point, Point, Point),
}