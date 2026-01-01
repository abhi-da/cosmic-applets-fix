// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

mod config;
mod localize;
mod mouse_area;
mod mpris_subscription;

use crate::localize::localize;
use config::{amplification_sink, amplification_source, AudioAppletConfig};
use cosmic::{
    applet::{menu_button, padded_control},
    cctk::sctk::reexports::calloop,
    cosmic_theme::Spacing,
    iced::{
        self,
        futures::StreamExt,
        platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup},
        widget::{column, row, slider, image},
        window, Alignment, Length,
    },
    theme,
    widget::{button, container, divider, horizontal_space, icon, text, Row},
    Element, Task, Theme,
};
use cosmic::iced::Renderer;

use cosmic_settings_sound_subscription as css;
use cosmic_time::{Instant, Timeline};
use mpris_subscription::{MprisRequest, MprisUpdate};
use mpris2_zbus::player::PlaybackStatus;
use std::process::Command;

// Icons
const GO_BACK: &str = "media-skip-backward-symbolic";
const GO_NEXT: &str = "media-skip-forward-symbolic";
const PAUSE: &str = "media-playback-pause-symbolic";
const PLAY: &str = "media-playback-start-symbolic";

pub fn run() -> cosmic::iced::Result {
    localize();
    cosmic::applet::run::<Audio>(())
}

#[derive(Default)]
pub struct Audio {
    core: cosmic::app::Core,
    popup: Option<window::Id>,
    model: css::Model,
    is_open: IsOpen,
    max_sink_volume: u32,
    max_source_volume: u32,
    sink_breakpoints: &'static [u32],
    source_breakpoints: &'static [u32],
    timeline: Timeline,
    config: AudioAppletConfig,
    player_status: Option<mpris_subscription::PlayerStatus>,
    token_tx: Option<calloop::channel::Sender<cosmic::applet::token::subscription::TokenRequest>>,
    
    // SAFE DRAG STATES
    sink_drag_val: Option<u32>,
    source_drag_val: Option<u32>,
    last_update: Option<Instant>,
}

#[derive(Debug, PartialEq, Eq, Default)]
enum IsOpen { #[default] None, Output, Input }

#[derive(Debug, Clone)]
pub enum Message {
    Ignore,
    SetSinkVolume(u32), DragSink(u32), CommitSink, ToggleSinkMute,
    SetSourceVolume(u32), DragSource(u32), CommitSource, ToggleSourceMute,
    SetDefaultSink(usize), SetDefaultSource(usize), OutputToggle, InputToggle,
    TogglePopup,
    CloseRequested(window::Id),
    ConfigChanged(AudioAppletConfig),
    Mpris(MprisUpdate), MprisRequest(MprisRequest),
    OpenSettings,
    Subscription(css::Message),
    Frame(Instant),
}

impl cosmic::Application for Audio {
    type Message = Message;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    const APP_ID: &'static str = "com.usr.AudioApplet";

    fn init(core: cosmic::app::Core, _flags: ()) -> (Self, Task<cosmic::Action<Self::Message>>) {
        (
            Self {
                core,
                model: css::Model::default(),
                ..Default::default()
            },
            Task::none(),
        )
    }

    fn core(&self) -> &cosmic::app::Core { &self.core }
    fn core_mut(&mut self) -> &mut cosmic::app::Core { &mut self.core }
    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> { Some(cosmic::applet::style()) }

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::Frame(now) => self.timeline.now(now),
            
            // --- SAFE AUDIO VOLUME (WPCTL) ---
            Message::DragSink(val) => { self.sink_drag_val = Some(val); self.model.sink_volume_text = format!("{}%", val); }
            Message::DragSource(val) => { self.source_drag_val = Some(val); self.model.source_volume_text = format!("{}%", val); }
            
            Message::CommitSink => {
                if let Some(val) = self.sink_drag_val.take() {
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{:.2}", val as f32 / 100.0)]).spawn();
                }
            }
            Message::CommitSource => {
                if let Some(val) = self.source_drag_val.take() {
                    let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SOURCE@", &format!("{:.2}", val as f32 / 100.0)]).spawn();
                }
            }
            Message::SetSinkVolume(val) => {
                if let Some(last) = self.last_update { if last.elapsed().as_millis() < 50 { return Task::none(); } }
                self.last_update = Some(Instant::now());
                let _ = Command::new("wpctl").args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{:.2}", val as f32 / 100.0)]).spawn();
            }
            
            Message::ToggleSinkMute => { let _ = Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]).spawn(); }
            Message::ToggleSourceMute => { let _ = Command::new("wpctl").args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", "toggle"]).spawn(); }
            
            Message::SetDefaultSink(idx) => return self.model.set_default_sink(idx).map(|m| cosmic::Action::from(Message::Subscription(m))),
            Message::SetDefaultSource(idx) => return self.model.set_default_source(idx).map(|m| cosmic::Action::from(Message::Subscription(m))),

            // --- MEDIA CONTROL ---
            Message::MprisRequest(req) => {
                match req {
                    MprisRequest::Play => { let _ = Command::new("playerctl").arg("play").spawn(); },
                    MprisRequest::Pause => { let _ = Command::new("playerctl").arg("pause").spawn(); },
                    MprisRequest::Next => { let _ = Command::new("playerctl").arg("next").spawn(); },
                    MprisRequest::Previous => { let _ = Command::new("playerctl").arg("previous").spawn(); },
                    MprisRequest::Raise => {},
                }
            }

            Message::OpenSettings => {
                let _ = Command::new("cosmic-settings").arg("sound").spawn();
            }

            Message::Subscription(m) => return self.model.update(m).map(|m| cosmic::Action::from(Message::Subscription(m))),
            Message::Mpris(MprisUpdate::Player(p)) => self.player_status = Some(p),
            Message::Mpris(MprisUpdate::Finished | MprisUpdate::Setup) => self.player_status = None,
            Message::ConfigChanged(c) => self.config = c,
            
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() { return destroy_popup(p); }
                let new_id = window::Id::unique();
                self.popup.replace(new_id);
                self.timeline = Timeline::new();
                (self.max_sink_volume, self.sink_breakpoints) = if amplification_sink() { (150, &[100][..]) } else { (100, &[][..]) };
                (self.max_source_volume, self.source_breakpoints) = if amplification_source() { (150, &[100][..]) } else { (100, &[][..]) };
                return get_popup(self.core.applet.get_popup_settings(self.core.main_window_id().unwrap(), new_id, None, None, None));
            }
            Message::OutputToggle => self.is_open = if self.is_open == IsOpen::Output { IsOpen::None } else { IsOpen::Output },
            Message::InputToggle => self.is_open = if self.is_open == IsOpen::Input { IsOpen::None } else { IsOpen::Input },
            Message::CloseRequested(id) => if Some(id) == self.popup { self.popup = None; },
            _ => {}
        }
        Task::none()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            self.timeline.as_subscription().map(|(_, now)| Message::Frame(now)),
            self.core.watch_config(Self::APP_ID).map(|u| Message::ConfigChanged(u.config)),
            mpris_subscription::mpris_subscription(0).map(Message::Mpris),
            iced::Subscription::run(|| css::watch().map(Message::Subscription)),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let btn = self.core.applet.icon_button(self.output_icon_name()).on_press_down(Message::TogglePopup);
        let btn = crate::mouse_area::MouseArea::new(btn).on_mouse_wheel(|delta| {
            let y = match delta { iced::mouse::ScrollDelta::Lines { y, .. } => y, iced::mouse::ScrollDelta::Pixels { y, .. } => y.signum() };
            let new_vol = (self.model.sink_volume as i32 + (y * 5.0) as i32).clamp(0, 100) as u32;
            Message::SetSinkVolume(new_vol)
        });
        self.core.applet.autosize_window(Element::from(btn)).into()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        let Spacing { space_xxs, space_s, .. } = theme::active().cosmic().spacing;
        
        let sink_vol = self.sink_drag_val.unwrap_or(self.model.sink_volume);
        let source_vol = self.source_drag_val.unwrap_or(self.model.source_volume);
        
        // --- 1. OUTPUT VOLUME ---
        let mut content = column![
            padded_control(row![
                button::icon(icon::from_name(self.output_icon_name()).size(24).symbolic(true))
                    .class(cosmic::theme::Button::Icon).on_press(Message::ToggleSinkMute),
                slider(0..=self.max_sink_volume, sink_vol, Message::DragSink)
                    .width(Length::FillPortion(5)).breakpoints(self.sink_breakpoints)
                    .on_release(Message::CommitSink),
                container(text(format!("{}%", sink_vol)).size(16)).width(Length::FillPortion(1)).align_x(Alignment::End)
            ].spacing(12).align_y(Alignment::Center)),
            
            revealer(self.is_open == IsOpen::Output, fl!("output"), 
                self.model.active_sink().and_then(|i| self.model.sinks().get(i)).cloned().unwrap_or("No Device".into()), 
                self.model.sinks(), Message::OutputToggle, Message::SetDefaultSink)
        ];

        // --- 2. INPUT VOLUME ---
        content = content.push(padded_control(divider::horizontal::default()).padding([space_xxs, space_s]));
        content = content.push(column![
             padded_control(row![
                button::icon(icon::from_name(self.input_icon_name()).size(24).symbolic(true))
                    .class(cosmic::theme::Button::Icon).on_press(Message::ToggleSourceMute),
                slider(0..=self.max_source_volume, source_vol, Message::DragSource)
                    .width(Length::FillPortion(5)).breakpoints(self.source_breakpoints)
                    .on_release(Message::CommitSource),
                container(text(format!("{}%", source_vol)).size(16)).width(Length::FillPortion(1)).align_x(Alignment::End)
            ].spacing(12).align_y(Alignment::Center)),
            
            revealer(self.is_open == IsOpen::Input, fl!("input"), 
                self.model.active_source().and_then(|i| self.model.sources().get(i)).cloned().unwrap_or("No Device".into()), 
                self.model.sources(), Message::InputToggle, Message::SetDefaultSource)
        ]);

        // --- 3. VERTICAL MEDIA WIDGET ---
        if let Some(s) = self.player_status.as_ref() {
             content = content.push(padded_control(divider::horizontal::default()).padding([space_xxs, space_s]));
             
             // ROW 1: ALBUM ART (Full Width + Margin)
             let art = if let Some(path) = s.icon.clone() {
                 // Length::Fill makes it fill the container, Padding creates the margin
                 container(image(path).width(Length::Fill)).padding([0, 24]) 
             } else {
                 container(icon::from_name("audio-x-generic-symbolic").size(96))
             };
             
             // ROW 2: CONTROLS
             let mut controls = Vec::new();
             if s.can_go_previous { controls.push(media_btn(GO_BACK, Message::MprisRequest(MprisRequest::Previous))); }
             let (icon_name, action) = match s.status {
                PlaybackStatus::Playing => (PAUSE, MprisRequest::Pause),
                _ => (PLAY, MprisRequest::Play)
             };
             controls.push(media_btn(icon_name, Message::MprisRequest(action)));
             if s.can_go_next { controls.push(media_btn(GO_NEXT, Message::MprisRequest(MprisRequest::Next))); }
             let controls_row = Row::with_children(controls).spacing(16).align_y(Alignment::Center);
             
             // ROW 3: TITLE
             let title_text = text::body(s.title.clone().unwrap_or_default());

             // ROW 4: ARTIST
             let artist_text = text::caption(s.artists.as_ref().map(|a| a.join(", ")).unwrap_or_else(|| fl!("unknown-artist")));

             let media_column = column![
                 art,
                 controls_row,
                 title_text,
                 artist_text
             ].spacing(12).align_x(Alignment::Center).width(Length::Fill);
             
             content = content.push(padded_control(media_column));
        }

        // --- 4. FOOTER ---
        content = content.push(padded_control(divider::horizontal::default()).padding([space_xxs, space_s]))
             .push(menu_button(text::body(fl!("sound-settings"))).on_press(Message::OpenSettings));

        self.core.applet.popup_container(container(content.align_x(Alignment::Start).padding([8, 0]))).into()
    }
}

impl Audio {
    fn output_icon_name(&self) -> &'static str {
        let v = self.sink_drag_val.unwrap_or(self.model.sink_volume);
        if self.model.sink_mute || v == 0 { "audio-volume-muted-symbolic" } else if v < 33 { "audio-volume-low-symbolic" } else if v < 66 { "audio-volume-medium-symbolic" } else { "audio-volume-high-symbolic" }
    }
    fn input_icon_name(&self) -> &'static str {
        let v = self.source_drag_val.unwrap_or(self.model.source_volume);
        if self.model.source_mute || v == 0 { "microphone-sensitivity-muted-symbolic" } else if v < 33 { "microphone-sensitivity-low-symbolic" } else if v < 66 { "microphone-sensitivity-medium-symbolic" } else { "microphone-sensitivity-high-symbolic" }
    }
}

fn revealer(open: bool, title: String, sel: String, devs: &[String], toggle: Message, mut change: impl FnMut(usize) -> Message + 'static) -> cosmic::iced::widget::Column<'static, Message, Theme, Renderer> {
    let head = menu_button(column![text::body(title).width(Length::Fill), text::caption(sel)]).on_press(toggle);
    if open { 
        devs.iter().enumerate().fold(column![head].width(Length::Fill), |c, (i, n)| c.push(menu_button(text::body(n.clone())).on_press(change(i)).width(Length::Fill).padding([8, 48]))) 
    } else { column![head] }
}

fn media_btn(name: &'static str, msg: Message) -> Element<'static, Message> {
    button::icon(icon::from_name(name).size(32).symbolic(true)).extra_small().class(cosmic::theme::Button::AppletIcon).on_press(msg).into()
}
