use crate::{
    app_data::{AppData, LineMode},
    board_widget::MeshWidget,
};
use druid::widget::prelude::*;
use druid::widget::{Button, CrossAxisAlignment, Flex, Label, RadioGroup, TextBox, WidgetExt};
use druid::Color;
use std::rc::Rc;

const BG: Color = Color::rgb8(0, 0, 53 as u8);

pub(crate) fn make_widget() -> impl Widget<AppData> {
    Flex::row()
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .with_flex_child(MeshWidget::new(), 1.)
        .with_flex_child(
            Flex::column()
                .cross_axis_alignment(CrossAxisAlignment::Start)
                .with_child(
                    Flex::row()
                        .with_child(
                            Button::new("Create mesh")
                                .on_click(|ctx, data: &mut AppData, _: &Env| {
                                    data.message =
                                        "Define transform mesh by dragging vertices".to_string();
                                    data.xs = data.columns_text.parse().unwrap_or(64);
                                    data.ys = data.rows_text.parse().unwrap_or(64);
                                    let (board, simplified_border) = AppData::create_board(
                                        (data.xs, data.ys),
                                        data.seed_text.parse().unwrap_or(1),
                                        data.simplify_text.parse().unwrap_or(1.),
                                    );
                                    *Rc::make_mut(&mut data.board) = board;
                                    *Rc::make_mut(&mut data.simplified_border) = simplified_border;
                                    ctx.request_paint();
                                })
                                .padding(5.0),
                        )
                        .with_child(
                            Button::new("Cancel")
                                .on_click(|ctx, data: &mut AppData, _: &Env| {
                                    data.message = "".to_string();
                                    ctx.request_paint();
                                })
                                .padding(5.0),
                        ),
                )
                .with_child(
                    // Validity label
                    Label::new(|data: &AppData, _env: &_| data.message.clone()).padding(5.),
                )
                .with_child(
                    Flex::row()
                        .with_child(Label::new("Line mode:").padding(3.0))
                        .with_child(
                            RadioGroup::new([
                                ("line", LineMode::Line),
                                ("polygon", LineMode::Polygon),
                            ])
                            .lens(AppData::line_mode)
                            .padding(5.),
                        ),
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
