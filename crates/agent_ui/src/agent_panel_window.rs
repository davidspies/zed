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
use workspace::{Workspace, WorkspaceSettings, client_side_decorations, dock::Panel as _};

use crate::AgentPanel;

pub(crate) fn open_agent_panel_window(
    workspace: &mut Workspace,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) {
    let Some(panel) = workspace.panel::<AgentPanel>(cx) else {
        return;
    };
    let position = panel.read(cx).position(window, cx);
    let dock = workspace.dock_at_position(position).clone();
    dock.update(cx, |dock, cx| dock.set_open(false, window, cx));
    AgentPanelWindow::open(workspace.weak_handle(), window.window_handle(), cx);
}

struct AgentPanelWindow {
    title_bar: Option<Entity<PlatformTitleBar>>,
    workspace: WeakEntity<Workspace>,
    source_window: AnyWindowHandle,
    agent_panel: Option<Entity<AgentPanel>>,
    focus_handle: FocusHandle,
}

impl AgentPanelWindow {
    fn open(workspace: WeakEntity<Workspace>, source_window: AnyWindowHandle, cx: &mut App) {
        for window_handle in cx.windows() {
            let Some(existing) = window_handle.downcast::<Self>() else {
                continue;
            };
            let activated = existing
                .update(cx, |agent_panel_window, window, _cx| {
                    if agent_panel_window.workspace.entity_id() == workspace.entity_id() {
                        window.activate_window();
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false);
            if activated {
                return;
            }
        }

        // We have to defer this to get the workspace off the stack.
        cx.defer(move |cx| {
            let current_rem_size: f32 = ThemeSettings::get_global(cx).ui_font_size(cx).into();
            let default_rem_size = 16.0;
            let scale_factor = current_rem_size / default_rem_size;
            let scaled_bounds = size(px(560.0), px(880.0)).map(|axis| axis * scale_factor);

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
                        title: Some("Zed — Agent Panel".into()),
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
                |window, cx| cx.new(|cx| Self::new(workspace, source_window, window, cx)),
            )
            .log_err();
        });
    }

    fn new(
        workspace: WeakEntity<Workspace>,
        source_window: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title_bar = (!cfg!(target_os = "macos"))
            .then(|| cx.new(|cx| PlatformTitleBar::new("agent-panel-window-title-bar", cx)));

        cx.spawn_in(window, {
            let workspace = workspace.clone();
            async move |this, cx| {
                let panel = AgentPanel::load(workspace.clone(), cx.clone()).await?;
                this.update_in(cx, |this, window, cx| {
                    panel.update(cx, |panel, cx| {
                        panel.set_active(true, window, cx);
                        panel.initialize_from_source_workspace_if_needed(workspace, window, cx);
                    });
                    panel.focus_handle(cx).focus(window, cx);
                    this.agent_panel = Some(panel);
                    cx.notify();
                })
            }
        })
        .detach_and_log_err(cx);

        // When this window closes, reveal the panel in the source workspace's
        // dock again. Ignore failures: the source window or workspace may
        // already be gone (e.g. when quitting the app).
        cx.on_release(|this, cx| {
            let workspace = this.workspace.clone();
            this.source_window
                .update(cx, |_, window, cx| {
                    workspace
                        .update(cx, |workspace, cx| {
                            workspace.focus_panel::<AgentPanel>(window, cx);
                        })
                        .ok();
                })
                .ok();
        })
        .detach();

        Self {
            title_bar,
            workspace,
            source_window,
            agent_panel: None,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for AgentPanelWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_font = theme_settings::setup_ui_font(window, cx);

        client_side_decorations(
            v_flex()
                .size_full()
                .font(ui_font)
                .text_color(cx.theme().colors().text)
                .bg(cx.theme().colors().panel_background)
                .children(self.title_bar.clone())
                .child(
                    div()
                        .id("agent-panel-window")
                        .key_context("AgentPanelWindow")
                        .track_focus(&self.focus_handle)
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .children(self.agent_panel.clone()),
                ),
            window,
            cx,
        )
    }
}
