use crate::{common::PathId, utils};
use geo::{prelude::*, Coordinate, Line, LineString, Rect};
use itertools::Itertools as _;
use rustc_hash::{FxHashMap, FxHashSet};
use std::iter::FromIterator;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct TileId(Coordinate<i32>);

#[derive(Default, Debug)]
pub(super) struct Tiling {
    tiles: FxHashMap<TileId, FxHashMap<PathId, Vec<Line<i32>>>>,
    tile_ids: FxHashMap<PathId, Vec<TileId>>,
}

impl<'a> FromIterator<(PathId, &'a LineString<i32>)> for Tiling {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (PathId, &'a LineString<i32>)>,
    {
        let mut this = Self::default();
        for (path_id, coords) in iter {
            this.insert_path(path_id, coords);
        }
        this
    }
}

impl Tiling {
    const TILE_LEN: i32 = 128;

    fn tile_rect(id: TileId) -> Rect<i32> {
        Rect::new(
            utils::coord_map_scalars(id.0, |s| s * Self::TILE_LEN),
            utils::coord_map_scalars(id.0, |s| (s + 1) * Self::TILE_LEN),
        )
    }

    fn bounding_tile_ids(
        geo: impl BoundingRect<i32, Output = Rect<i32>> + Intersects<Rect<i32>>,
    ) -> impl Iterator<Item = TileId> {
        let rect = geo.bounding_rect();
        let xs = rect.min().x.div_euclid(Self::TILE_LEN)..=rect.max().x.div_euclid(Self::TILE_LEN);
        let ys = rect.min().y.div_euclid(Self::TILE_LEN)..=rect.max().y.div_euclid(Self::TILE_LEN);
        xs.cartesian_product(ys)
            .map(|(x, y)| TileId(Coordinate { x, y }))
            .filter(move |&id| geo.intersects(&Self::tile_rect(id)))
    }

    pub fn bounding_tile_items(
        &self,
        geo: impl BoundingRect<i32, Output = Rect<i32>> + Intersects<Rect<i32>>,
    ) -> impl Iterator<Item = (PathId, &[Line<i32>])> {
        Self::bounding_tile_ids(geo)
            .flat_map(move |tile_id| self.tiles.get(&tile_id).into_iter().flatten())
            .map(|(&path_id, lines)| (path_id, lines.as_slice()))
    }

    pub fn insert_path(&mut self, path_id: PathId, coords: &LineString<i32>) {
        let mut tile_ids = FxHashSet::default();
        for line in coords.lines() {
            for tile_id in Self::bounding_tile_ids(line) {
                self.tiles
                    .entry(tile_id)
                    .or_default()
                    .entry(path_id)
                    .or_default()
                    .push(line);
                tile_ids.insert(tile_id);
            }
        }
        let old = self
            .tile_ids
            .insert(path_id, tile_ids.into_iter().collect());
        assert!(old.is_none(), "path already exists");
    }

    pub fn remove_path(&mut self, path_id: PathId) {
        let tile_ids = self.tile_ids.remove(&path_id).expect("path not found");
        for tile_id in tile_ids {
            self.tiles
                .get_mut(&tile_id)
                .expect("tile not found")
                .retain(|id, _| id != &path_id);
        }
    }

    pub fn clear(&mut self) {
        self.tiles.clear();
        self.tile_ids.clear();
    }
}
