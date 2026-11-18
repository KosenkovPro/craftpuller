//! Значок в системном трее и меню трея.

use crate::icon::{make_rgba, IconKind};
use crate::lang::t;
use muda::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

pub const ID_SHOW: &str = "show";
pub const ID_PAUSE: &str = "pause";
pub const ID_STOP: &str = "stop";
pub const ID_EXIT: &str = "exit";

pub struct Tray {
    pub show_item: MenuItem,
    pub pause_item: MenuItem,
    pub stop_item: MenuItem,
    pub exit_item: MenuItem,
    pub icon: TrayIcon,
    pub last_kind: IconKind,
}

impl Tray {
    pub fn build() -> Result<Self, String> {
        let menu = Menu::new();

        let show_item = MenuItem::with_id(MenuId::from(ID_SHOW), t("tray.show"), true, None);
        let pause_item = MenuItem::with_id(MenuId::from(ID_PAUSE), t("tray.pause"), true, None);
        let stop_item = MenuItem::with_id(MenuId::from(ID_STOP), t("tray.stop"), true, None);
        let exit_item = MenuItem::with_id(MenuId::from(ID_EXIT), t("tray.exit"), true, None);
        let sep = PredefinedMenuItem::separator();

        menu.append(&show_item).map_err(|e| format!("{e}"))?;
        menu.append(&pause_item).map_err(|e| format!("{e}"))?;
        menu.append(&stop_item).map_err(|e| format!("{e}"))?;
        menu.append(&sep).map_err(|e| format!("{e}"))?;
        menu.append(&exit_item).map_err(|e| format!("{e}"))?;

        let rgba = make_rgba(32, IconKind::Idle);
        let tray_icon_img = tray_icon::Icon::from_rgba(rgba, 32, 32)
            .map_err(|e| crate::lang::tf("err.tray", &[("{e}", &format!("{e:?}"))]))?;

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu) as Box<dyn muda::ContextMenu>)
            .with_tooltip(t("tray.tooltip_idle"))
            .with_icon(tray_icon_img)
            .with_menu_on_left_click(false)
            .build()
            .map_err(|e| crate::lang::tf("err.tray", &[("{e}", &e.to_string())]))?;

        Ok(Self {
            show_item,
            pause_item,
            stop_item,
            exit_item,
            icon: tray,
            last_kind: IconKind::Idle,
        })
    }

    /// Обновить иконку в трее (цвет зависит от состояния).
    pub fn set_kind(&mut self, kind: IconKind) {
        if kind == self.last_kind {
            return;
        }
        self.last_kind = kind;
        let rgba = make_rgba(32, kind);
        if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, 32, 32) {
            let _ = self.icon.set_icon(Some(icon));
        }
    }

    /// Обновить подсказку при наведении.
    pub fn set_tooltip(&self, text: &str) {
        let _ = self.icon.set_tooltip(Some(text));
    }

    /// Обновить подписи пунктов меню (после смены языка).
    pub fn set_texts(&self) {
        let _ = self.show_item.set_text(t("tray.show"));
        let _ = self.pause_item.set_text(t("tray.pause"));
        let _ = self.stop_item.set_text(t("tray.stop"));
        let _ = self.exit_item.set_text(t("tray.exit"));
    }

    /// Обновить подписи пунктов меню.
    pub fn set_paused(&self, paused: bool) {
        let _ = self
            .pause_item
            .set_text(if paused { t("tray.resume") } else { t("tray.pause") });
    }
}

/// Забрать события меню трея (не блокируя).
pub fn poll_menu_events() -> Vec<muda::MenuEvent> {
    let mut out = Vec::new();
    while let Ok(ev) = MenuEvent::receiver().try_recv() {
        out.push(ev);
    }
    out
}

/// Забрать клики по значку в трее (не блокируя).
pub fn poll_tray_events() -> Vec<TrayIconEvent> {
    let mut out = Vec::new();
    while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
        out.push(ev);
    }
    out
}
