//! Пружинная физика для «живых» переходов интерфейса.
//!
//! Обычная линейная интерполяция даёт «ступеньки»: движение начинается и
//! заканчивается рывком. Пружина вместо этого разгоняется, слегка перелетает
//! цель и мягко тормозит — как в анимациях, которые показывают в портфолио.

use egui::{Rect, Vec2};

/// Пружина: значение стремится к цели, слегка её перелетая.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    /// Текущее значение.
    pub value: f32,
    /// Скорость, ед/с.
    pub velocity: f32,
    /// Куда стремимся.
    pub target: f32,
    /// Жёсткость, 1/с².
    stiffness: f32,
    /// Затухание, 1/с.
    damping: f32,
}

impl Spring {
    /// Пружина для крупных переходов: заметный, но аккуратный перелёт цели.
    pub fn new(value: f32) -> Self {
        let stiffness = 165.0_f32;
        // Критическое затухание = 2·√k. Берём 0.55 от него — будет перелёт ~13%.
        let damping = 2.0 * stiffness.sqrt() * 0.55;
        Self {
            value,
            velocity: 0.0,
            target: value,
            stiffness,
            damping,
        }
    }

    /// Быстрая пружина для наведения: почти без перелёта.
    pub fn snappy(value: f32) -> Self {
        let stiffness = 460.0_f32;
        let damping = 2.0 * stiffness.sqrt() * 0.9;
        Self {
            value,
            velocity: 0.0,
            target: value,
            stiffness,
            damping,
        }
    }

    /// Задать цель.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Шаг интегрирования. Возвращает `true`, если пружина ещё движется.
    ///
    /// Интегрирование идёт мелкими подшагами: при крупном `dt` (перетаскивание
    /// окна, отладка) «жёсткая» пружина не должна разлететься.
    pub fn step(&mut self, dt: f32) -> bool {
        let dt = dt.clamp(0.0, 0.1);
        if dt <= 0.0 {
            return !self.at_rest();
        }
        let steps = ((dt / 0.006).ceil() as usize).clamp(1, 32);
        let h = dt / steps as f32;
        for _ in 0..steps {
            let a = -self.stiffness * (self.value - self.target) - self.damping * self.velocity;
            self.velocity += a * h;
            self.value += self.velocity * h;
        }
        // Считаем пружину остановившейся, когда она почти в цели и почти не движется.
        if (self.value - self.target).abs() < 0.0008 && self.velocity.abs() < 0.02 {
            self.value = self.target;
            self.velocity = 0.0;
            return false;
        }
        true
    }

    /// Пружина остановилась.
    pub fn at_rest(&self) -> bool {
        (self.value - self.target).abs() < 0.0008 && self.velocity.abs() < 0.02
    }
}

/// Линейная интерполяция.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Интерполяция прямоугольников: углы двигаются независимо друг от друга.
pub fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(a.min.lerp(b.min, t), a.max.lerp(b.max, t))
}

/// Смещение прямоугольника по вертикали.
pub fn shift(rect: Rect, dy: f32) -> Rect {
    Rect::from_min_max(rect.min + Vec2::new(0.0, dy), rect.max + Vec2::new(0.0, dy))
}
