use iced::Rectangle;

const MIN_RATIO: f32 = 0.05;
pub const DIVIDER: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub enum PaneNode {
    Leaf(u64),
    Split {
        axis: Axis,
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    pub fn leaves(&self) -> Vec<u64> {
        let mut out = Vec::new();
        self.collect_leaves(&mut out);
        out
    }

    fn collect_leaves(&self, out: &mut Vec<u64>) {
        match self {
            Self::Leaf(id) => out.push(*id),
            Self::Split { first, second, .. } => {
                first.collect_leaves(out);
                second.collect_leaves(out);
            }
        }
    }

    pub fn regions(&self, bounds: Rectangle) -> Vec<(u64, Rectangle)> {
        let mut out = Vec::new();
        self.collect_regions(bounds, &mut out);
        out
    }

    fn collect_regions(&self, bounds: Rectangle, out: &mut Vec<(u64, Rectangle)>) {
        match self {
            Self::Leaf(id) => out.push((*id, bounds)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (a, b) = split_rect(bounds, *axis, *ratio);
                first.collect_regions(a, out);
                second.collect_regions(b, out);
            }
        }
    }

    pub fn split(&mut self, target: u64, axis: Axis, new_id: u64) -> bool {
        match self {
            Self::Leaf(id) if *id == target => {
                *self = Self::Split {
                    axis,
                    ratio: 0.5,
                    first: Box::new(Self::Leaf(target)),
                    second: Box::new(Self::Leaf(new_id)),
                };
                true
            }
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                first.split(target, axis, new_id) || second.split(target, axis, new_id)
            }
        }
    }

    pub fn remove(&mut self, target: u64) -> bool {
        match self {
            Self::Leaf(_) => false,
            Self::Split { first, second, .. } => {
                if matches!(**first, Self::Leaf(id) if id == target) {
                    *self = (**second).clone();
                    return true;
                }
                if matches!(**second, Self::Leaf(id) if id == target) {
                    *self = (**first).clone();
                    return true;
                }
                first.remove(target) || second.remove(target)
            }
        }
    }
}

fn split_rect(bounds: Rectangle, axis: Axis, ratio: f32) -> (Rectangle, Rectangle) {
    let ratio = ratio.clamp(MIN_RATIO, 1.0 - MIN_RATIO);
    match axis {
        Axis::Vertical => {
            let usable = (bounds.width - DIVIDER).max(0.0);
            let first = (usable * ratio).max(0.0);
            (
                Rectangle {
                    width: first,
                    ..bounds
                },
                Rectangle {
                    x: bounds.x + first + DIVIDER,
                    width: (usable - first).max(0.0),
                    ..bounds
                },
            )
        }
        Axis::Horizontal => {
            let usable = (bounds.height - DIVIDER).max(0.0);
            let first = (usable * ratio).max(0.0);
            (
                Rectangle {
                    height: first,
                    ..bounds
                },
                Rectangle {
                    y: bounds.y + first + DIVIDER,
                    height: (usable - first).max(0.0),
                    ..bounds
                },
            )
        }
    }
}

pub fn pane_at(regions: &[(u64, Rectangle)], point: iced::Point) -> Option<u64> {
    regions
        .iter()
        .find(|(_, rect)| rect.contains(point))
        .map(|(id, _)| *id)
}

pub fn neighbour(regions: &[(u64, Rectangle)], from: u64, direction: Direction) -> Option<u64> {
    let origin = regions.iter().find(|(id, _)| *id == from)?.1;
    let center = |r: &Rectangle| (r.x + r.width / 2.0, r.y + r.height / 2.0);
    let (ox, oy) = center(&origin);

    regions
        .iter()
        .filter(|(id, _)| *id != from)
        .filter(|(_, r)| {
            let (cx, cy) = center(r);
            match direction {
                Direction::Left => cx < ox,
                Direction::Right => cx > ox,
                Direction::Up => cy < oy,
                Direction::Down => cy > oy,
            }
        })
        .filter(|(_, r)| match direction {
            Direction::Left | Direction::Right => {
                r.y < origin.y + origin.height && origin.y < r.y + r.height
            }
            Direction::Up | Direction::Down => {
                r.x < origin.x + origin.width && origin.x < r.x + r.width
            }
        })
        .min_by(|(_, a), (_, b)| {
            let (ax, ay) = center(a);
            let (bx, by) = center(b);
            let da = match direction {
                Direction::Left | Direction::Right => (ax - ox).abs(),
                Direction::Up | Direction::Down => (ay - oy).abs(),
            };
            let db = match direction {
                Direction::Left | Direction::Right => (bx - ox).abs(),
                Direction::Up | Direction::Down => (by - oy).abs(),
            };
            da.total_cmp(&db)
        })
        .map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 801.0,
            height: 601.0,
        }
    }

    fn ids(node: &PaneNode) -> Vec<u64> {
        node.leaves()
    }

    #[test]
    fn a_single_pane_fills_the_area() {
        let root = PaneNode::Leaf(1);
        let regions = root.regions(area());

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].0, 1);
        assert_eq!(regions[0].1, area());
    }

    #[test]
    fn a_vertical_split_halves_the_width_minus_the_divider() {
        let mut root = PaneNode::Leaf(1);
        assert!(root.split(1, Axis::Vertical, 2));

        let regions = root.regions(area());
        let (left, right) = (regions[0].1, regions[1].1);

        assert_eq!(left.width + right.width + DIVIDER, area().width);
        assert_eq!(left.height, area().height);
        assert_eq!(right.x, left.width + DIVIDER);
    }

    #[test]
    fn a_horizontal_split_halves_the_height() {
        let mut root = PaneNode::Leaf(1);
        assert!(root.split(1, Axis::Horizontal, 2));

        let regions = root.regions(area());
        let (top, bottom) = (regions[0].1, regions[1].1);

        assert_eq!(top.height + bottom.height + DIVIDER, area().height);
        assert_eq!(bottom.y, top.height + DIVIDER);
        assert_eq!(top.width, area().width);
    }

    #[test]
    fn splitting_a_nested_pane_only_touches_that_leaf() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        assert!(root.split(2, Axis::Horizontal, 3));

        assert_eq!(ids(&root), vec![1, 2, 3]);
        assert_eq!(root.regions(area()).len(), 3);
    }

    #[test]
    fn splitting_an_unknown_pane_changes_nothing() {
        let mut root = PaneNode::Leaf(1);
        assert!(!root.split(99, Axis::Vertical, 2));
        assert_eq!(ids(&root), vec![1]);
    }

    #[test]
    fn removing_a_pane_promotes_its_sibling() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        assert!(root.remove(1));

        assert_eq!(ids(&root), vec![2]);
        assert_eq!(root.regions(area())[0].1, area());
    }

    #[test]
    fn removing_a_nested_pane_keeps_the_rest_of_the_tree() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Horizontal, 3);
        assert!(root.remove(3));

        assert_eq!(ids(&root), vec![1, 2]);
    }

    #[test]
    fn the_last_pane_cannot_be_removed() {
        let mut root = PaneNode::Leaf(1);
        assert!(!root.remove(1));
        assert_eq!(ids(&root), vec![1]);
    }

    #[test]
    fn regions_never_overlap() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Horizontal, 3);
        root.split(1, Axis::Horizontal, 4);

        let regions = root.regions(area());
        for (i, (_, a)) in regions.iter().enumerate() {
            for (_, b) in regions.iter().skip(i + 1) {
                let overlap = a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height;
                assert!(!overlap, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn a_point_maps_to_the_pane_under_it() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let regions = root.regions(area());

        assert_eq!(pane_at(&regions, iced::Point::new(10.0, 10.0)), Some(1));
        assert_eq!(pane_at(&regions, iced::Point::new(790.0, 10.0)), Some(2));
        assert_eq!(pane_at(&regions, iced::Point::new(-5.0, 10.0)), None);
    }

    #[test]
    fn navigation_finds_the_pane_in_that_direction() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let regions = root.regions(area());

        assert_eq!(neighbour(&regions, 1, Direction::Right), Some(2));
        assert_eq!(neighbour(&regions, 2, Direction::Left), Some(1));
        assert_eq!(neighbour(&regions, 1, Direction::Up), None);
        assert_eq!(neighbour(&regions, 1, Direction::Left), None);
    }

    #[test]
    fn closing_panes_walks_back_to_a_single_leaf() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Horizontal, 3);

        assert!(root.remove(2));
        assert_eq!(ids(&root), vec![1, 3]);
        assert!(root.remove(3));
        assert_eq!(ids(&root), vec![1]);
        assert!(!root.remove(1));
    }

    #[test]
    fn navigation_crosses_a_nested_split() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Horizontal, 3);
        let regions = root.regions(area());

        assert_eq!(neighbour(&regions, 2, Direction::Down), Some(3));
        assert_eq!(neighbour(&regions, 3, Direction::Up), Some(2));
        assert_eq!(neighbour(&regions, 3, Direction::Left), Some(1));
    }

    #[test]
    fn a_vertical_split_gives_each_side_about_half_the_width() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let regions = root.regions(area());

        let half = (area().width - DIVIDER) / 2.0;
        assert!((regions[0].1.width - half).abs() < 1.0);
        assert!((regions[1].1.width - half).abs() < 1.0);
        assert!(regions[0].1.width < area().width * 0.6);
    }

    #[test]
    fn nested_splits_shrink_only_the_side_that_was_split() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let before = root.regions(area())[0].1;

        root.split(2, Axis::Horizontal, 3);
        let after = root.regions(area());

        assert_eq!(after[0].1.width, before.width);
        assert_eq!(after[0].1.height, before.height);
        assert!(after[1].1.height < before.height);
    }
}
