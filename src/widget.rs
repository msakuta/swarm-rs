use std::rc::Rc;

use crate::{
    agent::AvoidanceRenderParams,
    app_data::{AppData, LineMode},
    board_widget::BoardWidget,
    game::{AvoidanceMode, BoardType},
};
use behavior_tree_lite::parse_file;
use druid::widget::{
    Button, Checkbox, CrossAxisAlignment, Flex, Label, Radio, RadioGroup, Scroll, Slider, Tabs,
    TextBox, WidgetExt,
};
use druid::Color;
use druid::{lens::Field, widget::prelude::*};

const BG: Color = Color::rgb8(0, 0, 53 as u8);
const BAR_WIDTH: f64 = 400.;

pub(crate) fn make_widget() -> impl Widget<AppData> {
    let main_tab = Flex::column()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_child(Label::new(|data: &AppData, _env: &_| data.message.clone()).padding(5.))
        .with_child(
            Checkbox::new("Pause")
                .lens(Field::new(
                    |app_data: &AppData| &app_data.game_params.paused,
                    |app_data| &mut app_data.game_params.paused,
                ))
                .padding(5.),
        )
        .with_child(
            Flex::column()
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    Button::new("New Game")
                        .on_click(|ctx, data: &mut AppData, _: &Env| {
                            data.new_game();
                            ctx.request_paint();
                        })
                        .padding(5.0),
                )
                .with_child(
                    RadioGroup::row([
                        ("Rect", BoardType::Rect),
                        ("Crank", BoardType::Crank),
                        ("Perlin", BoardType::Perlin),
                        ("Rooms", BoardType::Rooms),
                        ("Maze", BoardType::Maze),
                    ])
                    .lens(AppData::board_type),
                )
                .with_child(
                    Flex::row()
                        .with_child(Label::new("X size:").padding(3.0))
                        .with_child(TextBox::new().lens(AppData::rows_text))
                        .with_child(Label::new("Y size: ").padding(3.0))
                        .with_child(TextBox::new().lens(AppData::columns_text))
                        .padding(5.0),
                )
                .with_child(
                    Flex::row()
                        .with_child(Label::new("Seed: ").padding(3.0))
                        .with_child(TextBox::new().lens(AppData::seed_text))
                        .with_child(Label::new("Maze expansion: ").padding(3.0))
                        .with_child(TextBox::new().lens(AppData::maze_expansions))
                        .padding(5.0),
                )
                .with_child(
                    Flex::row()
                        .with_child(Label::new("Simplify: ").padding(3.0))
                        .with_child(TextBox::new().lens(AppData::simplify_text))
                        .padding(5.0),
                )
                .padding(2.0)
                .border(Color::GRAY, 2.),
        )
        .with_child(
            Flex::row()
                .with_child(Label::new("Border line mode:").padding(3.0))
                .with_child(Radio::new("none", LineMode::None))
                .with_child(Radio::new("line", LineMode::Line))
                .with_child(Radio::new("polygon", LineMode::Polygon))
                .lens(AppData::line_mode)
                .padding(5.),
        )
        .with_child(
            Checkbox::new("Simplified border")
                .lens(AppData::simplified_visible)
                .padding(5.),
        )
        .with_child(
            Flex::row()
                .with_child(Checkbox::new("Triangulation").lens(AppData::triangulation_visible))
                .with_child(Checkbox::new("Unpassable").lens(AppData::unpassable_visible))
                .with_child(Checkbox::new("Label").lens(AppData::triangle_label_visible))
                .with_child(Checkbox::new("Label image").lens(AppData::show_label_image))
                .padding(5.),
        )
        .with_child(
            Flex::row()
                .with_child(
                    Checkbox::new("Path")
                        .lens(AppData::path_visible)
                        .padding(5.),
                )
                .with_child(
                    AvoidanceRenderParams::gen_widgets()
                        .lens(AppData::avoidance_render_params)
                        .padding(5.),
                ),
        )
        .with_child(
            Flex::row()
                .with_child(Label::new("Avoidance mode:").padding(3.0))
                .with_child(
                    Flex::column()
                        .with_child(
                            RadioGroup::row([
                                ("Kinematic", AvoidanceMode::Kinematic),
                                ("RRT", AvoidanceMode::Rrt),
                                ("RRT*", AvoidanceMode::RrtStar),
                            ])
                            .lens(Field::new(
                                |data: &AppData| &data.game_params.avoidance_mode,
                                |data: &mut AppData| &mut data.game_params.avoidance_mode,
                            ))
                            .padding(5.),
                        )
                        .with_child(
                            Flex::row()
                                .cross_axis_alignment(CrossAxisAlignment::Start)
                                .with_child(Label::new(|data: &AppData, _: &_| {
                                    format!("Expands: {:.0}", data.game_params.avoidance_expands)
                                }))
                                .with_child(Slider::new().with_range(1., 20.).lens(Field::new(
                                    |data: &AppData| &data.game_params.avoidance_expands,
                                    |data: &mut AppData| &mut data.game_params.avoidance_expands,
                                ))),
                        ),
                ),
        )
        .with_child(
            Flex::row()
                .with_child(
                    Checkbox::new("QTree")
                        .lens(AppData::qtree_visible)
                        .padding(5.),
                )
                .with_child(
                    Checkbox::new("QTree search")
                        .lens(AppData::qtree_search_visible)
                        .padding(5.),
                ),
        )
        .with_child(
            Flex::row()
                .with_child(
                    Checkbox::new("Target line")
                        .lens(AppData::target_visible)
                        .padding(5.),
                )
                .with_child(
                    Checkbox::new("Entity label")
                        .lens(AppData::entity_label_visible)
                        .padding(5.),
                ),
        )
        .with_child(
            Checkbox::new("Entity trace")
                .lens(AppData::entity_trace_visible)
                .padding(5.),
        )
        .with_child(Label::new(|data: &AppData, _: &_| {
            format!("Scale: {}", data.scale)
        }))
        .with_child(Label::new(|data: &AppData, _: &_| {
            data.render_stats.borrow().clone()
        }))
        .with_child(Flex::row().with_flex_child(
            Label::new(|data: &AppData, _: &_| {
                let game = data.game.borrow();
                let profiler = game.triangle_profiler.borrow();
                format!(
                    "Triangle time: {:.06}ms, calls: {} size: {}, refs: {}",
                    profiler.get_average() * 1e3,
                    profiler.get_count(),
                    std::mem::size_of::<AppData>(),
                    Rc::strong_count(&data.game)
                )
            }),
            1.,
        ))
        .with_child(Flex::row().with_flex_child(
            Label::new(|data: &AppData, _: &_| {
                let game = data.game.borrow();
                let profiler = game.pixel_profiler.borrow();
                format!(
                    "Pixel time: {:.06}ms, calls: {}",
                    profiler.get_average() * 1e3,
                    profiler.get_count()
                )
            }),
            1.,
        ))
        .with_child(Flex::row().with_flex_child(
            Label::new(|data: &AppData, _: &_| {
                let game = data.game.borrow();
                let profiler = game.qtree_profiler.borrow();
                format!(
                    "QTree update time: {:.06}ms, calls: {}",
                    profiler.get_average() * 1e3,
                    profiler.get_count()
                )
            }),
            1.,
        ))
        .with_child(Flex::row().with_flex_child(
            Label::new(|data: &AppData, _: &_| {
                let game = data.game.borrow();
                let profiler = game.path_find_profiler.borrow();
                format!(
                    "Path find time: {:.06}ms, calls: {}",
                    profiler.get_average() * 1e3,
                    profiler.get_count()
                )
            }),
            1.,
        ))
        .with_child(
            Label::new(|app_data: &AppData, _: &_| {
                if let Some(pos) = app_data.mouse_pos {
                    format!("{:.03}, {:.03}", pos.x, pos.y)
                } else {
                    "".to_string()
                }
            })
            .padding(5.0)
            .expand_width(),
        );

    let tab_behavior_tree = |get: fn(&AppData) -> &Rc<String>,
                             get_mut: fn(&mut AppData) -> &mut Rc<String>,
                             file: fn(&mut AppData) -> &str| {
        Flex::column()
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(Label::new(|data: &AppData, _env: &_| data.message.clone()).padding(5.))
            .with_child(
                Flex::row()
                    .with_child(Button::new("Apply").on_click(
                        move |_, app_data: &mut AppData, _| {
                            try_load_behavior_tree(app_data, get(app_data).clone());
                        },
                    ))
                    .with_child(TextBox::new().lens(AppData::agent_source_file))
                    .with_child(Button::new("Reload from file").on_click(
                        move |_, app_data: &mut AppData, _| match std::fs::read_to_string(&file(
                            app_data,
                        )) {
                            Ok(s) => {
                                let s = Rc::new(s);
                                if try_load_behavior_tree(app_data, s.clone()) {
                                    *get_mut(app_data) = s;
                                }
                            }
                            Err(e) => app_data.message = format!("Read file error! {e:?}"),
                        },
                    )),
            )
            // For some reason, scroll box doesn't seem to allow scrolling when the text box is longer than the window height.
            .with_child(
                Scroll::new(
                    TextBox::multiline()
                        .lens(Field::new(
                            move |app_data: &AppData| get(app_data).as_ref(),
                            move |app_data| Rc::make_mut(get_mut(app_data)),
                        ))
                        .padding(5.0)
                        .fix_width(BAR_WIDTH),
                )
                .vertical(),
            )
            .background(BG)
    };

    let tabs = Tabs::new()
        .with_tab("Main", main_tab)
        .with_tab(
            "Agent Behavior tree",
            tab_behavior_tree(
                |app_data: &AppData| &app_data.agent_source_buffer,
                |app_data| &mut app_data.agent_source_buffer,
                |app_data| &app_data.agent_source_file,
            ),
        )
        .with_tab(
            "Spawner Behavior tree",
            tab_behavior_tree(
                |app_data: &AppData| &app_data.spawner_source_buffer,
                |app_data| &mut app_data.spawner_source_buffer,
                |app_data| &app_data.spawner_source_file,
            ),
        )
        .border(Color::GRAY, 2.)
        .padding(5.0);

    Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_flex_child(BoardWidget::new(), 1.)
        .with_child(tabs.fix_width(BAR_WIDTH).background(BG))
}

fn count_newlines(src: &str) -> usize {
    src.lines().count()
}

fn try_load_behavior_tree(app_data: &mut AppData, src: Rc<String>) -> bool {
    // Check the syntax before applying
    match parse_file(&src) {
        Ok(("", _)) => {
            app_data.game_params.agent_source = src.clone();
            app_data.message = format!(
                "Behavior tree applied! {}",
                Rc::strong_count(&app_data.agent_source_buffer)
            );
            true
        }
        Ok((rest, _)) => {
            let parsed_src = &src[..rest.as_ptr() as usize - src.as_ptr() as usize];
            app_data.message = format!(
                "Behavior tree source ended unexpectedly at ({}) {:?}",
                count_newlines(parsed_src),
                rest
            );
            false
        }
        Err(e) => {
            app_data.message = format!("Behavior tree failed to parse: {}", e);
            false
        }
    }
}
