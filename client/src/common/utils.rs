use arrayvec::ArrayVec;
use geo::{CoordNum, Coordinate, Rect};

pub fn coordinate_map<T, U>(coord: Coordinate<T>, mut f: impl FnMut(T) -> U) -> Coordinate<U>
where
    T: CoordNum,
    U: CoordNum,
{
    Coordinate {
        x: f(coord.x),
        y: f(coord.y),
    }
}

pub fn expand_rect<T>(rect: Rect<T>, delta: T) -> Rect<T>
where
    T: CoordNum,
{
    let delta = Coordinate { x: delta, y: delta };
    Rect::new(rect.min() - delta, rect.max() + delta)
}

pub struct RectDiff {
    pub removed: ArrayVec<Rect<i32>, 2>,
    pub added: ArrayVec<Rect<i32>, 2>,
}

pub fn rect_diff(
    origin: Coordinate<i32>,
    old_diag: Coordinate<i32>,
    new_diag: Coordinate<i32>,
) -> RectDiff {
    let mut removed = ArrayVec::new();
    let mut added = ArrayVec::new();
    if origin.x <= old_diag.x && old_diag.x <= new_diag.x
        || new_diag.x <= old_diag.x && old_diag.x <= origin.x
    {
        added.push(Rect::new(
            Coordinate {
                x: old_diag.x,
                y: origin.y,
            },
            new_diag,
        ));
    } else {
        removed.push(Rect::new(
            Coordinate {
                x: if origin.x <= old_diag.x {
                    origin.x.max(new_diag.x)
                } else {
                    origin.x.min(new_diag.x)
                },
                y: origin.y,
            },
            old_diag,
        ));
    }
    if origin.y <= old_diag.y && old_diag.y <= new_diag.y
        || new_diag.y <= old_diag.y && old_diag.y <= origin.y
    {
        added.push(Rect::new(
            Coordinate {
                x: origin.x,
                y: old_diag.y,
            },
            new_diag,
        ));
    } else {
        removed.push(Rect::new(
            Coordinate {
                x: origin.x,
                y: if origin.y <= old_diag.y {
                    origin.y.max(new_diag.y)
                } else {
                    origin.y.min(new_diag.y)
                },
            },
            old_diag,
        ));
    }
    RectDiff { removed, added }
}
