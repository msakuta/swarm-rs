use eframe::{emath::RectTransform, epaint::PathShape};
use egui::{pos2, Color32, FontId, Frame, Painter, Pos2, Rect, Ui, Vec2};
use swarm_rs::behavior_tree_lite::{parse_file, parser::TreeDef, PortType};

use crate::SwarmRsApp;

impl SwarmRsApp {
    pub(crate) fn paint_bt(&mut self, ui: &mut Ui) {
        Frame::canvas(ui.style()).show(ui, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());

            let to_screen = egui::emath::RectTransform::from_to(
                Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                response.rect,
            );

            let source = &mut self.app_data.bt_buffer;
            let Ok((_, tree)) = parse_file(source) else {
                println!("Error on parsing source");
                return;
            };

            let Some(main) = tree.tree_defs.iter().find(|node| node.name() == "main") else {
                println!("No main tree defined in {:?}", tree.tree_defs.iter().map(|node| node.name()).collect::<Vec<_>>());
                return;
            };

            let node_painter = NodePainter::new(&painter, &to_screen);

            node_painter.paint_node_recurse(NODE_PADDING, NODE_PADDING, &main.root());
        });
    }
}

/// Padding in one side
const NODE_PADDING: f32 = 5.;
/// Padding in both sides
const NODE_PADDING2: f32 = NODE_PADDING * 2.;
/// Space between node rectangles
const NODE_SPACING: f32 = 20.;

struct NodePainter<'p> {
    painter: &'p Painter,
    to_screen: &'p RectTransform,
    font: FontId,
    port_font: FontId,
}

impl<'p> NodePainter<'p> {
    fn new(painter: &'p Painter, to_screen: &'p RectTransform) -> Self {
        Self {
            painter,
            to_screen,
            font: FontId::monospace(16.),
            port_font: FontId::monospace(12.),
        }
    }

    fn paint_node_recurse(&self, mut x: f32, mut y: f32, node: &TreeDef<'_>) -> Vec2 {
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
                self.paint_node_recurse(x, y + size.y + NODE_PADDING2 + NODE_SPACING, child);

            let to = self.to_screen.transform_pos(pos2(
                x + node_size.x / 2.,
                y + size.y + NODE_PADDING2 + NODE_SPACING,
            ));
            subnode_connectors.push(to);

            x += node_size.x + NODE_PADDING2 + NODE_SPACING;
        }

        let tree_width = size.x.max(x - initial_x - NODE_PADDING2 - NODE_SPACING);
        let node_left = initial_x + (tree_width - size.x) / 2.;

        y += size.y + NODE_PADDING;
        let ports: Vec<_> = node
            .port_maps()
            .iter()
            .map(|port| {
                let port_type = port.get_type();
                let port_galley = self.painter.layout_no_wrap(
                    port.node_port().to_string(),
                    self.port_font.clone(),
                    match port_type {
                        PortType::Input => Color32::from_rgb(255, 255, 127),
                        PortType::Output => Color32::from_rgb(127, 255, 255),
                        PortType::InOut => Color32::from_rgb(127, 255, 127),
                    },
                );
                let port_height = port_galley.size().y;
                let port = (port_galley, y, port_type);
                y += port_height;
                port
            })
            .collect();

        self.painter.rect(
            self.to_screen.transform_rect(Rect {
                min: pos2(node_left, initial_y),
                max: pos2(node_left + NODE_PADDING2 + size.x, y + NODE_PADDING),
            }),
            0.,
            Color32::from_rgb(63, 63, 31),
            (1., Color32::from_rgb(127, 127, 191)),
        );

        self.painter.galley(
            self.to_screen
                .transform_pos(pos2(node_left + NODE_PADDING, initial_y + NODE_PADDING)),
            galley,
        );

        for (port, y, port_type) in ports {
            self.painter.galley(
                self.to_screen
                    .transform_pos(pos2(node_left + NODE_PADDING, y)),
                port,
            );

            let render_input = || {
                let mut path = vec![pos2(-5., 0.), pos2(-5., 12.), pos2(5., 6.)];
                for node in &mut path {
                    node.x += node_left;
                    node.y += y;
                    *node = self.to_screen.transform_pos(*node);
                }
                self.painter.add(PathShape::convex_polygon(
                    path,
                    Color32::DARK_GRAY,
                    (1., Color32::WHITE),
                ));
            };

            let render_output = || {
                let mut path = vec![pos2(-5., 0.), pos2(-5., 12.), pos2(5., 6.)];
                for node in &mut path {
                    node.x += node_left + size.x + NODE_PADDING2;
                    node.y += y;
                    *node = self.to_screen.transform_pos(*node);
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

        let from = self
            .to_screen
            .transform_pos(pos2(node_left + size.x / 2., y + NODE_PADDING));
        for to in subnode_connectors {
            self.painter.line_segment([from, to], (2., Color32::YELLOW));
        }

        size.x = tree_width;

        size
    }
}
