use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::view::ViewId;

#[derive(Debug, Clone, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum SplitNode {
    Leaf(ViewId),
    Split {
        direction: SplitDirection,
        children: Vec<SplitNode>,
        ratios: Vec<f32>,
    },
}

impl SplitNode {
    pub fn single(view_id: ViewId) -> Self {
        SplitNode::Leaf(view_id)
    }

    pub fn compute_rects(&self, area: Rect) -> Vec<(ViewId, Rect)> {
        let mut result = Vec::new();
        self.collect_rects(area, &mut result);
        result
    }

    fn collect_rects(&self, area: Rect, out: &mut Vec<(ViewId, Rect)>) {
        match self {
            SplitNode::Leaf(id) => {
                out.push((*id, area));
            }
            SplitNode::Split { direction, children, ratios } => {
                let dir = match direction {
                    SplitDirection::Horizontal => Direction::Vertical,
                    SplitDirection::Vertical => Direction::Horizontal,
                };

                let constraints: Vec<Constraint> = ratios
                    .iter()
                    .map(|r| Constraint::Percentage((*r * 100.0) as u16))
                    .collect();

                let chunks = Layout::default()
                    .direction(dir)
                    .constraints(constraints)
                    .split(area);

                for (i, child) in children.iter().enumerate() {
                    if i < chunks.len() {
                        child.collect_rects(chunks[i], out);
                    }
                }
            }
        }
    }

    pub fn split_view(&mut self, target: ViewId, new_id: ViewId, direction: SplitDirection) -> bool {
        match self {
            SplitNode::Leaf(id) if *id == target => {
                *self = SplitNode::Split {
                    direction,
                    children: vec![
                        SplitNode::Leaf(target),
                        SplitNode::Leaf(new_id),
                    ],
                    ratios: vec![0.5, 0.5],
                };
                true
            }
            SplitNode::Leaf(_) => false,
            SplitNode::Split { children, .. } => {
                for child in children.iter_mut() {
                    if child.split_view(target, new_id, direction.clone()) {
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn remove_view(&mut self, target: ViewId) -> Option<ViewId> {
        match self {
            SplitNode::Leaf(id) => {
                if *id == target {
                    None // Can't remove the root leaf
                } else {
                    Some(*id) // Not found here
                }
            }
            SplitNode::Split { children, ratios, .. } => {
                // Find which child contains the target
                let mut target_idx = None;
                for (i, child) in children.iter().enumerate() {
                    if child.contains_view(target) {
                        target_idx = Some(i);
                        break;
                    }
                }

                if let Some(idx) = target_idx {
                    if let SplitNode::Leaf(id) = &children[idx] {
                        if *id == target {
                            // Remove this leaf
                            children.remove(idx);
                            ratios.remove(idx);

                            // Normalize ratios
                            let sum: f32 = ratios.iter().sum();
                            if sum > 0.0 {
                                for r in ratios.iter_mut() {
                                    *r /= sum;
                                }
                            }

                            // If only one child left, collapse
                            if children.len() == 1 {
                                *self = children.remove(0);
                            }

                            return Some(target);
                        }
                    }

                    // Recurse into the child
                    children[idx].remove_view(target)
                } else {
                    None
                }
            }
        }
    }

    pub fn contains_view(&self, target: ViewId) -> bool {
        match self {
            SplitNode::Leaf(id) => *id == target,
            SplitNode::Split { children, .. } => {
                children.iter().any(|c| c.contains_view(target))
            }
        }
    }

    pub fn view_ids(&self) -> Vec<ViewId> {
        let mut ids = Vec::new();
        self.collect_ids(&mut ids);
        ids
    }

    fn collect_ids(&self, out: &mut Vec<ViewId>) {
        match self {
            SplitNode::Leaf(id) => out.push(*id),
            SplitNode::Split { children, .. } => {
                for child in children {
                    child.collect_ids(out);
                }
            }
        }
    }

    pub fn next_view_after(&self, current: ViewId) -> Option<ViewId> {
        let ids = self.view_ids();
        if let Some(pos) = ids.iter().position(|&id| id == current) {
            let next = (pos + 1) % ids.len();
            Some(ids[next])
        } else {
            ids.first().copied()
        }
    }
}
