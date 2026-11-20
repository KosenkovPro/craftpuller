//! Карточка-герой: клик по плитке разворачивает её на всю ширину.
//!
//! Раскладка считается вручную, без `egui::Grid`: сетка пересчитывает строки
//! по фактической высоте, поэтому соседние плитки перетекают ступенчато. Здесь
//! у каждой карточки две целевые позиции — «плитка» и «герой», — а между ними
//! значение плавно интерполируется пружиной. Получается общий элемент: выбранная
//! карточка перетекает в широкую панель, остальные уезжают вниз.

use std::collections::HashMap;
use std::time::Instant;

use egui::{
    Align2, Color32, Context, FontFamily, FontId, Pos2, Rect, RichText, Stroke, TextureHandle,
    Vec2,
};

use super::anim::{lerp, lerp_rect, shift, Spring};
use super::repos::card_icon;
use super::{theme, CraftApp};
use crate::config::{AssetKind, RepoConfig};
use crate::lang::{t, tf};
use crate::model::{
    human_date, human_size, InstalledVersion, LogLevel, ReleaseInfo, TaskProgress, TaskState,
};
use crate::storage;

/// Высота плитки в свёрнутом виде.
const TILE_H: f32 = 124.0;
/// Высота раскрытой карточки.
const HERO_H: f32 = 256.0;
/// Зазор между карточками.
const GAP: f32 = 14.0;
/// Максимальная ширина плитки.
const MAX_TILE_W: f32 = 190.0;
/// Минимальная ширина плитки: ниже колонки схлопываются.
const MIN_TILE_W: f32 = 116.0;
/// Высота заголовка группы над сеткой.
const HEADER_H: f32 = 30.0;
/// Отступ между группами.
const GROUP_GAP: f32 = 20.0;

/// Состояние раскрытой карточки.
pub struct HeroState {
    /// Открытая карточка (slug).
    pub open: Option<String>,
    /// Карточка, которая откроется сразу после закрытия текущей.
    pending: Option<String>,
    /// Прогресс раскрытия: 0 — свёрнуто, 1 — раскрыто.
    pub progress: Spring,
    /// Пружины наведения — по одной на карточку.
    hover: HashMap<String, Spring>,
    /// Момент первого показа сетки: карточки появляются по очереди.
    appear_at: Option<Instant>,
}

impl Default for HeroState {
    fn default() -> Self {
        Self::new()
    }
}

impl HeroState {
    pub fn new() -> Self {
        Self {
            open: None,
            pending: None,
            progress: Spring::new(0.0),
            hover: HashMap::new(),
            appear_at: None,
        }
    }

    /// Клик по карточке: раскрыть, свернуть или переключиться на другую.
    pub fn toggle(&mut self, slug: &str) {
        match self.open.as_deref() {
            Some(cur) if cur == slug => {
                self.pending = None;
                self.progress.set_target(0.0);
            }
            Some(_) => {
                // Сначала аккуратно сворачиваем текущую, затем раскрываем новую.
                self.pending = Some(slug.to_string());
                self.progress.set_target(0.0);
            }
            None => {
                self.open = Some(slug.to_string());
                self.progress.set_target(1.0);
            }
        }
    }

    /// Закрыть раскрытую карточку.
    pub fn close(&mut self) {
        self.pending = None;
        if self.open.is_some() {
            self.progress.set_target(0.0);
        }
    }

    /// Раскрыта ли эта карточка прямо сейчас.
    #[allow(dead_code)]
    pub fn is_open(&self, slug: &str) -> bool {
        self.open.as_deref() == Some(slug)
    }

    /// Шаг анимации. Возвращает `true`, если анимация ещё идёт.
    pub fn tick(&mut self, dt: f32) -> bool {
        let moving = self.progress.step(dt);
        if !moving && self.progress.target <= 0.5 {
            match self.pending.take() {
                Some(next) => {
                    self.open = Some(next);
                    self.progress.set_target(1.0);
                    return true;
                }
                None => self.open = None,
            }
        }
        moving
    }

    /// Плавность наведения для карточки.
    fn hover_of(&mut self, slug: &str, hovered: bool, dt: f32) -> f32 {
        let spring = self
            .hover
            .entry(slug.to_string())
            .or_insert_with(|| Spring::snappy(0.0));
        spring.set_target(if hovered { 1.0 } else { 0.0 });
        spring.step(dt);
        spring.value
    }

    /// Двигается ли плавность наведения хотя бы у одной карточки.
    fn hover_moving(&self) -> bool {
        self.hover.values().any(|s| !s.at_rest())
    }
}

/// Действия, собранные при отрисовке карточек.
enum CardAction {
    Download(String),
    Releases(String),
    OpenFolder(String),
    TogglePin(String),
    ToggleEnabled(String),
    SetFilter(String, AssetKind),
    ToggleOpen(String),
    Close,
}

/// Данные карточки, собранные заранее: так рисование не блокирует состояние.
struct CardData {
    slug: String,
    name: String,
    accent: Color32,
    tex: Option<TextureHandle>,
    enabled: bool,
    description: String,
    stable: Option<ReleaseInfo>,
    edge: Option<ReleaseInfo>,
    installed: Vec<InstalledVersion>,
    pinned: Vec<String>,
    last_check: Option<String>,
    last_error: Option<String>,
    tasks: Vec<TaskProgress>,
    filter: AssetKind,
    /// Варианты файлов релиза: вид и подпись с размером.
    asset_options: Vec<(AssetKind, String)>,
}

/// Раскладка одной группы репозиториев.
struct GroupLayout {
    /// Прямоугольники плиток в свёрнутом виде (относительно верха группы).
    collapsed: Vec<Rect>,
    /// Прямоугольники плиток в раскрытом виде.
    expanded: Vec<Rect>,
    /// Высота группы в свёрнутом виде.
    height_collapsed: f32,
    /// Высота группы в раскрытом виде.
    height_expanded: f32,
    /// Индекс раскрытой карточки в группе.
    hero: Option<usize>,
}

impl GroupLayout {
    fn build(
        repos: &[RepoConfig],
        width: f32,
        tile_w: f32,
        cols: usize,
        hero_slug: Option<&str>,
    ) -> Self {
        let n = repos.len();
        let mut collapsed = Vec::with_capacity(n);
        for i in 0..n {
            let row = i / cols;
            let col = i % cols;
            collapsed.push(Rect::from_min_size(
                Pos2::new(
                    col as f32 * (tile_w + GAP),
                    HEADER_H + row as f32 * (TILE_H + GAP),
                ),
                Vec2::new(tile_w, TILE_H),
            ));
        }

        let rows = n.div_ceil(cols);
        let height_collapsed = HEADER_H
            + if rows == 0 {
                0.0
            } else {
                rows as f32 * TILE_H + (rows - 1) as f32 * GAP
            };

        let mut expanded = collapsed.clone();
        let mut height_expanded = height_collapsed;

        let hero = hero_slug.and_then(|slug| repos.iter().position(|r| r.slug == slug));
        if let Some(h) = hero {
            let row = h / cols;
            let hero_y = HEADER_H + row as f32 * (TILE_H + GAP);
            // Герой занимает всю ширину на месте строки, по которой кликнули.
            expanded[h] = Rect::from_min_size(
                Pos2::new(0.0, hero_y),
                Vec2::new(width, HERO_H),
            );

            // Соседи по строке и всё, что ниже, переезжают под героя.
            let flow_y = hero_y + HERO_H + GAP;
            let mut flow: Vec<usize> = Vec::new();
            for (i, _) in repos.iter().enumerate() {
                if i == h {
                    continue;
                }
                if i / cols >= row {
                    flow.push(i);
                }
            }
            for (k, i) in flow.iter().enumerate() {
                let r = k / cols;
                let c = k % cols;
                expanded[*i] = Rect::from_min_size(
                    Pos2::new(
                        c as f32 * (tile_w + GAP),
                        flow_y + r as f32 * (TILE_H + GAP),
                    ),
                    Vec2::new(tile_w, TILE_H),
                );
            }

            let mut bottom = 0.0_f32;
            for r in &expanded {
                bottom = bottom.max(r.max.y);
            }
            height_expanded = bottom;
        }

        Self {
            collapsed,
            expanded,
            height_collapsed,
            height_expanded,
            hero,
        }
    }
}

/// Число колонок сетки для заданной ширины.
fn grid_cols(width: f32) -> usize {
    if !width.is_finite() || width <= 0.0 {
        return 1;
    }
    let mut cols = ((width + GAP) / (MAX_TILE_W + GAP)).ceil().max(1.0) as usize;
    while cols > 1 {
        if tile_width(width, cols) >= MIN_TILE_W {
            break;
        }
        cols -= 1;
    }
    cols.max(1)
}

/// Ширина плитки при заданном числе колонок.
fn tile_width(width: f32, cols: usize) -> f32 {
    let cols = cols.max(1) as f32;
    ((width - GAP * (cols - 1.0)) / cols).max(40.0)
}

/// Одно размещение карточки в содержимом.
struct Placement {
    /// Номер группы.
    group: usize,
    /// Номер репозитория внутри группы.
    index: usize,
    /// Прямоугольник на экране (текущий, анимированный).
    rect: Rect,
    /// Прямоугольник свёрнутой плитки — от него и считается морф.
    tile_rect: Rect,
    /// Прогресс появления.
    appear: f32,
    /// Является ли карточка раскрытой.
    is_hero: bool,
}

/// Отрисовать все группы репозиториев в режиме сетки.
pub fn show_groups(
    ui: &mut egui::Ui,
    app: &mut CraftApp,
    groups: &[(String, Vec<RepoConfig>)],
    ctx: &Context,
) {
    let dt = ctx.input(|i| i.stable_dt).min(0.05);
    if app.hero.tick(dt) {
        ctx.request_repaint();
    }

    let width = ui.available_width();
    // Бесконечная ширина ломает раскладку — ограничиваем её.
    let width = if width.is_finite() { width.max(120.0) } else { 120.0 };
    let cols = grid_cols(width);
    let tile_w = tile_width(width, cols);
    let hero_slug = app.hero.open.clone();
    let prog = app.hero.progress.value.clamp(0.0, 1.0);

    // Раскладки групп: свёрнутая и раскрытая.
    let layouts: Vec<GroupLayout> = groups
        .iter()
        .map(|(_, repos)| GroupLayout::build(repos, width, tile_w, cols, hero_slug.as_deref()))
        .collect();

    // Позиции групп: раскрытая группа растягивает всё, что ниже неё.
    let mut tops: Vec<f32> = Vec::with_capacity(layouts.len());
    let mut y = 0.0_f32;
    for l in &layouts {
        tops.push(y);
        y += lerp(l.height_collapsed, l.height_expanded, prog) + GROUP_GAP;
    }

    // Появление карточек: каскад с задержкой.
    let appear_at = *app.hero.appear_at.get_or_insert_with(Instant::now);
    let elapsed = appear_at.elapsed().as_secs_f32();
    // Пока идёт появление, нужно перерисовывать окно — иначе анимация замерзнет.
    let appear_animating = elapsed < 0.45 + groups.iter().map(|(_, r)| r.len()).sum::<usize>() as f32 * 0.018;
    if appear_animating || app.hero.hover_moving() {
        ctx.request_repaint();
    }

    let origin = ui.max_rect().min;
    let mut placements: Vec<Placement> = Vec::new();
    let mut order = 0usize;
    let mut bottom = 0.0_f32;

    for (gi, (_, repos)) in groups.iter().enumerate() {
        let layout = &layouts[gi];
        bottom = bottom.max(tops[gi] + lerp(layout.height_collapsed, layout.height_expanded, prog));
        for (i, _) in repos.iter().enumerate() {
            let base = lerp_rect(layout.collapsed[i], layout.expanded[i], prog);

            // Каскад появления: карточки выезжают снизу по очереди.
            // Раскрытая карточка проявляется сама — её не гасим.
            let delay = order as f32 * 0.018;
            let appear = if layout.hero == Some(i) {
                1.0
            } else {
                (((elapsed - delay) / 0.42).clamp(0.0, 1.0)).powi(3)
            };
            order += 1;

            let tile_rect = layout.collapsed[i].translate(origin.to_vec2());
            let rect = shift(base, (1.0 - appear) * 18.0).translate(origin.to_vec2());
            placements.push(Placement {
                group: gi,
                index: i,
                rect,
                tile_rect,
                appear,
                is_hero: layout.hero == Some(i),
            });
        }
    }

    // Заголовки групп.
    for (gi, (author, repos)) in groups.iter().enumerate() {
        draw_header(
            ui.painter(),
            origin + Vec2::new(0.0, tops[gi]),
            author,
            repos.len(),
        );
    }

    // Резервируем область содержимого: по ней прокрутка узнаёт высоту.
    let content_rect = Rect::from_min_size(origin, Vec2::new(width, bottom + 20.0));
    ui.allocate_rect(content_rect, egui::Sense::hover());

    // Карточки. Раскрытая рисуется последней — она лежит поверх соседей.
    let mut actions: Vec<CardAction> = Vec::new();
    let mut order_of_draw: Vec<usize> = (0..placements.len()).collect();
    if let Some(pos) = placements.iter().position(|p| p.is_hero) {
        order_of_draw.remove(pos);
        order_of_draw.push(pos);
    }
    for idx in order_of_draw {
        let p = &placements[idx];
        let rc = &groups[p.group].1[p.index];

        // Реакция на наведение считается до отрисовки — от неё зависит вид.
        let id = ui.id().with(("hero_card", rc.slug.as_str()));
        let response = ui.interact(p.rect, id, egui::Sense::click());
        let hover = app.hero.hover_of(&rc.slug, response.hovered(), dt);

        let data = collect_data(app, rc, ctx);

        let resp = response.clone();
        // Содержимое перетекает в героя только у самой раскрытой карточки:
        // соседние плитки остаются плитками, меняется только их положение.
        let content_prog = if p.is_hero { prog } else { 0.0 };
        ui.scope_builder(egui::UiBuilder::new().max_rect(p.rect), |ui| {
            draw_card(
                ui,
                &data,
                p.rect,
                p.tile_rect,
                resp,
                content_prog,
                p.is_hero,
                hover,
                p.appear,
                &mut actions,
            );
        });
    }

    for action in actions {
        apply_action(app, action);
    }
}

/// Заголовок группы: «Автор storyteller» и число репозиториев.
fn draw_header(painter: &egui::Painter, pos: Pos2, author: &str, count: usize) {
    let font = FontId::new(13.0, FontFamily::Proportional);
    let text = format!("{}  {}", t("repo.group"), author);
    painter.text(
        Pos2::new(pos.x, pos.y + 14.0),
        Align2::LEFT_CENTER,
        text.clone(),
        font.clone(),
        theme::ACCENT,
    );
    let w = text_width(painter, &text, &font);
    painter.text(
        Pos2::new(pos.x + w + 8.0, pos.y + 14.0),
        Align2::LEFT_CENTER,
        count.to_string(),
        FontId::new(11.0, FontFamily::Proportional),
        theme::DIM,
    );
}

/// Собрать данные карточки.
fn collect_data(app: &mut CraftApp, rc: &RepoConfig, ctx: &Context) -> CardData {
    let tex = card_icon(app, rc, ctx);
    let rt = app.state.shared.repo_runtime(&rc.slug);
    let tasks = app.tasks_for(&rc.slug);
    // Варианты файлов: реальные файлы релиза, а не общие правила.
    let asset_options = rt
        .as_ref()
        .and_then(|r| {
            r.releases
                .iter()
                .find(|rel| !rel.is_edge())
                .or_else(|| r.releases.first())
        })
        .map(|rel| {
            crate::manager::asset_options(&rc.slug, rel)
                .into_iter()
                .map(|(k, a)| {
                    (
                        k,
                        format!("{} · {}", k.localized_label(), human_size(a.size)),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    CardData {
        slug: rc.slug.clone(),
        name: rc.short_name(),
        accent: theme::accent_for(&rc.slug),
        tex,
        enabled: rc.enabled,
        description: rc.description.trim().to_string(),
        stable: rt.as_ref().and_then(|r| r.latest_stable.clone()),
        edge: rt.as_ref().and_then(|r| r.latest_edge.clone()),
        installed: rt.as_ref().map(|r| r.installed.clone()).unwrap_or_default(),
        pinned: rc.pinned.clone(),
        last_check: rt.as_ref().and_then(|r| r.last_check.clone()),
        last_error: rt.as_ref().and_then(|r| r.last_error.clone()),
        tasks,
        filter: rc.filter,
        asset_options,
    }
}

/// Отрисовать карточку: плитку, промежуточное состояние или героя.
#[allow(clippy::too_many_arguments)]
fn draw_card(
    ui: &mut egui::Ui,
    data: &CardData,
    rect: Rect,
    tile_rect: Rect,
    response: egui::Response,
    prog: f32,
    is_hero: bool,
    hover: f32,
    appear: f32,
    actions: &mut Vec<CardAction>,
) {
    // Содержимое не должно вылезать за пределы растущей карточки.
    ui.set_clip_rect(ui.clip_rect().intersect(rect));

    let accent = data.accent;
    let radius = lerp(12.0, 18.0, prog);
    let mut painter = ui.painter().clone();
    painter.multiply_opacity(appear);

    // --- Фон ---
    let fill = mix(theme::CARD, accent, 0.05 * prog + 0.07 * hover);
    let stroke_color = if is_hero {
        with_alpha(accent, 0.50 + 0.30 * hover)
    } else {
        mix(theme::BORDER, accent, 0.45 * hover)
    };
    let stroke_width = lerp(1.0, 1.6, prog);

    // Мягкое свечение вокруг героя.
    if prog > 0.01 {
        let mut glow = egui::epaint::RectShape::new(
            rect.expand(3.0),
            corner(radius + 3.0),
            with_alpha(accent, 0.16 * prog),
            egui::Stroke::NONE,
            egui::StrokeKind::Outside,
        );
        glow.blur_width = 22.0;
        painter.add(glow);
    }

    let shadow = egui::epaint::Shadow {
        offset: [0, if is_hero { 10 } else { 2 }],
        blur: if is_hero { 26 } else { 10 },
        spread: 0,
        color: Color32::from_black_alpha(if is_hero { 120 } else { 70 }),
    };
    let frame = egui::Frame::new()
        .fill(fill)
        .corner_radius(corner(radius))
        .stroke(Stroke::new(stroke_width, stroke_color))
        .inner_margin(egui::Margin::ZERO)
        .shadow(shadow);
    painter.add(frame.paint(rect));

    // --- Общий элемент: иконка перетекает из плитки в героя ---
    // Точки отсчёта берём от свёрнутой плитки: иначе при росте карточки
    // «плиточная» позиция уезжает вправо и тянет иконку за собой.
    // У обычных плиток «плиточный» вид — это их текущее место.
    let tile_origin = if is_hero { tile_rect } else { rect };
    let icon_tile = Rect::from_min_size(
        Pos2::new(tile_origin.center().x - 28.0, tile_origin.top() + 12.0),
        Vec2::splat(56.0),
    );
    let icon_hero = Rect::from_min_size(
        Pos2::new(rect.left() + 20.0, rect.top() + 38.0),
        Vec2::splat(84.0),
    );
    let icon_rect = lerp_rect(icon_tile, icon_hero, prog);
    draw_icon(&painter, data, icon_rect, prog);

    // --- Общий элемент: название перетекает и увеличивается ---
    // Позиция названия считается от иконки: в плитке оно под иконкой,
    // в герое — справа от неё. Так текст не «отрывается» от картинки.
    let name_size = lerp(13.0, 22.0, prog);
    let name_font = FontId::new(name_size, FontFamily::Proportional);
    let name_w = text_width(&painter, &data.name, &name_font);
    let tile_left = icon_rect.center().x - name_w * 0.5;
    let hero_left = icon_rect.max.x + 20.0;
    let name_left = lerp(tile_left, hero_left, prog);
    let tile_mid = icon_rect.max.y + 12.0;
    let hero_mid = icon_rect.top() + 20.0;
    // Пока текст переезжает из-под иконки вбок, он слегка провисает вниз —
    // иначе прямая линия пересекала бы иконку.
    let arc = (std::f32::consts::PI * prog).sin() * 24.0;
    let name_mid = lerp(tile_mid, hero_mid, prog) + arc;
    painter.text(
        Pos2::new(name_left, name_mid),
        Align2::LEFT_CENTER,
        data.name.clone(),
        name_font,
        theme::TEXT,
    );
    let name_rect = Rect::from_center_size(
        Pos2::new(name_left + name_w * 0.5, name_mid),
        Vec2::new(name_w + 8.0, name_size * 1.5),
    );

    // --- Содержимое плитки: гаснет при раскрытии ---
    if prog < 0.995 {
        let alpha = ((1.0 - prog) / 0.55).clamp(0.0, 1.0);
        // Идёт загрузка — в плитке только полоса и процент.
        if let Some(task) = data.tasks.iter().find(|t| !t.state.is_finished()) {
            let bar_rect = Rect::from_min_size(
                Pos2::new(tile_origin.left() + 12.0, tile_origin.top() + 96.0),
                Vec2::new((tile_origin.width() - 24.0).max(30.0), 17.0),
            );
            draw_bar(
                &painter,
                bar_rect,
                task.fraction(),
                with_alpha(super::progress::fill_color(task.state), alpha),
                alpha,
            );
        } else {
            let (text, color) = if !data.enabled {
                (t("common.disabled").to_string(), theme::DIM)
            } else if let Some(rel) = &data.stable {
                (rel.tag.clone(), theme::OK)
            } else {
                (t("repo.no_releases").to_string(), theme::DIM)
            };
            painter.text(
                Pos2::new(tile_origin.center().x, tile_origin.top() + 100.0),
                Align2::CENTER_CENTER,
                text,
                FontId::new(11.0, FontFamily::Proportional),
                with_alpha(color, alpha),
            );
        }
    }

    // --- Содержимое героя: проявляется ---
    if prog > 0.005 {
        draw_hero_content(&painter, data, rect, prog, name_left, name_mid);
    }

    // --- Интерактив ---
    let checkbox_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 27.0, rect.top() + 10.0),
        Vec2::splat(15.0),
    );

    // Клик по телу карточки раскрывает её; клик по углу с чекбоксом — нет.
    // У раскрытой карточки клик по названию сворачивает её обратно.
    if response.clicked() {
        let pointer = response.interact_pointer_pos();
        let on_checkbox = pointer
            .map(|p| checkbox_rect.expand(6.0).contains(p))
            .unwrap_or(false);
        let on_title = pointer.map(|p| name_rect.contains(p)).unwrap_or(false);
        if !on_checkbox && (!is_hero || on_title) {
            actions.push(CardAction::ToggleOpen(data.slug.clone()));
        }
    }

    // Чекбокс «включить в загрузку».
    {
        let mut enabled = data.enabled;
        let inner = ui
            .scope_builder(egui::UiBuilder::new().max_rect(checkbox_rect), |ui| {
                ui.add_sized([15.0, 15.0], egui::Checkbox::new(&mut enabled, ""))
            })
            .inner;
        if inner.changed() {
            actions.push(CardAction::ToggleEnabled(data.slug.clone()));
        }
    }

    // Кнопки героя — только у раскрытой карточки.
    if is_hero && prog > 0.45 {
        draw_hero_buttons(ui, data, rect, prog, actions);
    }
}

/// Нарисовать иконку карточки.
fn draw_icon(painter: &egui::Painter, data: &CardData, rect: Rect, prog: f32) {
    match &data.tex {
        Some(tex) => {
            painter.add(egui::Shape::image(
                tex.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            ));
        }
        None => {
            let radius = lerp(10.0, 16.0, prog);
            painter.rect_filled(
                rect,
                corner(radius),
                with_alpha(data.accent.gamma_multiply(0.24), 1.0),
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                initials(&data.name),
                FontId::new(lerp(18.0, 26.0, prog), FontFamily::Proportional),
                data.accent,
            );
        }
    }
}

/// Содержимое раскрытой карточки: метаданные, описание, каналы.
///
/// Строки идут друг за другом с запасом: русские надписи длиннее английских,
/// и при абсолютных координатах они наезжали друг на друга.
fn draw_hero_content(
    painter: &egui::Painter,
    data: &CardData,
    rect: Rect,
    prog: f32,
    name_left: f32,
    name_mid: f32,
) {
    // Дополнительные сведения проявляются во второй половине перехода —
    // иначе они наезжают на название, которое ещё перетекает.
    let alpha = ((prog - 0.40) / 0.60).clamp(0.0, 1.0);
    let right_edge = rect.right() - 20.0;

    // Строка метаданных: репозиторий, версия, дата, время проверки.
    // Держится рядом с названием и едет вместе с ним.
    let mut parts: Vec<String> = vec![data.slug.clone()];
    if let Some(rel) = &data.stable {
        parts.push(rel.tag.clone());
    }
    if let Some(date) = data.stable.as_ref().and_then(|r| r.published_at.clone()) {
        parts.push(human_date(&date));
    }
    if let Some(checked) = &data.last_check {
        parts.push(format!("{}: {checked}", t("repo.checked")));
    }
    let meta_font = FontId::new(12.0, FontFamily::Proportional);
    let meta = elide(
        painter,
        &parts.join("  ·  "),
        &meta_font,
        (right_edge - name_left).max(60.0),
    );
    painter.text(
        Pos2::new(name_left, name_mid + 26.0),
        Align2::LEFT_CENTER,
        meta,
        meta_font,
        with_alpha(theme::DIM, alpha),
    );

    // Каналы: стабильный и нестабильный. Плашки ниже строки метаданных,
    // иначе они наезжают на неё.
    let mut x = name_left;
    if let Some(rel) = &data.stable {
        let w = draw_chip(
            painter,
            Vec2::new(x, name_mid + 46.0),
            t("popup.stable"),
            &rel.tag,
            theme::OK,
            alpha,
            right_edge,
        );
        if w <= 0.0 {
            return;
        }
        x += w + 8.0;
    }
    if let Some(rel) = &data.edge {
        draw_chip(
            painter,
            Vec2::new(x, name_mid + 46.0),
            t("popup.edge"),
            &rel.tag,
            theme::EDGE,
            alpha,
            right_edge,
        );
    }

    // Описание.
    let desc = if data.description.is_empty() {
        t("repo.desc_missing").to_string()
    } else {
        data.description.clone()
    };
    let desc_font = FontId::new(12.5, FontFamily::Proportional);
    let wrap = (rect.width() - 40.0).max(80.0);
    let galley = painter.layout(desc, desc_font, with_alpha(theme::TEXT, alpha), wrap);
    let desc_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 20.0, rect.top() + 134.0),
        Vec2::new(wrap, 36.0),
    );
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(desc_rect));
    clipped.galley(
        Pos2::new(desc_rect.left(), desc_rect.top()),
        galley,
        with_alpha(theme::TEXT, alpha),
    );

    // Нижняя строка: что уже скачано слева, активная загрузка справа.
    let row_y = rect.top() + 182.0;
    let small = FontId::new(11.0, FontFamily::Proportional);

    // Справа — активная загрузка: резервируем под неё место.
    let mut right_used = 0.0_f32;
    if let Some(task) = data.tasks.iter().find(|t| !t.state.is_finished()) {
        let label = format!(
            "{}  ·  {}",
            task.asset,
            match task.state {
                TaskState::Paused => t("status.paused").to_string(),
                TaskState::Queued => "…".to_string(),
                _ => crate::model::human_speed(task.speed),
            }
        );
        let w = text_width(painter, &label, &small);
        if right_edge - w - name_left > 120.0 {
            painter.text(
                Pos2::new(right_edge - w, row_y),
                Align2::LEFT_CENTER,
                label,
                small.clone(),
                with_alpha(theme::ACCENT, alpha),
            );
            right_used = w + 16.0;
        }
    }

    // Слева — установленные версии или ошибка.
    let max_left = (right_edge - right_used - (rect.left() + 20.0)).max(80.0);
    let text = if let Some(err) = &data.last_error {
        Some((format!("! {err}"), theme::DANGER))
    } else if !data.installed.is_empty() {
        let joined = data
            .installed
            .iter()
            .map(|v| format!("{} · {}", v.tag, human_size(v.size)))
            .collect::<Vec<_>>()
            .join("   ");
        Some((joined, theme::DIM))
    } else {
        None
    };
    if let Some((text, color)) = text {
        let trimmed = elide(painter, &text, &small, max_left);
        painter.text(
            Pos2::new(rect.left() + 20.0, row_y),
            Align2::LEFT_CENTER,
            trimmed,
            small,
            with_alpha(color, alpha),
        );
    }
}

/// Плашка канала: «stable v1.2.3». Возвращает занятую ширину.
///
/// Если плашка не помещается до `max_right`, она не рисуется и возвращает 0.
fn draw_chip(
    painter: &egui::Painter,
    pos: Vec2,
    title: &str,
    tag: &str,
    color: Color32,
    alpha: f32,
    max_right: f32,
) -> f32 {
    let f1 = FontId::new(10.0, FontFamily::Proportional);
    let f2 = FontId::new(11.0, FontFamily::Proportional);
    let w1 = text_width(painter, title, &f1);
    let w2 = text_width(painter, tag, &f2);
    let w = w1 + 6.0 + w2 + 18.0;
    if pos.x + w > max_right {
        return 0.0;
    }
    let rect = Rect::from_min_size(Pos2::new(pos.x, pos.y), Vec2::new(w, 18.0));
    painter.rect_filled(rect, corner(9.0), with_alpha(color, 0.15 * alpha));
    painter.rect_stroke(
        rect,
        corner(9.0),
        Stroke::new(1.0_f32, with_alpha(color, 0.45 * alpha)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(rect.left() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        f1,
        with_alpha(color, alpha),
    );
    painter.text(
        Pos2::new(rect.left() + 9.0 + w1 + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        tag,
        f2,
        with_alpha(theme::TEXT, alpha),
    );
    w
}

/// Полоса прогресса с процентом по центру — для карточки-героя.
fn draw_bar(painter: &egui::Painter, rect: Rect, frac: f32, color: Color32, alpha: f32) {
    let frac = frac.clamp(0.0, 1.0);
    let radius = egui::CornerRadius::same((rect.height() / 2.0).round().clamp(2.0, 12.0) as u8);
    painter.rect_filled(rect, radius, theme::BG);
    let fill_w = rect.width() * frac;
    if fill_w > 1.0 {
        let fill_rect = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + fill_w, rect.bottom()),
        );
        painter.rect_filled(fill_rect, radius, color);
    }
    let text = format!("{}%", (frac * 100.0).round() as u32);
    let font = FontId::new(10.5, FontFamily::Proportional);
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
        painter.text(
            rect.center() + egui::vec2(dx, dy),
            Align2::CENTER_CENTER,
            text.clone(),
            font.clone(),
            Color32::from_black_alpha(190),
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        font,
        with_alpha(Color32::WHITE, alpha),
    );
}

/// Обрезать строку по ширине, добавив многоточие.
fn elide(painter: &egui::Painter, text: &str, font: &FontId, max_w: f32) -> String {
    if max_w <= 0.0 {
        return String::new();
    }
    if text_width(painter, text, font) <= max_w {
        return text.to_string();
    }
    let mut out = String::new();
    let mut w = 0.0_f32;
    for c in text.chars() {
        let cw = painter.fonts(|f| f.glyph_width(font, c));
        if w + cw > max_w - 10.0 {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// Кнопки раскрытой карточки.
fn draw_hero_buttons(
    ui: &mut egui::Ui,
    data: &CardData,
    rect: Rect,
    prog: f32,
    actions: &mut Vec<CardAction>,
) {
    let alpha = ((prog - 0.40) / 0.60).clamp(0.0, 1.0);
    let top = rect.top() + 208.0;
    let mut x = rect.left() + 20.0;

    // Главная кнопка «Скачать».
    let dl_rect = Rect::from_min_size(Pos2::new(x, top), Vec2::new(108.0, 30.0));
    x += 108.0 + 8.0;
    let accent = data.accent;
    let inner = ui
        .scope_builder(egui::UiBuilder::new().max_rect(dl_rect), |ui| {
            ui.set_style(faded_style(ui.style(), alpha));
            ui.add_sized(
                [108.0, 30.0],
                egui::Button::new(
                    RichText::new(t("common.download"))
                        .size(13.0)
                        .color(with_alpha(Color32::WHITE, alpha)),
                )
                .fill(with_alpha(accent, alpha))
                .stroke(Stroke::new(
                    1.0_f32,
                    with_alpha(accent.gamma_multiply(0.75), alpha),
                ))
                .corner_radius(corner(9.0)),
            )
        })
        .inner;
    if inner.clicked() {
        actions.push(CardAction::Download(data.slug.clone()));
    }

    // «Релизы».
    let rel_rect = Rect::from_min_size(Pos2::new(x, top), Vec2::new(92.0, 30.0));
    x += 92.0 + 8.0;
    let inner = ui
        .scope_builder(egui::UiBuilder::new().max_rect(rel_rect), |ui| {
            ui.set_style(faded_style(ui.style(), alpha));
            ui.add_sized(
                [92.0, 30.0],
                egui::Button::new(
                    RichText::new(t("common.releases"))
                        .size(13.0)
                        .color(with_alpha(theme::TEXT, alpha)),
                )
                .corner_radius(corner(9.0)),
            )
        })
        .inner;
    if inner.clicked() {
        actions.push(CardAction::Releases(data.slug.clone()));
    }

    // «Папка».
    let open_rect = Rect::from_min_size(Pos2::new(x, top), Vec2::new(78.0, 30.0));
    x += 78.0 + 8.0;
    let inner = ui
        .scope_builder(egui::UiBuilder::new().max_rect(open_rect), |ui| {
            ui.set_style(faded_style(ui.style(), alpha));
            ui.add_sized(
                [78.0, 30.0],
                egui::Button::new(
                    RichText::new(t("hero.open_folder"))
                        .size(13.0)
                        .color(with_alpha(theme::TEXT, alpha)),
                )
                .corner_radius(corner(9.0)),
            )
        })
        .inner;
    if inner.clicked() {
        actions.push(CardAction::OpenFolder(data.slug.clone()));
    }

    // «Закрепить».
    let pinned = data
        .stable
        .as_ref()
        .map(|rel| data.pinned.contains(&rel.tag))
        .unwrap_or(false);
    let pin_text = if pinned {
        t("common.unpin")
    } else {
        t("common.pin")
    };
    let pin_rect = Rect::from_min_size(Pos2::new(x, top), Vec2::new(104.0, 30.0));
    let inner = ui
        .scope_builder(egui::UiBuilder::new().max_rect(pin_rect), |ui| {
            ui.set_style(faded_style(ui.style(), alpha));
            ui.add_sized(
                [104.0, 30.0],
                egui::Button::new(
                    RichText::new(pin_text)
                        .size(13.0)
                        .color(with_alpha(
                            if pinned { theme::WARN } else { theme::TEXT },
                            alpha,
                        )),
                )
                .corner_radius(corner(9.0)),
            )
        })
        .inner;
    if inner.clicked() {
        actions.push(CardAction::TogglePin(data.slug.clone()));
    }

    // Выбор файлов — справа от кнопок: реальные файлы релиза,
    // по умолчанию — вариант под текущую ОС.
    let combo_w = 230.0_f32;
    let combo_left = rect.right() - 20.0 - combo_w;
    let combo_rect = Rect::from_min_size(
        Pos2::new(combo_left, top),
        Vec2::new(combo_w, 28.0),
    );
    let label = t("repo.filter");
    let label_font = FontId::new(11.5, FontFamily::Proportional);
    let label_w = text_width(ui.painter(), label, &label_font);
    ui.painter().text(
        Pos2::new(combo_left - label_w - 10.0, top + 14.0),
        Align2::LEFT_CENTER,
        label,
        label_font,
        with_alpha(theme::DIM, alpha),
    );
    let mut filter = data.filter;
    let mut changed = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(combo_rect), |ui| {
        ui.set_style(faded_style(ui.style(), alpha));
        let combo = egui::ComboBox::from_id_salt(("hero_filter", data.slug.as_str()))
            .selected_text(filter.localized_label())
            .width(combo_w)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut filter,
                    AssetKind::Auto,
                    format!(
                        "{} · {}",
                        AssetKind::Auto.localized_label(),
                        t("filter.auto_hint")
                    ),
                );
                for (k, text) in &data.asset_options {
                    ui.selectable_value(&mut filter, *k, text.clone());
                }
                ui.separator();
                ui.selectable_value(
                    &mut filter,
                    AssetKind::Source,
                    AssetKind::Source.localized_label(),
                );
                ui.selectable_value(
                    &mut filter,
                    AssetKind::All,
                    AssetKind::All.localized_label(),
                );
            });
        changed = combo.response.changed();
    });
    if changed {
        actions.push(CardAction::SetFilter(data.slug.clone(), filter));
    }

    // Кнопка закрытия — в правом верхнем углу.
    let close_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 60.0, rect.top() + 6.0),
        Vec2::splat(24.0),
    );
    let inner = ui
        .scope_builder(egui::UiBuilder::new().max_rect(close_rect), |ui| {
            ui.set_style(faded_style(ui.style(), alpha));
            ui.add_sized(
                [24.0, 24.0],
                egui::Button::new(RichText::new("×").size(15.0).color(with_alpha(theme::DIM, alpha)))
                    .fill(Color32::TRANSPARENT)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(corner(8.0)),
            )
        })
        .inner;
    if inner.clicked() {
        actions.push(CardAction::Close);
    }
}

/// Применить действие, собранное на карточке.
fn apply_action(app: &mut CraftApp, action: CardAction) {
    match action {
        CardAction::Download(slug) => {
            let st = app.state.clone();
            let handle = st.handle.clone();
            let slug_c = slug.clone();
            handle.spawn(async move {
                st.download_fresh(&slug_c, crate::manager::DownloadRequest::Targets)
                    .await
            });
            app.notify(tf("repo.download_started", &[("{s}", &slug)]), LogLevel::Info);
        }
        CardAction::Releases(slug) => {
            app.open_releases(&slug);
        }
        CardAction::OpenFolder(slug) => {
            let cfg = app.state.cfg();
            let dir = storage::repo_dir(&cfg.root_dir, &slug);
            let _ = std::fs::create_dir_all(&dir);
            let _ = open::that(&dir);
        }
        CardAction::TogglePin(slug) => {
            let mut cfg = app.state.cfg();
            let tag = app.state.shared.repo_runtime(&slug).and_then(|r| {
                r.latest_stable
                    .or(r.latest_edge)
                    .map(|rel| rel.tag.clone())
            });
            if let (Some(tag), Some(rc)) = (tag, cfg.repo_mut(&slug)) {
                if let Some(pos) = rc.pinned.iter().position(|p| p == &tag) {
                    rc.pinned.remove(pos);
                } else {
                    rc.pinned.push(tag);
                }
            }
            app.state.save_cfg(cfg);
            app.notify(t("notify.pin_changed"), LogLevel::Ok);
        }
        CardAction::ToggleEnabled(slug) => {
            let mut cfg = app.state.cfg();
            if let Some(rc) = cfg.repo_mut(&slug) {
                rc.enabled = !rc.enabled;
            }
            app.state.save_cfg(cfg);
        }
        CardAction::SetFilter(slug, filter) => {
            let mut cfg = app.state.cfg();
            if let Some(rc) = cfg.repo_mut(&slug) {
                rc.filter = filter;
            }
            app.state.save_cfg(cfg);
        }
        CardAction::ToggleOpen(slug) => {
            app.hero.toggle(&slug);
        }
        CardAction::Close => {
            app.hero.close();
        }
    }
}

/// Осветлить/затемнить стиль виджета для плавного появления.
fn faded_style(style: &egui::Style, alpha: f32) -> egui::Style {
    let mut style = style.clone();
    style.visuals = faded_visuals(&style.visuals, alpha);
    style
}

/// Затемнить цвета виджетов.
fn faded_visuals(v: &egui::Visuals, alpha: f32) -> egui::Visuals {
    let mut v = v.clone();
    v.override_text_color = v.override_text_color.map(|c| with_alpha(c, alpha));
    v.hyperlink_color = with_alpha(v.hyperlink_color, alpha);
    v.warn_fg_color = with_alpha(v.warn_fg_color, alpha);
    v.error_fg_color = with_alpha(v.error_fg_color, alpha);
    v.selection.bg_fill = with_alpha(v.selection.bg_fill, alpha);
    v.selection.stroke = Stroke::new(
        v.selection.stroke.width,
        with_alpha(v.selection.stroke.color, alpha),
    );
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = with_alpha(w.bg_fill, alpha);
        w.weak_bg_fill = with_alpha(w.weak_bg_fill, alpha);
        w.bg_stroke = Stroke::new(w.bg_stroke.width, with_alpha(w.bg_stroke.color, alpha));
        w.fg_stroke = Stroke::new(w.fg_stroke.width, with_alpha(w.fg_stroke.color, alpha));
    }
    v
}

/// Цвет с заданной прозрачностью.
fn with_alpha(c: Color32, alpha: f32) -> Color32 {
    let a = (c.a() as f32 * alpha.clamp(0.0, 1.0)).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Смешать два цвета.
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| ((x as f32) + ((y as f32) - (x as f32)) * t).round() as u8;
    Color32::from_rgba_unmultiplied(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()), f(a.a(), b.a()))
}

/// Скругление из дробного числа.
fn corner(radius: f32) -> egui::CornerRadius {
    egui::CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8)
}

/// Ширина строки в пикселях.
fn text_width(painter: &egui::Painter, text: &str, font: &FontId) -> f32 {
    painter
        .fonts(|f| text.chars().map(|c| f.glyph_width(font, c)).sum::<f32>())
}

/// Две первые буквы названия — заглушка вместо иконки.
fn initials(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}
