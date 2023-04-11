use std::collections::HashMap;

use cgmath::Matrix3;
use eframe::{emath::RectTransform, epaint::PathShape};
use egui::{pos2, vec2, Color32, FontId, Frame, Painter, Pos2, Rect, RichText, Ui, Vec2};
use swarm_rs::behavior_tree_lite::{
    parser::PortMapOwned, parser::TreeDef, AbstractPortMap, BehaviorNodeContainer, BehaviorResult,
    BlackboardValueOwned, PortType,
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

    pub(crate) fn view_transform(&self) -> Matrix3<f64> {
        Matrix3::from_translation(self.origin.into())
    }
}

impl SwarmRsApp {
    pub(crate) fn paint_bt(&mut self, ui: &mut Ui) {
        struct UiResult {
            pointer: bool,
            delta: Vec2,
            clicked: Option<Pos2>,
        }

        let ui_result = {
            let input = ui.input();

            UiResult {
                pointer: input.pointer.primary_down(),
                delta: input.pointer.delta(),
                clicked: if input.pointer.primary_clicked() {
                    input.pointer.interact_pos()
                } else {
                    None
                },
            }
        };

        // let source
        let trees = match self.open_panel {
            Panel::Main => self.app_data.selected_entity.and_then(|id| {
                self.app_data.game.entities.iter().find_map(|entity| {
                    let entity = entity.borrow();
                    if entity.get_id() == id {
                        Some(entity)
                    } else {
                        None
                    }
                })
            }),
            _ => None,
            // Panel::BTEditor => {
            //     if self.app_data.bt_buffer.is_empty() {
            //         None
            //     } else {
            //         Some(Rc::new(self.app_data.bt_buffer.clone()))
            //     }
            // }
        };
        // let trees = source
        //     .as_ref()
        //     .and_then(|source| parse_file(source).ok())
        //     .map(|(_, trees)| trees);

        ui.horizontal(|ui| {
            ui.label("Tree:");

            ui.group(|ui| {
                if let Some(trees) = trees.as_ref().and_then(|trees| trees.behavior_tree()) {
                    // for tree in &trees.tree_defs {
                    let tree = &trees.0;
                    {
                        let mut tree_name = RichText::new(tree.name());
                        if self.app_data.bt_widget.tree == tree.name() {
                            // TODO: use black in light theme
                            tree_name = tree_name.underline().color(Color32::WHITE);
                        }
                        if ui.label(tree_name).interact(egui::Sense::click()).clicked() {
                            self.app_data.bt_widget.tree = tree.name().to_owned();
                        }
                    }
                } else {
                    match self.open_panel {
                        Panel::Main => ui.label(
                            RichText::new("Select an entity to show its behavior trees!")
                                .font(FontId::proportional(18.0))),
                        Panel::BTEditor => ui.label(
                            RichText::new("Select a btc source file or enter source to show its behavior trees!")
                                .font(FontId::proportional(18.0))),
                    };
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

            // let Some(main) = trees.tree_defs.iter().find(|node| node.name() == self.app_data.bt_widget.tree) else {
            //     return;
            // };
            let Some(main) = trees.behavior_tree() else { return };

            let mut node_painter = NodePainter::new(&self.app_data.bt_widget, &painter, &to_screen);

            if ui.ui_contains_pointer() {
                node_painter.clicked = ui_result.clicked;
            }

            let scale = self.app_data.bt_widget.scale as f32;
            node_painter.paint_node_recurse(NODE_PADDING * scale, NODE_PADDING * scale, &main.0);

            if self.app_data.bt_widget.show_var_connections {
                node_painter.render_variable_connections();
            }

            if ui.ui_contains_pointer() {
                // We disallow changing scale with a mouse wheel, because the font size does not scale linearly.
                // if ui_result.scroll_delta != 0. || ui_result.zoom_delta != 1. {
                //     let old_offset = transform_point(
                //         &self.app_data.bt_compo.inverse_view_transform(),
                //         ui_result.interact_pos,
                //     );
                //     if ui_result.scroll_delta < 0. {
                //         self.app_data.bt_compo.scale /= 1.2;
                //     } else if 0. < ui_result.scroll_delta {
                //         self.app_data.bt_compo.scale *= 1.2;
                //     } else if ui_result.zoom_delta != 1. {
                //         self.app_data.bt_compo.scale *= ui_result.zoom_delta as f64;
                //     }
                //     let new_offset = transform_point(
                //         &self.app_data.bt_compo.inverse_view_transform(),
                //         ui_result.interact_pos,
                //     );
                //     let diff = new_offset - old_offset;
                //     self.app_data.bt_compo.origin =
                //         (Vector2::<f64>::from(self.app_data.bt_compo.origin) + diff).into();
                // }

                if ui_result.pointer {
                    self.app_data.bt_widget.origin[0] += ui_result.delta[0] as f64; // self.app_data.bt_compo.scale;
                    self.app_data.bt_widget.origin[1] += ui_result.delta[1] as f64;
                    // self.app_data.bt_compo.scale;
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

#[derive(Default)]
struct BBConnection {
    source: Vec<Pos2>,
    dest: Vec<Pos2>,
}

trait AbstractNode<'src> {
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
}

impl<'src> AbstractNode<'src> for TreeDef<'src> {
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

struct NodePainter<'p> {
    bt_component: &'p BTWidget,
    painter: &'p Painter,
    to_screen: &'p RectTransform,
    font: FontId,
    port_font: FontId,
    bb_connections: HashMap<String, BBConnection>,
    view_transform: Matrix3<f64>,
    clicked: Option<Pos2>,
}

impl<'p> NodePainter<'p> {
    fn new(bt_component: &'p BTWidget, painter: &'p Painter, to_screen: &'p RectTransform) -> Self {
        let view_transform = bt_component.view_transform();
        Self {
            bt_component,
            painter,
            to_screen,
            font: FontId::monospace(bt_component.scale as f32 * 16.),
            port_font: FontId::monospace(bt_component.scale as f32 * 12.),
            bb_connections: HashMap::new(),
            view_transform,
            clicked: None,
        }
    }

    fn to_pos2(&self, pos: impl Into<[f32; 2]>) -> Pos2 {
        let offset = vec2(
            self.bt_component.origin[0] as f32,
            self.bt_component.origin[1] as f32,
        );
        let scale = 1.; //self.bt_component.scale;
        let pos = pos.into();
        let pos = transform_point(&self.view_transform, [pos[0] as f64, pos[1] as f64]);
        let pos = Vec2::new(pos.x as f32, pos.y as f32);
        self.to_screen
            .transform_pos(((pos + offset) * scale as f32).to_pos2())
    }

    fn paint_node_recurse<'src>(
        &mut self,
        mut x: f32,
        mut y: f32,
        node: &'src impl AbstractNode<'src>,
    ) -> Vec2 {
        let node_padding = NODE_PADDING * self.bt_component.scale as f32;
        let node_padding2 = NODE_PADDING2 * self.bt_component.scale as f32;
        let node_spacing = NODE_SPACING * self.bt_component.scale as f32;
        let child_node_spacing = CHILD_NODE_SPACING * self.bt_component.scale as f32;
        let port_radius = PORT_RADIUS * self.bt_component.scale as f32;
        let port_diameter = PORT_DIAMETER * self.bt_component.scale as f32;

        let initial_x = x;
        let initial_y = y;
        let galley = self.painter.layout_no_wrap(
            node.get_type().to_string(),
            self.font.clone(),
            Color32::WHITE,
        );

        let mut size = galley.size();
        let mut subtree_height = 0f32;

        let mut subnode_connectors = vec![];
        if !node.is_subtree() || node.is_subtree_expanded() {
            for child in node.children() {
                let child_y_offset = size.y + node_padding2 + child_node_spacing;
                let node_size = self.paint_node_recurse(x, y + child_y_offset, child);

                let to = self.to_pos2([
                    x + (node_size.x + node_padding) / 2.,
                    y + size.y + node_padding2 + child_node_spacing,
                ]);
                subnode_connectors.push(to);

                x += node_size.x + node_padding2 + node_spacing;
                subtree_height = subtree_height.max(node_size.y + child_y_offset);
            }
        }

        let tree_width = size.x.max(x - initial_x - node_padding2 - node_spacing);
        let node_left = initial_x + (tree_width - size.x) / 2.;

        y += size.y + node_padding;
        let ports: Vec<_> = node
            .port_maps()
            .map(|port| {
                let port_type = port.get_type();
                let port_galley = self.painter.layout_no_wrap(
                    if let BlackboardValueOwned::Literal(lit) = port.blackboard_value() {
                        format!("{} <- {:?}", port.node_port().to_string(), lit)
                    } else {
                        port.node_port().to_string()
                    },
                    self.port_font.clone(),
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

        let min = self.to_pos2([node_left, initial_y]);
        let max = self.to_pos2([node_left + node_padding2 + size.x, y + node_padding]);
        let rect = Rect { min, max };

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

        self.painter.rect(
            rect,
            0.,
            match node.get_last_result() {
                Some(BehaviorResult::Success) => Color32::from_rgb(31, 127, 31),
                Some(BehaviorResult::Fail) => Color32::from_rgb(127, 31, 31),
                Some(BehaviorResult::Running) => Color32::from_rgb(127, 127, 31),
                _ => Color32::from_rgb(63, 63, 31),
            },
            (1., Color32::from_rgb(127, 127, 191)),
        );

        self.painter.galley(
            self.to_pos2([node_left + node_padding, initial_y + node_padding]),
            galley,
        );

        for (port, y, port_type) in ports {
            self.painter
                .galley(self.to_pos2([node_left + node_padding, y]), port);

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

        let from = self.to_pos2([node_left + (size.x + node_padding) / 2., y + node_padding]);
        for to in subnode_connectors {
            self.painter.line_segment([from, to], (2., Color32::YELLOW));
        }

        let subtree_offset = if node.is_subtree_expanded() {
            // Add a bit of offset to separate nested subtree borders
            NODE_SPACING
        } else {
            0.
        };

        size.x = size.x.max(tree_width + subtree_offset);
        size.y = size.y.max(subtree_height + subtree_offset);

        size
    }

    fn render_variable_connections(&self) {
        for (name, con) in &self.bb_connections {
            for source in &con.source {
                for dest in &con.dest {
                    let from = self.to_pos2(*source);
                    let to = self.to_pos2(*dest);
                    let mut points = vec![];
                    let midpoint = ((from.to_vec2() + to.to_vec2()) / 2.).to_pos2();
                    let cp_length =
                        ((to.x - from.x) / 2.).min(100. * self.bt_component.scale as f32);
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

                    if self.bt_component.show_vars {
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
