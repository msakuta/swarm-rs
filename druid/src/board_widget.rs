use crate::{app_data::AppData, paint_board::paint_game};
use druid::widget::prelude::*;
use druid::{Affine, TimerToken, Vec2};
use std::time::Duration;

pub(crate) struct BoardWidget {
    timer_id: TimerToken,
    panning: Option<Vec2>,
}

impl BoardWidget {
    pub(crate) fn new() -> Self {
        Self {
            panning: None,
            timer_id: TimerToken::INVALID,
        }
    }
}

impl AppData {
    pub(crate) fn view_transform(&self) -> Affine {
        Affine::scale(self.scale) * Affine::translate(self.origin)
    }

    pub(crate) fn inverse_view_transform(&self) -> Affine {
        Affine::translate(-self.origin) * Affine::scale(1. / self.scale)
    }
}

impl Widget<AppData> for BoardWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppData, _env: &Env) {
        match event {
            Event::WindowConnected => {
                ctx.request_paint();
                let deadline = Duration::from_millis(data.game.borrow().interval as u64);
                self.timer_id = ctx.request_timer(deadline);
            }
            Event::Timer(id) => {
                if *id == self.timer_id {
                    let (paused, interval) = data.update();
                    if !paused {
                        ctx.request_paint();
                    }
                    data.big_message_time = (data.big_message_time - interval).max(0.);
                    let deadline = Duration::from_millis(interval as u64);
                    self.timer_id = ctx.request_timer(deadline);
                }
            }
            Event::MouseDown(e) => {
                self.panning = Some(e.pos.to_vec2());
            }
            Event::MouseUp(_e) => {
                self.panning = None;
            }
            Event::MouseMove(e) => {
                let affine = data.inverse_view_transform();
                data.mouse_pos = Some(affine * e.pos);
                if let Some(ref mut panning) = self.panning {
                    let newpos = Vec2::new(e.pos.x, e.pos.y);
                    let delta = (newpos - *panning) / data.scale;
                    data.origin += delta;
                    *panning = newpos;
                    return;
                }
            }
            Event::Wheel(e) => {
                let old_offset = data.view_transform().inverse() * e.pos;
                if e.wheel_delta.y < 0. {
                    data.scale *= 1.2;
                } else if 0. < e.wheel_delta.y {
                    data.scale /= 1.2;
                }
                let new_offset = data.view_transform().inverse() * e.pos;
                let diff = new_offset.to_vec2() - old_offset.to_vec2();
                data.origin += diff;
            }
            _ => {}
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        _event: &LifeCycle,
        _data: &AppData,
        _env: &Env,
    ) {
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &AppData, _data: &AppData, _env: &Env) {
        ctx.request_paint();
    }

    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &AppData,
        _env: &Env,
    ) -> Size {
        bc.max()
        // let max_size = bc.max();
        // let min_side = max_size.height.min(max_size.width);//.min(800.);
        // Size {
        //     width: min_side,
        //     height: min_side,
        // }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppData, env: &Env) {
        paint_game(ctx, data, env);
    }
}
