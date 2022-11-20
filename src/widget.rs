use std::rc::Rc;

use crate::{
    app_data::{AppData, LineMode},
    board_widget::BoardWidget,
};
use behavior_tree_lite::parse_file;
use druid::widget::{
    Button, Checkbox, CrossAxisAlignment, Either, Flex, Label, LensWrap, RadioGroup, Scroll,
    Switch, TextBox, WidgetExt,
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
                Button::new("Create board")
                    .on_click(|ctx, data: &mut AppData, _: &Env| {
                        let xs = data.columns_text.parse().unwrap_or(64);
                        let ys = data.rows_text.parse().unwrap_or(64);
                        let seed = data.seed_text.parse().unwrap_or(1);
                        let simplify = data.simplify_text.parse().unwrap_or(1.);
                        data.game.new_board((xs, ys), seed, simplify);
                        ctx.request_paint();
                    })
                    .padding(5.0),
            )
            .with_child(
                Checkbox::new("Pause")
                    .lens(Field::new(
                        |app_data: &AppData| &app_data.game.paused,
                        |app_data| &mut app_data.game.paused,
                    ))
                    .padding(5.),
            )
            .with_child(
                Flex::row()
                    .with_child(Label::new("Border line mode:").padding(3.0))
                    .with_child(
                        RadioGroup::new([
                            ("none", LineMode::None),
                            ("line", LineMode::Line),
                            ("polygon", LineMode::Polygon),
                        ])
                        .lens(AppData::line_mode)
                        .padding(5.),
                    ),
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
                        Checkbox::new("Avoidance")
                            .lens(AppData::avoidance_visible)
                            .padding(5.),
                    ),
            )
            .with_child(
                Checkbox::new("Target line")
                    .lens(AppData::target_visible)
                    .padding(5.),
            )
            .with_child(
                Checkbox::new("Entity label")
                    .lens(AppData::entity_label_visible)
                    .padding(5.),
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
                    let profiler = data.game.triangle_profiler.borrow();
                    format!(
                        "Triangle time: {:.09}s, calls: {} size: {}, refs: {}",
                        profiler.get_average(),
                        profiler.get_count(),
                        std::mem::size_of::<AppData>(),
                        Rc::strong_count(&data.game.triangulation)
                    )
                }),
                1.,
            ))
            .with_child(Flex::row().with_flex_child(
                Label::new(|data: &AppData, _: &_| {
                    format!(
                        "Pixel time: {:.09}s, calls: {}",
                        data.game.pixel_profiler.borrow().get_average(),
                        data.game.pixel_profiler.borrow().get_count()
                    )
                }),
                1.,
            )),
        Flex::column()
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .with_child(
                Flex::row()
                    .with_child(
                        Button::new("Apply").on_click(|_, app_data: &mut AppData, _| {
                            try_load_behavior_tree(app_data, app_data.source_buffer.clone());
                        }),
                    )
                    .with_child(Button::new("Reload from file").on_click(
                        |_, app_data: &mut AppData, _| match std::fs::read_to_string(
                            "behavior_tree.txt",
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

fn try_load_behavior_tree(app_data: &mut AppData, src: Rc<String>) -> bool {
    // Check the syntax before applying
    match parse_file(&src) {
        Ok(("", _)) => {
            app_data.game.source = src.clone();
            app_data.message = format!(
                "Behavior tree applied! {}",
                Rc::strong_count(&app_data.source_buffer)
            );
            true
        }
        Ok((rest, _)) => {
            app_data.message = format!("Behavior tree source ended unexpectedly at {:?}", rest);
            false
        }
        Err(e) => {
            app_data.message = format!("Behavior tree failed to parse: {}", e);
            false
        }
    }
}
