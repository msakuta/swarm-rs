use std::collections::HashMap;

use cgmath::Matrix3;
use eframe::{emath::RectTransform, epaint::PathShape};
use egui::{pos2, vec2, Color32, FontId, Frame, Painter, Pos2, Rect, RichText, Ui, Vec2};
use swarm_rs::behavior_tree_lite::{
    parse_file, parser::BlackboardValue, parser::TreeDef, PortType,
};

use crate::SwarmRsApp;

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
}

impl BTWidget {
    pub(crate) fn new() -> Self {
        Self {
            origin: [0.; 2],
            scale: 1.,
            canvas_offset: Pos2::ZERO,
            font_size: FontSize::Normal,
            tree: "main".to_string(),
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
        }

        let ui_result = {
            let input = ui.input();

            UiResult {
                pointer: input.pointer.primary_down(),
                delta: input.pointer.delta(),
            }
        };

        let source = &mut self.app_data.bt_buffer;
        let Ok((_, trees)) = parse_file(source) else {
            println!("Error on parsing source");
            return;
        };

        ui.horizontal(|ui| {
            ui.label("Tree:");

            ui.group(|ui| {
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
        });

        Frame::canvas(ui.style()).show(ui, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());

            self.app_data.bt_widget.canvas_offset = response.rect.min;
            self.app_data.bt_widget.scale = match self.app_data.bt_widget.font_size {
                FontSize::Small => 0.7,
                FontSize::Normal => 1.,
                FontSize::Large => 1.5,
            };

            let to_screen = egui::emath::RectTransform::from_to(
                Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                response.rect,
            );

            let Some(main) = trees.tree_defs.iter().find(|node| node.name() == self.app_data.bt_widget.tree) else {
                println!("No tree {main:?} defined in {tree:?}", main = self.app_data.bt_widget.tree, tree = trees.tree_defs.iter().map(|node| node.name()).collect::<Vec<_>>());
                return;
            };

            let mut node_painter = NodePainter::new(&self.app_data.bt_widget, &painter, &to_screen);

            let scale = self.app_data.bt_widget.scale as f32;
            node_painter.paint_node_recurse(NODE_PADDING * scale, NODE_PADDING * scale, &main.root());

            node_painter.render_connections();

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
                    self.app_data.bt_widget.origin[1] += ui_result.delta[1] as f64; // self.app_data.bt_compo.scale;
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

struct NodePainter<'p> {
    bt_component: &'p BTWidget,
    painter: &'p Painter,
    to_screen: &'p RectTransform,
    font: FontId,
    port_font: FontId,
    bb_connections: HashMap<String, BBConnection>,
    view_transform: Matrix3<f64>,
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

    fn paint_node_recurse(&mut self, mut x: f32, mut y: f32, node: &TreeDef<'_>) -> Vec2 {
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

        let mut subnode_connectors = vec![];
        for child in node.children() {
            let node_size =
                self.paint_node_recurse(x, y + size.y + node_padding2 + child_node_spacing, child);

            let to = self.to_pos2([
                x + node_size.x / 2.,
                y + size.y + node_padding2 + child_node_spacing,
            ]);
            subnode_connectors.push(to);

            x += node_size.x + node_padding2 + node_spacing;
        }

        let tree_width = size.x.max(x - initial_x - node_padding2 - node_spacing);
        let node_left = initial_x + (tree_width - size.x) / 2.;

        y += size.y + node_padding;
        let ports: Vec<_> = node
            .port_maps()
            .iter()
            .map(|port| {
                let port_type = port.get_type();
                let port_galley = self.painter.layout_no_wrap(
                    if let BlackboardValue::Literal(lit) = port.blackboard_value() {
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

                if let BlackboardValue::Ref(bbref) = port.blackboard_value() {
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

        self.painter.rect(
            Rect { min, max },
            0.,
            Color32::from_rgb(63, 63, 31),
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

        let from = self.to_pos2([node_left + size.x / 2., y + node_padding]);
        for to in subnode_connectors {
            self.painter.line_segment([from, to], (2., Color32::YELLOW));
        }

        size.x = size.x.max(tree_width);

        size
    }

    fn render_connections(&self) {
        for (_, con) in &self.bb_connections {
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
                }
            }
        }
    }
}

fn interp(v0: Pos2, v1: Pos2, f: f32) -> Pos2 {
    (v0.to_vec2() * (1. - f) + v1.to_vec2() * f).to_pos2()
}
