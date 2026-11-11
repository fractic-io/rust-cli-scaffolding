#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Corners {
    All,
    Only(Corner),
    Except(Corner),
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Corners {
    pub fn iter(&self) -> Vec<Corner> {
        match self {
            Corners::All => vec![
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ],
            Corners::Only(c) => vec![*c],
            Corners::Except(c) => vec![
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ]
            .into_iter()
            .filter(|&x| x != *c)
            .collect(),
            Corners::Top => vec![Corner::TopLeft, Corner::TopRight],
            Corners::Bottom => vec![Corner::BottomLeft, Corner::BottomRight],
            Corners::Left => vec![Corner::TopLeft, Corner::BottomLeft],
            Corners::Right => vec![Corner::TopRight, Corner::BottomRight],
        }
    }
}
