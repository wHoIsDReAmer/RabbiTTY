use iced::{Point, Rectangle, Size};

const MIN_RATIO: f32 = 0.05;
const GRAB_BEFORE: f32 = 2.0;
const GRAB_AFTER: f32 = 4.0;
pub const DIVIDER: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn for_rect(rect: Rectangle) -> Self {
        if rect.width >= rect.height {
            Self::Vertical
        } else {
            Self::Horizontal
        }
    }

    pub fn across(self, point: Point) -> f32 {
        match self {
            Self::Vertical => point.x,
            Self::Horizontal => point.y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Divider {
    pub leaf: u64,
    pub axis: Axis,
    pub rect: Rectangle,
}

impl Divider {
    pub fn position(&self) -> f32 {
        self.axis.across(self.rect.position())
    }

    fn grab_area(&self) -> Rectangle {
        let reach = GRAB_BEFORE + GRAB_AFTER;
        match self.axis {
            Axis::Vertical => Rectangle {
                x: self.rect.x - GRAB_BEFORE,
                width: self.rect.width + reach,
                ..self.rect
            },
            Axis::Horizontal => Rectangle {
                y: self.rect.y - GRAB_BEFORE,
                height: self.rect.height + reach,
                ..self.rect
            },
        }
    }
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

    pub fn regions(&self, bounds: Rectangle, min: Size) -> Vec<(u64, Rectangle)> {
        let mut out = Vec::new();
        self.collect_regions(bounds, min, &mut out);
        out
    }

    fn collect_regions(&self, bounds: Rectangle, min: Size, out: &mut Vec<(u64, Rectangle)>) {
        match self {
            Self::Leaf(id) => out.push((*id, bounds)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (a, b) = split_rect(bounds, *axis, *ratio, first, second, min);
                first.collect_regions(a, min, out);
                second.collect_regions(b, min, out);
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

    fn first_leaf(&self) -> u64 {
        match self {
            Self::Leaf(id) => *id,
            Self::Split { first, .. } => first.first_leaf(),
        }
    }

    pub fn dividers(&self, bounds: Rectangle, min: Size) -> Vec<Divider> {
        let mut out = Vec::new();
        self.collect_dividers(bounds, min, &mut out);
        out
    }

    fn collect_dividers(&self, bounds: Rectangle, min: Size, out: &mut Vec<Divider>) {
        let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        else {
            return;
        };
        let (a, b) = split_rect(bounds, *axis, *ratio, first, second, min);
        let rect = match axis {
            Axis::Vertical => Rectangle {
                x: a.x + a.width,
                width: DIVIDER,
                ..bounds
            },
            Axis::Horizontal => Rectangle {
                y: a.y + a.height,
                height: DIVIDER,
                ..bounds
            },
        };
        out.push(Divider {
            leaf: second.first_leaf(),
            axis: *axis,
            rect,
        });
        first.collect_dividers(a, min, out);
        second.collect_dividers(b, min, out);
    }

    pub fn move_divider(&mut self, leaf: u64, bounds: Rectangle, position: f32, min: Size) -> bool {
        let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        else {
            return false;
        };
        if second.first_leaf() != leaf {
            let (a, b) = split_rect(bounds, *axis, *ratio, first, second, min);
            return first.move_divider(leaf, a, position, min)
                || second.move_divider(leaf, b, position, min);
        }
        let (start, extent) = match axis {
            Axis::Vertical => (bounds.x, bounds.width),
            Axis::Horizontal => (bounds.y, bounds.height),
        };
        let usable = extent - DIVIDER;
        let lowest = first.min_extent(*axis, min).max(usable * MIN_RATIO);
        let highest = (usable - second.min_extent(*axis, min)).min(usable * (1.0 - MIN_RATIO));
        if lowest > highest {
            return false;
        }
        let moved = (position - start).clamp(lowest, highest) / usable;
        let changed = moved != *ratio;
        *ratio = moved;
        changed
    }

    fn min_extent(&self, along: Axis, min: Size) -> f32 {
        match self {
            Self::Leaf(_) => match along {
                Axis::Vertical => min.width,
                Axis::Horizontal => min.height,
            },
            Self::Split {
                axis,
                first,
                second,
                ..
            } => {
                let (a, b) = (first.min_extent(along, min), second.min_extent(along, min));
                if *axis == along {
                    a + b + DIVIDER
                } else {
                    a.max(b)
                }
            }
        }
    }
}

fn split_rect(
    bounds: Rectangle,
    axis: Axis,
    ratio: f32,
    first: &PaneNode,
    second: &PaneNode,
    min: Size,
) -> (Rectangle, Rectangle) {
    let ratio = ratio.clamp(MIN_RATIO, 1.0 - MIN_RATIO);
    let extent = match axis {
        Axis::Vertical => bounds.width,
        Axis::Horizontal => bounds.height,
    };
    let usable = (extent - DIVIDER).max(0.0);
    let (first_min, second_min) = (first.min_extent(axis, min), second.min_extent(axis, min));
    let mut cut = (usable * ratio).max(0.0);
    if first_min + second_min <= usable {
        cut = cut.clamp(first_min, usable - second_min);
    }
    match axis {
        Axis::Vertical => (
            Rectangle {
                width: cut,
                ..bounds
            },
            Rectangle {
                x: bounds.x + cut + DIVIDER,
                width: (usable - cut).max(0.0),
                ..bounds
            },
        ),
        Axis::Horizontal => (
            Rectangle {
                height: cut,
                ..bounds
            },
            Rectangle {
                y: bounds.y + cut + DIVIDER,
                height: (usable - cut).max(0.0),
                ..bounds
            },
        ),
    }
}

pub fn pane_at(regions: &[(u64, Rectangle)], point: iced::Point) -> Option<u64> {
    regions
        .iter()
        .find(|(_, rect)| rect.contains(point))
        .map(|(id, _)| *id)
}

pub fn divider_at(dividers: &[Divider], point: Point) -> Option<Divider> {
    dividers
        .iter()
        .copied()
        .find(|divider| divider.grab_area().contains(point))
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
        let regions = root.regions(area(), Size::ZERO);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].0, 1);
        assert_eq!(regions[0].1, area());
    }

    #[test]
    fn a_vertical_split_halves_the_width_minus_the_divider() {
        let mut root = PaneNode::Leaf(1);
        assert!(root.split(1, Axis::Vertical, 2));

        let regions = root.regions(area(), Size::ZERO);
        let (left, right) = (regions[0].1, regions[1].1);

        assert_eq!(left.width + right.width + DIVIDER, area().width);
        assert_eq!(left.height, area().height);
        assert_eq!(right.x, left.width + DIVIDER);
    }

    #[test]
    fn a_horizontal_split_halves_the_height() {
        let mut root = PaneNode::Leaf(1);
        assert!(root.split(1, Axis::Horizontal, 2));

        let regions = root.regions(area(), Size::ZERO);
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
        assert_eq!(root.regions(area(), Size::ZERO).len(), 3);
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
        assert_eq!(root.regions(area(), Size::ZERO)[0].1, area());
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

        let regions = root.regions(area(), Size::ZERO);
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
        let regions = root.regions(area(), Size::ZERO);

        assert_eq!(pane_at(&regions, iced::Point::new(10.0, 10.0)), Some(1));
        assert_eq!(pane_at(&regions, iced::Point::new(790.0, 10.0)), Some(2));
        assert_eq!(pane_at(&regions, iced::Point::new(-5.0, 10.0)), None);
    }

    #[test]
    fn navigation_finds_the_pane_in_that_direction() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let regions = root.regions(area(), Size::ZERO);

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
        let regions = root.regions(area(), Size::ZERO);

        assert_eq!(neighbour(&regions, 2, Direction::Down), Some(3));
        assert_eq!(neighbour(&regions, 3, Direction::Up), Some(2));
        assert_eq!(neighbour(&regions, 3, Direction::Left), Some(1));
    }

    #[test]
    fn a_vertical_split_gives_each_side_about_half_the_width() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let regions = root.regions(area(), Size::ZERO);

        let half = (area().width - DIVIDER) / 2.0;
        assert!((regions[0].1.width - half).abs() < 1.0);
        assert!((regions[1].1.width - half).abs() < 1.0);
        assert!(regions[0].1.width < area().width * 0.6);
    }

    #[test]
    fn nested_splits_shrink_only_the_side_that_was_split() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let before = root.regions(area(), Size::ZERO)[0].1;

        root.split(2, Axis::Horizontal, 3);
        let after = root.regions(area(), Size::ZERO);

        assert_eq!(after[0].1.width, before.width);
        assert_eq!(after[0].1.height, before.height);
        assert!(after[1].1.height < before.height);
    }

    #[test]
    fn auto_axis_splits_across_the_longer_side() {
        let wide = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 1200.0,
            height: 800.0,
        };
        let tall = Rectangle {
            width: 600.0,
            ..wide
        };
        assert_eq!(Axis::for_rect(wide), Axis::Vertical);
        assert_eq!(Axis::for_rect(tall), Axis::Horizontal);
    }

    #[test]
    fn repeated_auto_splits_alternate_direction() {
        let mut root = PaneNode::Leaf(1);
        for next in (2..).take(3) {
            let regions = root.regions(area(), Size::ZERO);
            let focused = regions.last().expect("region").1;
            root.split(next - 1, Axis::for_rect(focused), next);
        }

        let regions = root.regions(area(), Size::ZERO);
        assert_eq!(regions.len(), 4);
        // No pane should be a thin sliver: an always-vertical split of a
        // 801px area would leave the last pane around 100px wide.
        for (_, rect) in &regions {
            assert!(rect.width > 150.0, "{rect:?} is too narrow");
            assert!(rect.height > 100.0, "{rect:?} is too short");
        }
    }

    fn min() -> Size {
        Size::new(100.0, 50.0)
    }

    fn widths(node: &PaneNode) -> Vec<f32> {
        node.regions(area(), min())
            .iter()
            .map(|(_, rect)| rect.width.round())
            .collect()
    }

    #[test]
    fn each_split_has_one_divider_named_after_the_pane_that_follows_it() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Horizontal, 3);

        assert_eq!(
            root.dividers(area(), min()),
            vec![
                Divider {
                    leaf: 2,
                    axis: Axis::Vertical,
                    rect: Rectangle {
                        x: 400.0,
                        y: 0.0,
                        width: DIVIDER,
                        height: 601.0,
                    },
                },
                Divider {
                    leaf: 3,
                    axis: Axis::Horizontal,
                    rect: Rectangle {
                        x: 401.0,
                        y: 300.0,
                        width: 400.0,
                        height: DIVIDER,
                    },
                },
            ]
        );
    }

    #[test]
    fn a_divider_is_grabbed_from_a_few_pixels_around_it_and_no_further() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let dividers = root.dividers(area(), min());
        let grabbed = |x: f32| divider_at(&dividers, Point::new(x, 10.0)).map(|d| d.leaf);

        assert_eq!(grabbed(398.5), Some(2));
        assert_eq!(grabbed(404.5), Some(2));
        assert_eq!(
            grabbed(397.5),
            None,
            "the scrollbar keeps the rest of its width"
        );
        assert_eq!(grabbed(405.5), None);
    }

    #[test]
    fn moving_a_divider_resizes_only_the_two_sides_it_separates() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Vertical, 3);

        assert!(root.move_divider(3, area(), 700.0, min()));

        assert_eq!(widths(&root), vec![400.0, 299.0, 100.0]);
        let dividers = root.dividers(area(), min());
        assert_eq!(dividers[1].position().round(), 700.0);
    }

    #[test]
    fn a_horizontal_divider_follows_the_vertical_position() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Horizontal, 2);

        assert!(root.move_divider(2, area(), 150.0, min()));

        let regions = root.regions(area(), min());
        assert_eq!(regions[0].1.height.round(), 150.0);
        assert_eq!(regions[1].1.y.round(), 151.0);
    }

    #[test]
    fn a_divider_stops_where_a_pane_on_either_side_would_get_too_small() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);

        root.move_divider(2, area(), -50.0, min());
        assert_eq!(widths(&root), vec![100.0, 700.0]);

        root.move_divider(2, area(), 5000.0, min());
        assert_eq!(widths(&root), vec![700.0, 100.0]);
    }

    #[test]
    fn panes_nested_on_the_shrinking_side_each_keep_their_minimum() {
        let mut side_by_side = PaneNode::Leaf(1);
        side_by_side.split(1, Axis::Vertical, 2);
        side_by_side.split(2, Axis::Vertical, 3);
        side_by_side.move_divider(2, area(), 5000.0, min());
        assert_eq!(widths(&side_by_side), vec![599.0, 100.0, 100.0]);

        let mut stacked = PaneNode::Leaf(1);
        stacked.split(1, Axis::Vertical, 2);
        stacked.split(2, Axis::Horizontal, 3);
        stacked.move_divider(2, area(), 5000.0, min());
        assert_eq!(widths(&stacked), vec![700.0, 100.0, 100.0]);

        let mut uneven = PaneNode::Leaf(1);
        uneven.split(1, Axis::Vertical, 2);
        uneven.split(2, Axis::Vertical, 3);
        uneven.move_divider(3, area(), 501.0, min());
        uneven.move_divider(2, area(), 5000.0, min());
        assert_eq!(widths(&uneven), vec![599.0, 100.0, 100.0]);
        uneven.move_divider(2, area(), 400.0, min());
        assert_eq!(widths(&uneven), vec![400.0, 100.0, 299.0]);
    }

    #[test]
    fn a_divider_stays_put_when_there_is_no_room_or_its_pane_is_gone() {
        let cramped = Rectangle {
            width: 150.0,
            ..area()
        };
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        let before = root.regions(cramped, min());

        assert!(!root.move_divider(2, cramped, 120.0, min()));
        assert!(!root.move_divider(9, area(), 120.0, min()));

        assert_eq!(root.regions(cramped, min()), before);
    }

    #[test]
    fn a_pane_dragged_to_its_minimum_keeps_it_while_the_area_shrinks() {
        let mut side_by_side = PaneNode::Leaf(1);
        side_by_side.split(1, Axis::Vertical, 2);
        side_by_side.move_divider(2, area(), 5000.0, min());
        let narrower = Rectangle {
            width: 401.0,
            ..area()
        };
        let shrunk: Vec<f32> = side_by_side
            .regions(narrower, min())
            .iter()
            .map(|(_, rect)| rect.width.round())
            .collect();
        assert_eq!(shrunk, vec![300.0, 100.0]);

        let mut stacked = PaneNode::Leaf(1);
        stacked.split(1, Axis::Horizontal, 2);
        stacked.move_divider(2, area(), -50.0, min());
        let shorter = Rectangle {
            height: 201.0,
            ..area()
        };
        let heights: Vec<f32> = stacked
            .regions(shorter, min())
            .iter()
            .map(|(_, rect)| rect.height.round())
            .collect();
        assert_eq!(heights, vec![50.0, 150.0]);
    }

    #[test]
    fn a_nested_pane_keeps_its_minimum_while_the_area_shrinks() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 2);
        root.split(2, Axis::Vertical, 3);
        root.move_divider(3, area(), 501.0, min());
        let narrower = Rectangle {
            width: 501.0,
            ..area()
        };

        let shrunk: Vec<f32> = root
            .regions(narrower, min())
            .iter()
            .map(|(_, rect)| rect.width.round())
            .collect();
        assert_eq!(shrunk, vec![250.0, 100.0, 149.0]);
    }

    #[test]
    fn a_divider_inside_a_side_held_back_by_the_minimum_follows_the_pointer() {
        let mut root = PaneNode::Leaf(1);
        root.split(1, Axis::Vertical, 3);
        root.split(1, Axis::Vertical, 2);
        root.move_divider(3, area(), 5000.0, min());
        let narrower = Rectangle {
            width: 501.0,
            ..area()
        };

        assert!(root.move_divider(2, narrower, 250.0, min()));

        let inner = root.dividers(narrower, min())[1];
        assert_eq!(inner.position().round(), 250.0);
    }
}
