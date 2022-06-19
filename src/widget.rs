use crate::{
    app_data::{AppData, LineMode},
    board_widget::BoardWidget,
};
use druid::widget::prelude::*;
use druid::widget::{
    Button, Checkbox, CrossAxisAlignment, Flex, Label, RadioGroup, TextBox, WidgetExt,
};
use druid::Color;

const BG: Color = Color::rgb8(0, 0, 53 as u8);

pub(crate) fn make_widget() -> impl Widget<AppData> {
    Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_flex_child(BoardWidget::new(), 1.)
        .with_flex_child(
            Flex::column()
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    Flex::row().with_child(
                        Button::new("Create board")
                            .on_click(|ctx, data: &mut AppData, _: &Env| {
                                let xs = data.columns_text.parse().unwrap_or(64);
                                let ys = data.rows_text.parse().unwrap_or(64);
                                let seed = data.seed_text.parse().unwrap_or(1);
                                let simplify = data.simplify_text.parse().unwrap_or(1.);
                                data.new_board((xs, ys), seed, simplify);
                                ctx.request_paint();
                            })
                            .padding(5.0),
                    ),
                )
                .with_child(
                    // Validity label
                    Label::new(|data: &AppData, _env: &_| data.message.clone()).padding(5.),
                )
                .with_child(Checkbox::new("Pause").lens(AppData::paused).padding(5.))
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
                        .with_child(
                            Checkbox::new("Triangulation").lens(AppData::triangulation_visible),
                        )
                        .with_child(Checkbox::new("Unpassable").lens(AppData::unpassable_visible))
                        .with_child(Checkbox::new("Label").lens(AppData::triangle_label_visible))
                        .padding(5.),
                )
                .with_child(
                    Checkbox::new("Path")
                        .lens(AppData::path_visible)
                        .padding(5.),
                )
                .with_child(
                    Checkbox::new("Target line")
                        .lens(AppData::target_visible)
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
                    Label::new(|data: &AppData, _: &_| data.render_stats.borrow().clone()),
                    1.,
                ))
                .with_child(Flex::row().with_flex_child(
                    Label::new(|data: &AppData, _: &_| {
                        format!(
                            "Render board: {:.06}s, Get board: {:.06}s",
                            data.render_board_time.get(),
                            &data.get_board_time
                        )
                    }),
                    1.,
                ))
                .fix_width(400.)
                .background(BG)
                .expand_height(),
            0.,
        )
}
