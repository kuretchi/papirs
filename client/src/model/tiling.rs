use crate::common::{utils, PathId};
use geo::{prelude::*, Coordinate, Line, LineString, Rect};
use itertools::Itertools as _;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct TileId(Coordinate<i32>);

#[derive(Default, Debug)]
pub(super) struct Tiling {
    tile_items: FxHashMap<TileId, FxHashMap<PathId, Vec<Line<i32>>>>,
    tile_ids: FxHashMap<PathId, Vec<TileId>>,
}

impl Tiling {
    const TILE_LEN: i32 = 100;

    fn tile_rect(id: TileId) -> Rect<i32> {
        Rect::new(
            utils::coordinate_map(id.0, |d| d * Self::TILE_LEN),
            utils::coordinate_map(id.0, |d| (d + 1) * Self::TILE_LEN),
        )
    }

    fn bounding_tile_ids(
        geo: impl BoundingRect<i32, Output = Rect<i32>> + Intersects<Rect<i32>>,
    ) -> impl Iterator<Item = TileId> {
        let rect = geo.bounding_rect();
        let xs = (rect.min().x.div_euclid(Self::TILE_LEN)
            ..=rect.max().x.div_euclid(Self::TILE_LEN))
            .dedup();
        let ys = (rect.min().y.div_euclid(Self::TILE_LEN)
            ..=rect.max().y.div_euclid(Self::TILE_LEN))
            .dedup();
        xs.cartesian_product(ys)
            .map(|(x, y)| TileId(Coordinate { x, y }))
            .filter(move |&id| geo.intersects(&Self::tile_rect(id)))
    }

    pub fn bounding_tile_items(
        &self,
        geo: impl BoundingRect<i32, Output = Rect<i32>> + Intersects<Rect<i32>>,
    ) -> impl Iterator<Item = (PathId, &[Line<i32>])> {
        Self::bounding_tile_ids(geo)
            .flat_map(move |tile_id| self.tile_items.get(&tile_id).into_iter().flatten())
            .map(|(&path_id, lines)| (path_id, lines.as_slice()))
    }

    pub fn insert_path(&mut self, path_id: PathId, lines: &LineString<i32>) {
        let mut tile_ids = FxHashSet::default();
        for line in lines.lines() {
            for tile_id in Self::bounding_tile_ids(line) {
                self.tile_items
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
            self.tile_items
                .get_mut(&tile_id)
                .expect("tile not found")
                .retain(|id, _| id != &path_id);
        }
    }

    pub fn clear(&mut self) {
        self.tile_items.clear();
        self.tile_ids.clear();
    }
}
