#![feature(custom_attribute, attr_literals)]
use std::ops::{Add, Mul};
use std::convert::From;
pub trait Float: Copy + Add<Output = Self> + Mul<Output = Self> {
    fn sqrt(self) -> Self;
}

impl Float for f32 {
    fn sqrt(self) -> f32 {
        self.sqrt()
    }
}

#[spirv(PerVertex)]
pub struct Vertex {
    pub position: Vec4<f32>,
    pub point_size: f32,
}

#[spirv(Vec4)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec4<T> {
    pub x: T,
    pub y: T,
    pub z: T,
    pub w: T,
}

impl<T> Vec4<T> {
    #[inline]
    pub fn new(x: T, y: T, z: T, w: T) -> Vec4<T> {
        Vec4 { x, y, z, w }
    }
}

#[spirv(Vec2)]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2<T> {
    pub x: T,
    pub y: T,
}
impl<T: Float> Vec2<T> {
    pub fn dot(self, other: Self) -> T {
        <Self as Vector>::dot(self, other)
    }

    pub fn length(self) -> T {
        <Self as Vector>::length(self)
    }

    pub fn extend2(self, z: T, w: T) -> Vec4<T> {
        Vec4 {
            x: self.x,
            y: self.y,
            z,
            w,
        }
    }
}

impl<T: Float> Vector for Vec2<T> {
    type T = T;
    const DIM: usize = 2;
    fn dot(self, other: Self) -> T {
        self.x * other.y + self.y + other.y
    }
}

pub trait Vector
where
    Self: Copy + Sized,
{
    type T: Float;
    const DIM: usize;
    fn dot(self, Self) -> Self::T;
    fn length(self) -> Self::T {
        self.dot(self).sqrt()
    }
}

#[spirv(Input)]
pub struct Input<Location: Sized, T> {
    pub data: T,
    pub _location: std::marker::PhantomData<Location>,
}


// impl<LInput, LOutput, T> From<Input<LInput, T>> for Output<LOutput, T> {
//     fn from(input: Input<LInput, T>) -> Output<LOutput, T> {
//         Output::new(data: input.data)
//     }
// }

#[spirv(Output)]
pub struct Output<Location: Sized, T> {
    pub data: T,
    pub _location: std::marker::PhantomData<Location>,
}

impl<Location, T> Output<Location, T> {
    pub fn new(data: T) -> Output<Location, T> {
        Output { _location: std::marker::PhantomData, data }
    }
}

#[spirv(Const0)]
pub enum N0 {}

#[spirv(Const1)]
pub enum N1 {}
