use std::collections::HashMap;

use cgmath::{Matrix3, SquareMatrix};
use eframe::{emath::RectTransform, epaint::PathShape};
use egui::{
    pos2, vec2, Color32, FontId, Frame, Galley, Painter, Pos2, Rect, Response, RichText, Ui, Vec2,
};
use swarm_rs::behavior_tree_lite::{
    parse_file,
    parser::PortMapOwned,
    parser::{TreeDef, TreeSource},
    AbstractPortMap, BehaviorNodeContainer, BehaviorResult, BlackboardValueOwned, PortType,
};

use crate::{app::Panel, SwarmRsApp};

use super::transform_point;

#[derive(PartialEq, Eq, Debug)]
enum FontSize {
    Small,
    Normal,
    Large,
}

/// Data associated with behavior tree graphical editor widget.
/// egui does not really have a concept of widget, but it is the closest concept.
pub(crate) struct BTWidget {
    origin: [f64; 2],
    scale: f64,
    pub(crate) canvas_offset: Pos2,
    font_size: FontSize,
    tree: String,
    show_var_connections: bool,
    /// Whether to show the blackboard variables names on the connections.
    show_vars: bool,
}

impl BTWidget {
    pub(crate) fn new() -> Self {
        Self {
            origin: [0.; 2],
            scale: 1.,
            canvas_offset: Pos2::ZERO,
            font_size: FontSize::Normal,
            tree: "main".to_string(),
            show_var_connections: true,
            show_vars: true,
        }
    }
}

impl SwarmRsApp {
    pub(crate) fn paint_bt(&mut self, ui: &mut Ui) {
        let ui_result = UiResult::new(ui);

        enum Tree<'src> {
            Main(usize),
            BTEditor(TreeSource<'src>),
        }

        // let source
        let trees = match self.open_panel {
            Panel::Main => self.app_data.selected_entity.and_then(|id| {
                self.app_data.game.entities.iter().find_map(|entity| {
                    let entity = entity.borrow();
                    if entity.get_id() == id {
                        Some(Tree::Main(id))
                    } else {
                        None
                    }
                })
            }),
            Panel::BTEditor => {
                if self.app_data.bt_buffer.is_empty() {
                    None
                } else {
                    parse_file(&self.app_data.bt_buffer)
                        .ok()
                        .map(|(_, trees)| Tree::BTEditor(trees))
                }
            }
        };

        ui.horizontal(|ui| {
            ui.label("Tree:");

            ui.group(|ui| {
                match &trees {
                    &Some(Tree::Main(_id)) => {
                        let mut tree_name = RichText::new("Main");
                        // TODO: use black in light theme
                        tree_name = tree_name.underline().color(Color32::WHITE);
                        ui.label(tree_name);
                    }
                    Some(Tree::BTEditor(trees)) => {
                        for tree in &trees.tree_defs {
                            let mut tree_name = RichText::new(tree.name());
                            if self.app_data.bt_widget.tree == tree.name() {
                                // TODO: use black in light theme
                                tree_name = tree_name.underline().color(Color32::WHITE);
                            }
                            if ui.label(tree_name).interact(egui::Sense::click()).clicked() {
                                self.app_data.bt_widget.tree = tree.name().to_owned();
                            }
                        }
                    }
                    _ => {
                        match self.open_panel {
                            Panel::Main => ui.label(
                                RichText::new("Select an entity to show its behavior trees!")
                                    .font(FontId::proportional(18.0))),
                            Panel::BTEditor => ui.label(
                                RichText::new("Select a btc source file or enter source to show its behavior trees!")
                                    .font(FontId::proportional(18.0))),
                        };
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("Font size:");
            ui.radio_value(
                &mut self.app_data.bt_widget.font_size,
                FontSize::Small,
                "Small",
            );
            ui.radio_value(
                &mut self.app_data.bt_widget.font_size,
                FontSize::Normal,
                "Normal",
            );
            ui.radio_value(
                &mut self.app_data.bt_widget.font_size,
                FontSize::Large,
                "Large",
            );
            ui.checkbox(
                &mut self.app_data.bt_widget.show_var_connections,
                "Show variable connections",
            );
            ui.checkbox(
                &mut self.app_data.bt_widget.show_vars,
                "Show variable labels",
            );
        });

        Frame::canvas(ui.style()).show(ui, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());

            let Some(trees) = trees else { return };

            self.app_data.bt_widget.canvas_offset = response.rect.min;
            self.app_data.bt_widget.scale = match self.app_data.bt_widget.font_size {
                FontSize::Small => 0.3,
                FontSize::Normal => 1.,
                FontSize::Large => 1.5,
            };

            let to_screen = egui::emath::RectTransform::from_to(
                Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                response.rect,
            );

            match trees {
                Tree::Main(id) => {
                    let node_painter = NodePainter::new(
                        &mut self.app_data.bt_widget,
                        ui,
                        &ui_result,
                        &painter,
                        &response,
                        &to_screen,
                    );
                    if let Some(entity) = self.app_data.game.get_entity(id) {
                        if let Some(tree) = entity.behavior_tree() {
                            node_painter.draw_trees(&tree.0);
                        }
                    }
                }
                Tree::BTEditor(trees) => {
                    if let Some(main) = trees
                        .tree_defs
                        .iter()
                        .find(|node| node.name() == self.app_data.bt_widget.tree)
                    {
                        let node_painter = NodePainter::new(
                            &mut self.app_data.bt_widget,
                            ui,
                            &ui_result,
                            &painter,
                            &response,
                            &to_screen,
                        );
                        node_painter.draw_trees(main.root());
                    }
                }
            }

            if ui.ui_contains_pointer() {
                if ui_result.pointer {
                    self.app_data.bt_widget.origin[0] += ui_result.delta[0] as f64;
                    self.app_data.bt_widget.origin[1] += ui_result.delta[1] as f64;
                }
            }
        });
    }
}

/// Padding in one side
const NODE_PADDING: f32 = 5.;
/// Padding in both sides
const NODE_PADDING2: f32 = NODE_PADDING * 2.;
/// Space between node rectangles
const NODE_SPACING: f32 = 20.;
const NODE_BORDER_OFFSET: f32 = 5.;
/// Space between node rectangles
const CHILD_NODE_SPACING: f32 = 40.;
/// Radius of the port markers
const PORT_RADIUS: f32 = 6.;
/// Diameter of the port markers
const PORT_DIAMETER: f32 = PORT_RADIUS * 2.;
/// Node minimap width, height is calculated
const NODE_MAP_WIDTH: f32 = 200.;

#[derive(Default)]
struct BBConnection {
    source: Vec<Pos2>,
    dest: Vec<Pos2>,
}

trait AbstractNode<'src> {
    type Galley: AbstractGalley;
    fn get_type(&self) -> &str;
    fn children(&'src self) -> Box<dyn Iterator<Item = &Self> + 'src>;
    fn port_maps(&'src self) -> Box<dyn Iterator<Item = PortMapOwned> + 'src>;
    fn get_last_result(&self) -> Option<BehaviorResult>;
    fn is_subtree(&self) -> bool;
    fn is_subtree_expanded(&self) -> bool {
        false
    }
    fn expand_subtree(&self, _b: bool) -> bool {
        false
    }
    /// Customization point for rendered rectangle position and size.
    fn rect(&self) -> Option<Rect> {
        None
    }
    fn connector_top(&self, subtree_size: Vec2, size: Vec2, xy: Vec2) -> [f32; 2] {
        [
            (subtree_size.x + NODE_PADDING) / 2. + xy.x,
            size.y + NODE_PADDING2 + CHILD_NODE_SPACING + xy.y,
        ]
    }
    fn draw_border(&self) -> bool {
        true
    }
    fn line_width(&self) -> f32 {
        2.
    }
}

impl<'src> AbstractNode<'src> for TreeDef<'src> {
    type Galley = std::sync::Arc<Galley>;

    fn get_type(&self) -> &str {
        TreeDef::get_type(self)
    }

    fn children(&'src self) -> Box<dyn Iterator<Item = &Self> + 'src> {
        Box::new(self.children().iter())
    }

    fn port_maps(&'src self) -> Box<dyn Iterator<Item = PortMapOwned> + 'src> {
        Box::new(self.port_maps().iter().map(|port| port.to_owned()))
    }

    fn get_last_result(&self) -> Option<BehaviorResult> {
        None
    }

    fn is_subtree(&self) -> bool {
        false
    }
}

impl<'src> AbstractNode<'src> for BehaviorNodeContainer {
    type Galley = std::sync::Arc<Galley>;

    fn get_type(&self) -> &str {
        self.name()
    }

    fn children(&'src self) -> Box<dyn Iterator<Item = &Self> + 'src> {
        Box::new(self.children().iter())
    }

    fn port_maps(&self) -> Box<dyn Iterator<Item = PortMapOwned>> {
        Box::new(self.port_map())
    }

    fn get_last_result(&self) -> Option<BehaviorResult> {
        self.last_result()
    }

    fn is_subtree(&self) -> bool {
        BehaviorNodeContainer::is_subtree(self)
    }

    fn is_subtree_expanded(&self) -> bool {
        self.is_subtree_expanded()
    }

    fn expand_subtree(&self, b: bool) -> bool {
        BehaviorNodeContainer::expand_subtree(self, b);
        true
    }
}

trait AbstractGalley {
    fn layout_no_wrap(
        painter: &Painter,
        font: &FontId,
        s: impl ToString + AsRef<str>,
        color: Color32,
    ) -> Self;
    fn size(&self) -> Vec2;
    fn galley(&self, painter: &Painter, pos: Pos2);
}

impl AbstractGalley for std::sync::Arc<Galley> {
    fn layout_no_wrap(
        painter: &Painter,
        font: &FontId,
        s: impl ToString + AsRef<str>,
        _color: Color32,
    ) -> Self {
        painter.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE)
    }

    fn size(&self) -> Vec2 {
        Galley::size(&self)
    }

    fn galley(&self, painter: &Painter, pos: Pos2) {
        painter.galley(pos, self.clone());
    }
}

impl AbstractGalley for usize {
    fn layout_no_wrap(
        _painter: &Painter,
        _font: &FontId,
        s: impl ToString + AsRef<str>,
        _color: Color32,
    ) -> Self {
        s.as_ref().len()
    }

    fn size(&self) -> Vec2 {
        vec2(*self as f32 * 4., 6.)
    }

    fn galley(&self, _painter: &Painter, _pos: Pos2) {}
}

struct RenderedNode {
    rect: Rect,
    children: Vec<RenderedNode>,
    result: Option<BehaviorResult>,
}

impl<'src> AbstractNode<'src> for RenderedNode {
    type Galley = usize;

    fn get_type(&self) -> &str {
        ""
    }

    fn children(&'src self) -> Box<dyn Iterator<Item = &Self> + 'src> {
        Box::new(self.children.iter())
    }

    fn port_maps(&'src self) -> Box<dyn Iterator<Item = PortMapOwned> + 'src> {
        Box::new(std::iter::empty())
    }

    fn get_last_result(&self) -> Option<BehaviorResult> {
        self.result
    }

    fn is_subtree(&self) -> bool {
        false
    }

    fn rect(&self) -> Option<Rect> {
        Some(self.rect)
    }

    fn connector_top(&self, _node_size: Vec2, _size: Vec2, _xy: Vec2) -> [f32; 2] {
        [self.rect.center().x, self.rect.top()]
    }

    fn draw_border(&self) -> bool {
        false
    }

    fn line_width(&self) -> f32 {
        0.5
    }
}

#[derive(Clone, Copy)]
struct UiResult {
    pointer: bool,
    delta: Vec2,
    clicked: Option<Pos2>,
    interact_pos: Pos2,
}

impl UiResult {
    fn new(ui: &mut Ui) -> Self {
        let input = ui.input();
        let interact_pos = input.pointer.interact_pos().unwrap_or(Pos2::ZERO);

        UiResult {
            pointer: input.pointer.primary_down(),
            delta: input.pointer.delta(),
            clicked: if input.pointer.primary_clicked() {
                input.pointer.interact_pos()
            } else {
                None
            },
            interact_pos,
        }
    }
}

struct NodePainter<'p> {
    bt_widget: &'p mut BTWidget,
    ui: &'p Ui,
    ui_result: &'p UiResult,
    painter: &'p Painter,
    response: &'p Response,
    to_screen: &'p RectTransform,
    font: FontId,
    port_font: FontId,
    render_font: bool,
    bb_connections: HashMap<String, BBConnection>,
    view_transform: Matrix3<f64>,
    clicked: Option<Pos2>,
    offset: Vec2,
    scale: f32,
    record_rendered_tree: bool,
}

impl<'p> NodePainter<'p> {
    fn new(
        bt_widget: &'p mut BTWidget,
        ui: &'p Ui,
        ui_result: &'p UiResult,
        painter: &'p Painter,
        response: &'p Response,
        to_screen: &'p RectTransform,
    ) -> Self {
        let view_transform = Matrix3::identity(); //bt_component.view_transform();
        let font = FontId::monospace(bt_widget.scale as f32 * 16.);
        let port_font = FontId::monospace(bt_widget.scale as f32 * 12.);
        let offset = vec2(bt_widget.origin[0] as f32, bt_widget.origin[1] as f32);
        let scale = bt_widget.scale as f32;
        Self {
            bt_widget,
            ui,
            ui_result,
            painter,
            response,
            to_screen,
            font,
            port_font,
            render_font: true,
            bb_connections: HashMap::new(),
            view_transform,
            clicked: if ui.ui_contains_pointer() {
                ui_result.clicked
            } else {
                None
            },
            offset,
            scale,
            record_rendered_tree: true,
        }
    }

    fn to_pos2(&self, pos: impl Into<[f32; 2]>) -> Pos2 {
        let pos = pos.into();
        let pos = transform_point(&self.view_transform, [pos[0] as f64, pos[1] as f64]);
        let pos = Vec2::new(pos.x as f32, pos.y as f32);
        self.to_screen
            .transform_pos(((pos + self.offset) * self.scale as f32).to_pos2())
    }

    fn draw_trees<'a, 'src>(mut self, main: &'a impl AbstractNode<'src>)
    where
        'a: 'src,
    {
        let map_screen_rect = self.to_screen.transform_rect(Rect {
            min: pos2(
                self.painter.clip_rect().max.x - NODE_MAP_WIDTH - NODE_SPACING,
                0.,
            ),
            max: pos2(
                self.painter.clip_rect().max.x - NODE_SPACING,
                NODE_MAP_WIDTH,
            ),
        });

        if self.ui.rect_contains_pointer(map_screen_rect) {
            self.clicked = None;
        }

        let scale = self.scale as f32;
        let (tree_size, rendered_tree) =
            self.paint_node_recurse(NODE_PADDING * scale, NODE_PADDING * scale, main);

        if self.bt_widget.show_var_connections {
            self.render_variable_connections();
        }

        let painter_rect_size = self.painter.clip_rect().size();

        self.painter.rect(
            map_screen_rect,
            0.,
            Color32::from_black_alpha(127),
            (1., Color32::GRAY),
        );

        let origin = self.bt_widget.origin;
        let subpainter = self.painter.with_clip_rect(map_screen_rect);
        let mut node_painter = self;

        node_painter.painter = &subpainter;
        node_painter.offset = Vec2::ZERO;
        node_painter.scale = map_screen_rect.width() / tree_size.x;
        node_painter.render_font = false;
        node_painter.record_rendered_tree = false;
        node_painter.view_transform = Matrix3::identity();
        let map_extent_rect = Rect::from_min_size(Pos2::ZERO, map_screen_rect.size());
        let to_screen = egui::emath::RectTransform::from_to(map_extent_rect, map_screen_rect);
        node_painter.to_screen = &to_screen;
        let rendered_tree = rendered_tree.unwrap();
        node_painter.paint_node_recurse(0., 0., &rendered_tree);

        let view_rect = Rect::from_min_size(
            Pos2::new(-origin[0] as f32, -origin[1] as f32),
            node_painter.response.rect.size(),
        );
        let view_rect = Rect {
            min: node_painter.to_pos2(view_rect.min),
            max: node_painter.to_pos2(view_rect.max),
        };
        node_painter
            .painter
            .rect_stroke(view_rect, 0., (1., Color32::WHITE));

        if node_painter.ui.rect_contains_pointer(map_screen_rect) {
            if node_painter.ui_result.pointer {
                let inverse_map_trans = RectTransform::from_to(map_screen_rect, map_extent_rect);
                let interact_pos = inverse_map_trans
                    .transform_pos(node_painter.ui_result.interact_pos)
                    .to_vec2();
                let point =
                    -interact_pos * tree_size.x / map_screen_rect.width() + painter_rect_size / 2.;
                node_painter.bt_widget.origin[0] = point.x as f64;
                node_painter.bt_widget.origin[1] = point.y as f64;
            }
        }
    }

    fn paint_node_recurse<'src, N: AbstractNode<'src>>(
        &mut self,
        mut x: f32,
        mut y: f32,
        node: &'src N,
    ) -> (Vec2, Option<RenderedNode>) {
        let node_padding = NODE_PADDING * self.scale;
        let node_padding2 = NODE_PADDING2 * self.scale;
        let node_spacing = NODE_SPACING * self.scale;
        let child_node_spacing = CHILD_NODE_SPACING * self.scale;
        let port_radius = PORT_RADIUS * self.scale;
        let port_diameter = PORT_DIAMETER * self.scale;

        let initial_x = x;
        let initial_y = y;
        let galley =
            N::Galley::layout_no_wrap(&self.painter, &self.font, node.get_type(), Color32::WHITE);

        let mut size = galley.size();
        let mut subtree_height = 0f32;

        let mut subnode_connectors = vec![];
        let mut rendered_subtrees = vec![];
        if !node.is_subtree() || node.is_subtree_expanded() {
            for child in node.children() {
                let child_y_offset = size.y + node_padding2 + child_node_spacing;
                let (subtree_size, rendered_subtree) =
                    self.paint_node_recurse(x, y + child_y_offset, child);

                let to = self.to_pos2(Vec2::from(child.connector_top(
                    subtree_size,
                    size,
                    vec2(x, y),
                )));
                subnode_connectors.push(to);

                x += subtree_size.x + node_padding2 + node_spacing;
                subtree_height = subtree_height.max(subtree_size.y + child_y_offset);
                if let Some(rendered_subtree) = rendered_subtree {
                    rendered_subtrees.push(rendered_subtree);
                }
            }
        }

        let tree_width = size.x.max(x - initial_x - node_padding2 - node_spacing);
        let node_left = initial_x + (tree_width - size.x) / 2.;

        y += size.y + node_padding;
        let ports: Vec<_> = node
            .port_maps()
            .map(|port| {
                let port_type = port.get_type();
                let port_galley = N::Galley::layout_no_wrap(
                    &self.painter,
                    &self.port_font,
                    if let BlackboardValueOwned::Literal(lit) = port.blackboard_value() {
                        format!("{} <- {:?}", port.node_port().to_string(), lit)
                    } else {
                        port.node_port().to_string()
                    },
                    match port_type {
                        PortType::Input => Color32::from_rgb(255, 255, 127),
                        PortType::Output => Color32::from_rgb(127, 255, 255),
                        PortType::InOut => Color32::from_rgb(127, 255, 127),
                    },
                );
                let port_height = port_galley.size().y;
                let port_width = port_galley.size().x;
                let ret = (port_galley, y, port_type);

                if let BlackboardValueOwned::Ref(bbref) = port.blackboard_value() {
                    let con = self
                        .bb_connections
                        .entry(bbref.to_string())
                        .or_insert(BBConnection::default());
                    match port.get_type() {
                        PortType::Input => con.dest.push(pos2(node_left, y + port_radius)),
                        PortType::Output => con
                            .source
                            .push(pos2(node_left + size.x + node_padding2, y + port_radius)),
                        _ => (),
                    }
                }

                size.x = size.x.max(port_width);

                y += port_height;
                ret
            })
            .collect();

        let min = [node_left, initial_y];
        let max = [node_left + node_padding2 + size.x, y + node_padding];
        let rect_org = node.rect().unwrap_or(Rect {
            min: min.into(),
            max: max.into(),
        });
        let rect = Rect {
            min: self.to_pos2(rect_org.min),
            max: self.to_pos2(rect_org.max),
        };

        subtree_height = subtree_height.max(y - initial_y);

        if node.is_subtree() {
            if let Some(pos) = self.clicked {
                if rect.intersects(Rect { min: pos, max: pos }) {
                    node.expand_subtree(!node.is_subtree_expanded());
                }
            }
            // Show double border to imply that it is expandable with a click
            self.painter.rect_stroke(
                rect.expand(NODE_BORDER_OFFSET),
                0.,
                (1., Color32::from_rgb(127, 127, 191)),
            );

            if node.is_subtree_expanded() {
                let tree_rect = Rect {
                    min: self.to_pos2([initial_x, initial_y]),
                    max: self.to_pos2([
                        initial_x + tree_width + node_padding2,
                        initial_y + subtree_height + node_padding,
                    ]),
                }
                .expand(NODE_BORDER_OFFSET);
                self.painter
                    .rect_stroke(tree_rect, 0., (1., Color32::from_rgb(127, 127, 191)));
            }
        }

        let fill_color = match node.get_last_result() {
            Some(BehaviorResult::Success) => Color32::from_rgb(31, 127, 31),
            Some(BehaviorResult::Fail) => Color32::from_rgb(127, 31, 31),
            Some(BehaviorResult::Running) => Color32::from_rgb(127, 127, 31),
            _ => Color32::from_rgb(31, 31, 95),
        };
        if node.draw_border() {
            self.painter
                .rect(rect, 0., fill_color, (1., Color32::from_rgb(127, 127, 191)));
        } else {
            self.painter.rect_filled(rect, 0., fill_color);
        }

        galley.galley(
            &self.painter,
            self.to_pos2([node_left + node_padding, initial_y + node_padding]),
        );

        for (port, y, port_type) in ports {
            port.galley(&self.painter, self.to_pos2([node_left + node_padding, y]));

            let render_input = || {
                let mut path = vec![
                    pos2(-port_radius, 0.),
                    pos2(-port_radius, port_diameter),
                    pos2(port_radius, port_radius),
                ];
                for node in &mut path {
                    node.x += node_left;
                    node.y += y;
                    *node = self.to_pos2(*node);
                }
                self.painter.add(PathShape::convex_polygon(
                    path,
                    Color32::DARK_GRAY,
                    (1., Color32::WHITE),
                ));
            };

            let render_output = || {
                let mut path = vec![
                    pos2(-port_radius, 0.),
                    pos2(-port_radius, port_diameter),
                    pos2(port_radius, port_radius),
                ];
                for node in &mut path {
                    node.x += node_left + size.x + node_padding2;
                    node.y += y;
                    *node = self.to_pos2(*node);
                }
                self.painter.add(PathShape::convex_polygon(
                    path,
                    Color32::DARK_GRAY,
                    (1., Color32::WHITE),
                ));
            };

            match port_type {
                PortType::Input => render_input(),
                PortType::Output => render_output(),
                _ => {
                    render_input();
                    render_output();
                }
            }
        }

        let from = pos2(rect.center().x, rect.bottom());
        for to in subnode_connectors {
            self.painter
                .line_segment([from, to], (node.line_width(), Color32::YELLOW));
        }

        let subtree_offset = if node.is_subtree_expanded() {
            // Add a bit of offset to separate nested subtree borders
            NODE_SPACING
        } else {
            0.
        };

        size.x = size.x.max(tree_width + subtree_offset);
        size.y = size.y.max(subtree_height + subtree_offset);

        (
            size,
            if self.record_rendered_tree {
                Some(RenderedNode {
                    rect: rect_org,
                    children: rendered_subtrees,
                    result: node.get_last_result(),
                })
            } else {
                None
            },
        )
    }

    fn render_variable_connections(&self) {
        for (name, con) in &self.bb_connections {
            for source in &con.source {
                for dest in &con.dest {
                    let from = self.to_pos2(*source);
                    let to = self.to_pos2(*dest);
                    let mut points = vec![];
                    let midpoint = ((from.to_vec2() + to.to_vec2()) / 2.).to_pos2();
                    let cp_length = ((to.x - from.x) / 2.).min(100. * self.scale);
                    let from_cp = from + vec2(cp_length, 0.);
                    let to_cp = to + vec2(-cp_length, 0.);
                    let interpolates = 10;
                    for i in 0..=interpolates {
                        let f = i as f32 / interpolates as f32;
                        let p0 = interp(from, from_cp, f);
                        let p1 = interp(from_cp, midpoint, f);
                        let p2 = interp(p0, p1, f);
                        points.push(p2);
                    }
                    for i in 0..=interpolates {
                        let f = i as f32 / interpolates as f32;
                        let p0 = interp(midpoint, to_cp, f);
                        let p1 = interp(to_cp, to, f);
                        let p2 = interp(p0, p1, f);
                        points.push(p2);
                    }
                    self.painter.add(PathShape::line(
                        points,
                        (2., Color32::from_rgb(255, 127, 255)),
                    ));

                    if self.bt_widget.show_vars {
                        let galley = self.painter.layout_no_wrap(
                            name.to_string(),
                            self.port_font.clone(),
                            Color32::WHITE,
                        );
                        let mut rect = galley.rect;
                        let text_pos = pos2(midpoint.x - rect.width() / 2., midpoint.y);
                        rect.min = text_pos;
                        rect.max += text_pos.to_vec2();
                        self.painter.rect(
                            rect,
                            0.,
                            Color32::from_black_alpha(255),
                            (1., Color32::from_rgb(255, 127, 255)),
                        );
                        self.painter.galley(text_pos, galley);
                    }
                }
            }
        }
    }
}

fn interp(v0: Pos2, v1: Pos2, f: f32) -> Pos2 {
    (v0.to_vec2() * (1. - f) + v1.to_vec2() * f).to_pos2()
}
