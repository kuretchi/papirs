use arrayvec::ArrayVec;
use geo::{map_coords::MapCoordsInplace as _, prelude::*, CoordNum, Coordinate, Rect};
use std::cmp::Ordering;

pub fn coord_map_scalars<T, U>(coord: Coordinate<T>, mut f: impl FnMut(T) -> U) -> Coordinate<U>
where
    T: CoordNum,
    U: CoordNum,
{
    Coordinate {
        x: f(coord.x),
        y: f(coord.y),
    }
}

pub trait MapScalars<T, U> {
    type Output;
    fn map_scalars(&self, f: impl FnMut(T) -> U + Copy) -> Self::Output;
}

impl<G, T, U> MapScalars<T, U> for G
where
    G: MapCoords<T, U>,
    T: CoordNum,
    U: CoordNum,
{
    type Output = G::Output;
    fn map_scalars(&self, f: impl FnMut(T) -> U + Copy) -> Self::Output {
        self.map_coords(|&c| coord_map_scalars(c.into(), f).x_y())
    }
}

pub fn expand_rect<T>(rect: Rect<T>, delta: T) -> Rect<T>
where
    T: CoordNum,
{
    let delta = Coordinate { x: delta, y: delta };
    Rect::new(rect.min() - delta, rect.max() + delta)
}

#[derive(Debug)]
pub struct RectDiff {
    pub removed: ArrayVec<Rect<i32>, 2>,
    pub added: ArrayVec<Rect<i32>, 2>,
}

/// Returns the "difference" between two rectangles with one common vertex.
///
/// Let _R₁_ and _R₂_ are rectangles which have least one common vertex.
/// A "removed part" _R₁_ \ _R₂_ and an "added part" _R₂_ \ _R₁_ can be represented as
/// an union of at most two rectangles, respectively, and they are what this function returns.
pub fn rect_diff(
    mut origin: Coordinate<i32>,
    mut old_diag: Coordinate<i32>,
    mut new_diag: Coordinate<i32>,
) -> RectDiff {
    let mut removed = ArrayVec::new();
    let mut added = ArrayVec::new();

    let flip_x = old_diag.x < origin.x;
    if flip_x {
        origin.x *= -1;
        old_diag.x *= -1;
        new_diag.x *= -1;
    }
    let flip_y = old_diag.y < origin.y;
    if flip_y {
        origin.y *= -1;
        old_diag.y *= -1;
        new_diag.y *= -1;
    }
    debug_assert!(origin.x <= old_diag.x);
    debug_assert!(origin.y <= old_diag.y);

    if new_diag.x <= origin.x || new_diag.y <= origin.y {
        removed.push(Rect::new(origin, old_diag));
        added.push(Rect::new(origin, new_diag));
    } else {
        match old_diag.x.cmp(&new_diag.x) {
            Ordering::Less => {
                added.push(Rect::new(
                    Coordinate {
                        x: old_diag.x,
                        y: origin.y,
                    },
                    new_diag,
                ));
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                removed.push(Rect::new(
                    Coordinate {
                        x: new_diag.x,
                        y: origin.y,
                    },
                    old_diag,
                ));
            }
        }
        match old_diag.y.cmp(&new_diag.y) {
            Ordering::Less => {
                added.push(Rect::new(
                    Coordinate {
                        x: origin.x,
                        y: old_diag.y,
                    },
                    new_diag,
                ));
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                removed.push(Rect::new(
                    Coordinate {
                        x: origin.x,
                        y: new_diag.y,
                    },
                    old_diag,
                ));
            }
        }
    }

    for rect in removed.iter_mut().chain(added.iter_mut()) {
        rect.map_coords_inplace(|&(x, y)| {
            (if flip_x { -x } else { x }, if flip_y { -y } else { y })
        });
    }

    RectDiff { removed, added }
}
