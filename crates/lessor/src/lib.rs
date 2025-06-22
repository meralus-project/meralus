use std::collections::HashMap;

pub type Size2D = glamour::Size2;
pub type Point2D = glamour::Point2;
pub type Rect2D = glamour::Rect;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Thickness([f32; 4]);

impl Thickness {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self([left, top, right, bottom])
    }

    pub const fn all(value: f32) -> Self {
        Self([value; 4])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThicknessRule {
    Enlarge,
    Shrink,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutNode {
    pub outer: Rect2D,
    pub inner: Rect2D,
    pub padding_rule: ThicknessRule,
}

impl LayoutNode {
    pub const EMPTY: Self = Self::new(Point2D::ZERO, Size2D::ZERO);

    #[must_use]
    pub const fn new(origin: Point2D, size: Size2D) -> Self {
        let outer = Rect2D { origin, size };

        Self {
            outer,
            inner: outer,
            padding_rule: ThicknessRule::Enlarge,
        }
    }

    pub const fn set_origin(&mut self, origin: Point2D) {
        let diff_x = self.inner.origin.x - self.outer.origin.x;
        let diff_y = self.inner.origin.y - self.outer.origin.y;

        self.outer.origin = origin;
        self.inner.origin = origin;
        self.inner.origin.x += diff_x;
        self.inner.origin.y += diff_y;
    }

    pub const fn set_size(&mut self, size: Size2D) {
        self.set_width(size.width);
        self.set_height(size.height);
    }

    pub fn width(&self) -> f32 {
        let diff = self.outer.size.width - self.inner.size.width;

        self.outer.size.width - diff
    }

    pub fn height(&self) -> f32 {
        let diff = self.outer.size.height - self.inner.size.height;

        self.outer.size.height - diff
    }

    pub const fn set_width(&mut self, value: f32) {
        let diff = self.outer.size.width - self.inner.size.width;

        match self.padding_rule {
            ThicknessRule::Enlarge => {
                self.outer.size.width = value + diff;
                self.inner.size.width = value;
            }
            ThicknessRule::Shrink => {
                self.outer.size.width = value;
                self.inner.size.width = value - diff;
            }
        }
    }

    pub const fn set_height(&mut self, value: f32) {
        let diff = self.outer.size.height - self.inner.size.height;

        match self.padding_rule {
            ThicknessRule::Enlarge => {
                self.outer.size.height = value + diff;
                self.inner.size.height = value;
            }
            ThicknessRule::Shrink => {
                self.outer.size.height = value;
                self.inner.size.height = value - diff;
            }
        }
    }

    pub const fn try_set_width(&mut self, value: f32) {
        let diff = self.outer.size.width - self.inner.size.width;

        match self.padding_rule {
            ThicknessRule::Enlarge => {
                if self.inner.size.width < value {
                    self.outer.size.width = value + diff;
                    self.inner.size.width = value;
                }
            }
            ThicknessRule::Shrink => {
                if self.outer.size.width < value {
                    self.outer.size.width = value;
                    self.inner.size.width = value - diff;
                }
            }
        }
    }

    pub const fn try_set_height(&mut self, value: f32) {
        let diff = self.outer.size.height - self.inner.size.height;

        match self.padding_rule {
            ThicknessRule::Enlarge => {
                if self.inner.size.height < value {
                    self.outer.size.height = value + diff;
                    self.inner.size.height = value;
                }
            }
            ThicknessRule::Shrink => {
                if self.outer.size.height < value {
                    self.outer.size.height = value;
                    self.inner.size.height = value - diff;
                }
            }
        }
    }

    pub const fn add_width(&mut self, value: f32) {
        self.outer.size.width += value;
        self.inner.size.width += value;
    }

    pub const fn sub_width(&mut self, value: f32) {
        self.outer.size.width -= value;
        self.inner.size.width -= value;
    }

    pub const fn add_height(&mut self, value: f32) {
        self.outer.size.height += value;
        self.inner.size.height += value;
    }

    pub const fn sub_height(&mut self, value: f32) {
        self.outer.size.height -= value;
        self.inner.size.height -= value;
    }

    pub const fn get_padding(&self) -> Thickness {
        let left = self.inner.origin.x - self.outer.origin.x;
        let top = self.inner.origin.y - self.outer.origin.y;

        let right = self.outer.size.width - self.inner.size.width - left;
        let bottom = self.outer.size.height - self.inner.size.height - top;

        Thickness::new(left, top, right, bottom)
    }

    pub const fn add_padding(&mut self, value: Thickness, rule: ThicknessRule) {
        self.padding_rule = rule;

        let Thickness([left, top, right, bottom]) = value;

        match rule {
            ThicknessRule::Enlarge => {
                self.inner.origin.x += left;
                self.inner.origin.y += top;
                self.outer.size.width += left + right;
                self.outer.size.height += top + bottom;
            }
            ThicknessRule::Shrink => {
                self.inner.origin.x += left;
                self.inner.origin.y += top;
                self.inner.size.width -= left + right;
                self.inner.size.height -= top + bottom;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutContext {
    nodes_children: HashMap<usize, Vec<usize>>,
    nodes: Vec<LayoutNode>,
    current_node: usize,
}

impl LayoutContext {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            nodes_children: HashMap::from_iter([(0, Vec::new())]),
            nodes: vec![LayoutNode::new(Point2D::ZERO, Size2D::new(width, height))],
            current_node: 0,
        }
    }

    pub fn root_mut(&mut self) -> &mut LayoutNode {
        &mut self.nodes[0]
    }

    pub const fn next_node_index(&self) -> usize {
        self.nodes.len()
    }

    pub fn get_node_children(&self, node: usize) -> &[usize] {
        &self.nodes_children[&node]
    }

    pub fn get_node(&self, node: usize) -> &LayoutNode {
        &self.nodes[node]
    }

    pub fn clear(&mut self) {
        self.current_node = 0;
        self.nodes_children.clear();
        self.nodes_children.insert(0, Vec::new());
        self.nodes.truncate(1);
    }

    pub fn set_origin(&mut self, node_index: usize, origin: Point2D) {
        let prev_origin = self.nodes[node_index].outer.origin;

        self.nodes[node_index].set_origin(origin);

        let nodes = self.nodes_children[&node_index]
            .iter()
            .map(|node| origin + self.nodes[*node].outer.origin - prev_origin.to_vector())
            .collect::<Vec<_>>();

        for node in nodes {
            self.set_origin(node_index, node);
        }
    }

    pub fn measure_from_root<TM: TextMeasurer, T: Measurable<TM>>(
        &mut self,
        text_measurer: &TM,
        element: &T,
    ) -> (LayoutNode, usize) {
        self.measure(text_measurer, self.nodes[0].inner, element)
    }

    pub fn measure<TM: TextMeasurer, T: Measurable<TM>>(
        &mut self,
        text_measurer: &TM,
        bounds: Rect2D,
        element: &T,
    ) -> (LayoutNode, usize) {
        let node_index = self.next_node_index();

        self.nodes.push(LayoutNode::EMPTY);
        self.nodes_children.insert(node_index, Vec::new());

        let parent = self.current_node;

        self.current_node = node_index;

        let node = element.measure(text_measurer, self, bounds);

        self.current_node = parent;

        self.nodes[node_index] = node;

        if let Some(parent) = self.nodes_children.get_mut(&parent) {
            parent.push(node_index);
        }

        (node, node_index)
    }
}

pub trait TextMeasurer {
    fn measure_text<F: AsRef<str>, T: AsRef<str>>(
        &self,
        font: F,
        text: T,
        size: f32,
        width: Option<f32>,
    ) -> Size2D;
}

impl TextMeasurer for () {
    fn measure_text<F: AsRef<str>, T: AsRef<str>>(
        &self,
        _: F,
        _: T,
        _: f32,
        _: Option<f32>,
    ) -> Size2D {
        Size2D::ZERO
    }
}

pub trait Measurable<T: TextMeasurer = ()> {
    fn measure(&self, text_measurer: &T, context: &mut LayoutContext, bounds: Rect2D)
    -> LayoutNode;
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, marker::PhantomData};

    use float_cmp::assert_approx_eq;

    use crate::{
        LayoutContext, LayoutNode, Measurable, Point2D, Rect2D, Size2D, TextMeasurer, Thickness,
        ThicknessRule,
    };

    #[test]
    fn test_padding_enlarge() {
        let mut node = LayoutNode::new(Point2D::splat(32.0), Size2D::splat(48.0));

        node.add_padding(Thickness::all(24.0), ThicknessRule::Enlarge);

        assert_approx_eq!(f32, node.outer.origin.x, 8.0);
        assert_approx_eq!(f32, node.outer.origin.y, 8.0);
        assert_approx_eq!(f32, node.outer.size.width, 96.0);
        assert_approx_eq!(f32, node.outer.size.height, 96.0);

        let padding = node.get_padding();

        assert_approx_eq!(f32, padding.0[0], 24.0);
        assert_approx_eq!(f32, padding.0[1], 24.0);
        assert_approx_eq!(f32, padding.0[2], 24.0);
        assert_approx_eq!(f32, padding.0[3], 24.0);
    }

    #[test]
    fn test_padding_shrink() {
        let mut node = LayoutNode::new(Point2D::splat(32.0), Size2D::splat(48.0));

        node.add_padding(Thickness::all(24.0), ThicknessRule::Shrink);

        assert_approx_eq!(f32, node.inner.origin.x, 56.0);
        assert_approx_eq!(f32, node.inner.origin.y, 56.0);
        assert_approx_eq!(f32, node.inner.size.width, 0.0);
        assert_approx_eq!(f32, node.inner.size.height, 0.0);

        let padding = node.get_padding();

        assert_approx_eq!(f32, padding.0[0], 24.0);
        assert_approx_eq!(f32, padding.0[1], 24.0);
        assert_approx_eq!(f32, padding.0[2], 24.0);
        assert_approx_eq!(f32, padding.0[3], 24.0);
    }

    #[test]
    fn test_moving() {
        let mut node = LayoutNode::new(Point2D::splat(32.0), Size2D::splat(48.0));

        node.add_padding(Thickness::all(24.0), ThicknessRule::Shrink);

        node.set_origin(Point2D::splat(24.0));

        assert_approx_eq!(f32, node.inner.origin.x, 48.0);
        assert_approx_eq!(f32, node.inner.origin.y, 48.0);
        assert_approx_eq!(f32, node.inner.size.width, 0.0);
        assert_approx_eq!(f32, node.inner.size.height, 0.0);

        let padding = node.get_padding();

        assert_approx_eq!(f32, padding.0[0], 24.0);
        assert_approx_eq!(f32, padding.0[1], 24.0);
        assert_approx_eq!(f32, padding.0[2], 24.0);
        assert_approx_eq!(f32, padding.0[3], 24.0);
    }

    #[test]
    fn test_layout_context() {
        struct Rect200x200;

        impl Measurable<Boo<'_>> for Rect200x200 {
            fn measure(&self, _: &Boo<'_>, _: &mut LayoutContext, bounds: Rect2D) -> LayoutNode {
                LayoutNode::new(bounds.origin, Size2D::splat(200.0))
            }
        }

        struct Column {
            children: Vec<Rect200x200>,
        }

        impl Measurable<Boo<'_>> for Column {
            fn measure(
                &self,
                text_measurer: &Boo<'_>,
                context: &mut LayoutContext,
                bounds: Rect2D,
            ) -> LayoutNode {
                let mut available_size = bounds.size;
                let mut size = Size2D::ZERO;

                for node in &self.children {
                    let (node, _) = context.measure(
                        text_measurer,
                        Rect2D::new(
                            bounds.origin + size.with_width(0.0).to_vector(),
                            available_size,
                        ),
                        node,
                    );

                    size.width = size.width.max(node.outer.size.width);
                    size.height += node.outer.size.height;

                    available_size.height -= node.outer.size.height;
                }

                LayoutNode::new(bounds.origin, size)
            }
        }

        struct Boo<'a> {
            boo: PhantomData<&'a ()>,
        }

        impl TextMeasurer for Boo<'_> {
            fn measure_text<F: AsRef<str>, T: AsRef<str>>(
                &self,
                _: F,
                _: T,
                _: f32,
                _: Option<f32>,
            ) -> Size2D {
                Size2D::ZERO
            }
        }

        let mut context = LayoutContext::new(1024.0, 1024.0);

        context.clear();
        context.measure_from_root(&Boo { boo: PhantomData }, &Column {
            children: vec![Rect200x200, Rect200x200, Rect200x200],
        });

        assert_eq!(context.current_node, 0);
        assert_eq!(context.nodes.len(), 5);
        assert_eq!(
            context.nodes_children,
            HashMap::from_iter([
                (0, vec![1]),
                (1, vec![2, 3, 4]),
                (2, Vec::new()),
                (3, Vec::new()),
                (4, Vec::new()),
            ])
        );

        assert_approx_eq!(f32, context.nodes[0].inner.size.width, 1024.0);
        assert_approx_eq!(f32, context.nodes[0].inner.size.height, 1024.0);

        assert_approx_eq!(f32, context.nodes[1].inner.size.width, 200.0);
        assert_approx_eq!(f32, context.nodes[1].inner.size.height, 600.0);

        assert_approx_eq!(f32, context.nodes[2].inner.size.width, 200.0);
        assert_approx_eq!(f32, context.nodes[2].inner.size.height, 200.0);

        assert_approx_eq!(f32, context.nodes[3].inner.origin.y, 200.0);
        assert_approx_eq!(f32, context.nodes[3].inner.size.width, 200.0);
        assert_approx_eq!(f32, context.nodes[3].inner.size.height, 200.0);

        assert_approx_eq!(f32, context.nodes[4].inner.origin.y, 400.0);
        assert_approx_eq!(f32, context.nodes[4].inner.size.width, 200.0);
        assert_approx_eq!(f32, context.nodes[4].inner.size.height, 200.0);
    }
}
