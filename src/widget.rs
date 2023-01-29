use std::rc::Rc;

use crate::{
    agent::AvoidanceRenderParams,
    app_data::{AppData, LineMode},
    board_widget::BoardWidget,
    game::{AvoidanceMode, BoardParams, BoardType},
};
use behavior_tree_lite::parse_file;
use druid::widget::{
    Button, Checkbox, CrossAxisAlignment, Either, Flex, Label, LensWrap, Radio, RadioGroup, Scroll,
    Slider, Switch, TextBox, WidgetExt,
};
use druid::Color;
use druid::{lens::Field, widget::prelude::*};

const BG: Color = Color::rgb8(0, 0, 53 as u8);
const BAR_WIDTH: f64 = 400.;

pub(crate) fn make_widget() -> impl Widget<AppData> {
    let either = Either::new(
        |data, _| !data.source_visible,
        Flex::column()
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                Flex::row()
                    .with_child(
                        Button::new("Create board")
                            .on_click(|ctx, data: &mut AppData, _: &Env| {
                                let xs = data.columns_text.parse().unwrap_or(64);
                                let ys = data.rows_text.parse().unwrap_or(64);
                                let seed = data.seed_text.parse().unwrap_or(1);
                                let simplify = data.simplify_text.parse().unwrap_or(1.);
                                let params = BoardParams {
                                    shape: (xs, ys),
                                    seed,
                                    simplify,
                                    maze_expansions: data.maze_expansions.parse().unwrap_or(1),
                                };
                                data.game.borrow_mut().new_board(data.board_type, &params);
                                ctx.request_paint();
                            })
                            .padding(5.0),
                    )
                    .with_child(Radio::new("Rect", BoardType::Rect).lens(AppData::board_type))
                    .with_child(Radio::new("Crank", BoardType::Crank).lens(AppData::board_type))
                    .with_child(Radio::new("Perlin", BoardType::Perlin).lens(AppData::board_type))
                    .with_child(Radio::new("Maze", BoardType::Maze).lens(AppData::board_type)),
            )
            .with_child(
                Checkbox::new("Pause")
                    .lens(Field::new(
                        |app_data: &AppData| &app_data.game_params.paused,
                        |app_data| &mut app_data.game_params.paused,
                    ))
                    .padding(5.),
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
                        RadioGroup::new([
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
                        Flex::column()
                            .cross_axis_alignment(CrossAxisAlignment::Start)
                            .with_child(Label::new(|data: &AppData, _: &_| {
                                format!("Expands: {:.0}", data.game_params.avoidance_expands)
                            }))
                            .with_child(Slider::new().with_range(1., 20.).lens(Field::new(
                                |data: &AppData| &data.game_params.avoidance_expands,
                                |data: &mut AppData| &mut data.game_params.avoidance_expands,
                            ))),
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
            .with_child(Flex::row().with_flex_child(
                Label::new(|data: &AppData, _: &_| format!("Scale: {}", data.scale)),
                1.,
            ))
            .with_child(Flex::row().with_flex_child(
                Label::new(|data: &AppData, _: &_| data.render_stats.borrow().clone()),
                1.,
            ))
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
                        "QTree time: {:.06}ms, calls: {}",
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
            ),
        Flex::column()
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                Flex::row()
                    .with_child(
                        Button::new("Apply").on_click(|_, app_data: &mut AppData, _| {
                            try_load_behavior_tree(app_data, app_data.source_buffer.clone());
                        }),
                    )
                    .with_child(TextBox::new().lens(AppData::source_file))
                    .with_child(Button::new("Reload from file").on_click(
                        |_, app_data: &mut AppData, _| match std::fs::read_to_string(
                            &app_data.source_file,
                        ) {
                            Ok(s) => {
                                let s = Rc::new(s);
                                if try_load_behavior_tree(app_data, s.clone()) {
                                    app_data.source_buffer = s;
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
                            |app_data: &AppData| app_data.source_buffer.as_ref(),
                            |app_data| Rc::make_mut(&mut app_data.source_buffer),
                        ))
                        .padding(5.0)
                        .fix_width(BAR_WIDTH),
                )
                .vertical(),
            )
            .background(BG),
    );

    Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_flex_child(
            BoardWidget::new(),
            //     .with_flex_child(either/*.with_child()*/, 0.5),
            1.,
        )
        .with_flex_child(
            Flex::column()
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    Flex::row()
                        .with_child(
                            LensWrap::new(Switch::new(), AppData::source_visible).padding(5.0),
                        )
                        .with_child(Label::new("Behavior tree editor"))
                        .padding(5.0),
                )
                .with_child(
                    // Validity label
                    Label::new(|data: &AppData, _env: &_| data.message.clone()).padding(5.),
                )
                .with_child(either)
                .fix_width(BAR_WIDTH)
                .background(BG)
                .expand_height(),
            0.,
        )
}

fn count_newlines(src: &str) -> usize {
    src.lines().count()
}

fn try_load_behavior_tree(app_data: &mut AppData, src: Rc<String>) -> bool {
    // Check the syntax before applying
    match parse_file(&src) {
        Ok(("", _)) => {
            app_data.game_params.source = src.clone();
            app_data.message = format!(
                "Behavior tree applied! {}",
                Rc::strong_count(&app_data.source_buffer)
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
