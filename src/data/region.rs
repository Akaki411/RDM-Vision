#[derive(Debug, Clone, Copy)]
pub struct Point
{
    pub x: f32,
    pub y: f32
}

#[derive(Debug, Clone, Copy)]
pub struct Region
{
    pub score: f32,
    pub corners: [Point; 4]
}
