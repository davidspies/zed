use gpui::{
    AnyWindowHandle, App, Context, Entity, FocusHandle, Focusable as _, Render, TitlebarOptions,
    WeakEntity, Window, WindowBounds, WindowOptions, point, prelude::*, px, size,
};
use platform_title_bar::PlatformTitleBar;
use release_channel::ReleaseChannel;
use settings::Settings as _;
use theme_settings::ThemeSettings;
use ui::prelude::*;
use util::ResultExt as _;
use workspace::{WorkspaceSettings, client_side_decorations};

use crate::{
    element::{EditorElement, SplitSide},
    split::SplittableEditor,
};

pub(crate) fn open_lhs_popout_window(
    splittable_editor: Entity<SplittableEditor>,
    source_window: AnyWindowHandle,
    cx: &mut App,
) {
    // We have to defer this to get the current entity update off the stack.
    cx.defer(move |cx| {
        let current_rem_size: f32 = ThemeSettings::get_global(cx).ui_font_size(cx).into();
        let default_rem_size = 16.0;
        let scale_factor = current_rem_size / default_rem_size;
        let scaled_bounds = size(px(720.0), px(880.0)).map(|axis| axis * scale_factor);

        let app_id = ReleaseChannel::global(cx).app_id();
        let window_decorations = match std::env::var("ZED_WINDOW_DECORATIONS") {
            Ok(val) if val == "server" => gpui::WindowDecorations::Server,
            Ok(val) if val == "client" => gpui::WindowDecorations::Client,
            _ => match WorkspaceSettings::get_global(cx).window_decorations {
                settings::WindowDecorations::Server => gpui::WindowDecorations::Server,
                settings::WindowDecorations::Client => gpui::WindowDecorations::Client,
            },
        };

        cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Zed — Diff: Original".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(12.0))),
                }),
                focus: true,
                show: true,
                is_movable: true,
                kind: gpui::WindowKind::Normal,
                window_background: cx.theme().window_background_appearance(),
                app_id: Some(app_id.to_owned()),
                window_decorations: Some(window_decorations),
                window_min_size: Some(size(px(400.0), px(320.0))),
                window_bounds: Some(WindowBounds::centered(scaled_bounds, cx)),
                ..Default::default()
            },
            |window, cx| {
                cx.new(|cx| {
                    SplitDiffPopoutWindow::new(splittable_editor, source_window, window, cx)
                })
            },
        )
        .log_err();
    });
}

struct SplitDiffPopoutWindow {
    title_bar: Option<Entity<PlatformTitleBar>>,
    splittable_editor: WeakEntity<SplittableEditor>,
    source_window: AnyWindowHandle,
    focus_handle: FocusHandle,
}

impl SplitDiffPopoutWindow {
    fn new(
        splittable_editor: Entity<SplittableEditor>,
        source_window: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_bar = (!cfg!(target_os = "macos"))
            .then(|| cx.new(|cx| PlatformTitleBar::new("split-diff-popout-title-bar", cx)));

        splittable_editor.update(cx, |splittable_editor, cx| {
            splittable_editor.set_lhs_popout_window(Some(window.window_handle()), cx);
        });
        if let Some(lhs_editor) = splittable_editor.read(cx).lhs_editor().cloned() {
            lhs_editor.focus_handle(cx).focus(window, cx);
        }

        // The popped-out side has no meaning once the diff item is gone.
        cx.observe_release_in(&splittable_editor, window, |_, _, window, _| {
            window.remove_window();
        })
        .detach();

        // When this window closes, fold the lhs back into the main window's
        // side-by-side view. Ignore failures: the diff or the source window
        // may already be gone (e.g. when quitting the app).
        cx.on_release(|this, cx| {
            this.splittable_editor
                .update(cx, |splittable_editor, cx| {
                    splittable_editor.set_lhs_popout_window(None, cx);
                })
                .ok();
            this.source_window
                .update(cx, |_, window, _| window.activate_window())
                .ok();
        })
        .detach();

        Self {
            title_bar,
            splittable_editor: splittable_editor.downgrade(),
            source_window,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for SplitDiffPopoutWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_font = theme_settings::setup_ui_font(window, cx);

        let lhs_element = self.splittable_editor.upgrade().and_then(|splittable| {
            let splittable = splittable.read(cx);
            let lhs_editor = splittable.lhs_editor()?;
            let style = splittable.rhs_editor().read(cx).create_style(cx);
            let mut element = EditorElement::new(lhs_editor, style);
            element.set_split_side(SplitSide::Left);
            Some(element)
        });

        client_side_decorations(
            v_flex()
                .size_full()
                .font(ui_font)
                .text_color(cx.theme().colors().text)
                .bg(cx.theme().colors().editor_background)
                .children(self.title_bar.clone())
                .child(
                    div()
                        .id("split-diff-popout-window")
                        .key_context("SplitDiffPopoutWindow")
                        .track_focus(&self.focus_handle)
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .overflow_hidden()
                        .children(lhs_element),
                ),
            window,
            cx,
        )
    }
}
