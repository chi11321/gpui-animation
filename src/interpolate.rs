use std::{
    fmt::Debug,
    mem::transmute,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::*;

use crate::{
    animation::AnimationPriority,
    transition::{Transition, TransitionRegistry},
};

macro_rules! optional_refine_interp {
    ($self:expr, $other:expr, $field:ident, $t:expr) => {
        if let Some(a) = $self.$field.as_ref()
            && let Some(b) = $other.$field.as_ref()
            && a.ne(b)
        {
            Some(a.interpolate(b, $t))
        } else {
            $other.$field.clone()
        }
    };
}

macro_rules! refine_interp {
    ($self:expr, $other:expr, $field:ident, $t:expr) => {
        if $self.$field.ne(&$other.$field) {
            $self.$field.interpolate(&$other.$field, $t)
        } else {
            $self.$field.clone()
        }
    };
}

macro_rules! fast_optional_refine_interp {
    ($self:expr, $other:expr, $field:ident, $t:expr, $out:expr) => {
        if let Some(a) = $self.$field.as_ref()
            && let Some(b) = $other.$field.as_ref()
            && a.ne(b)
        {
            $out.$field = Some(a.interpolate(b, $t));
        }
    };
}

macro_rules! fast_refine_interp {
    ($self:expr, $other:expr, $field:ident, $t:expr, $out:expr) => {
        if $self.$field.ne(&$other.$field) {
            $out.$field = $self.$field.interpolate(&$other.$field, $t);
        }
    };
}

pub trait Interpolatable: Clone {
    fn interpolate(&self, other: &Self, t: f32) -> Self;
}

pub trait FastInterpolatable: Clone {
    fn fast_interpolate(&self, other: &Self, t: f32, out: &mut Self);
}

impl Interpolatable for Hsla {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        let mut dt = other.h - self.h;

        if dt > 0.5 {
            dt -= 1.0;
        } else if dt < -0.5 {
            dt += 1.0;
        }

        let h = (self.h + dt * t).rem_euclid(1.0);

        Hsla {
            h,
            s: self.s + (other.s - self.s) * t,
            l: self.l + (other.l - self.l) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl Interpolatable for f32 {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        *self + (*other - *self) * t
    }
}

impl Interpolatable for Pixels {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        let from: f32 = unsafe { transmute(*self) };
        let to: f32 = unsafe { transmute(*other) };

        from.interpolate(&to, t).into()
    }
}

impl Interpolatable for Rems {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        // fixed cache
        Rems((self.0.interpolate(&other.0, t) * 120.).round() / 120.)
    }
}

impl Interpolatable for AbsoluteLength {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        match (self, other) {
            (AbsoluteLength::Pixels(f), AbsoluteLength::Pixels(t_val)) => {
                AbsoluteLength::Pixels(f.interpolate(t_val, t))
            }
            (AbsoluteLength::Rems(f), AbsoluteLength::Rems(t_val)) => {
                AbsoluteLength::Rems(f.interpolate(t_val, t))
            }
            (AbsoluteLength::Rems(f), AbsoluteLength::Pixels(t_val)) => AbsoluteLength::Pixels(
                f.to_pixels(TransitionRegistry::rem_size())
                    .interpolate(t_val, t),
            ),
            (AbsoluteLength::Pixels(f), AbsoluteLength::Rems(t_val)) => AbsoluteLength::Pixels(
                f.interpolate(&t_val.to_pixels(TransitionRegistry::rem_size()), t),
            ),
        }
    }
}

impl Interpolatable for FontWeight {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        self.0.interpolate(&other.0, t).into()
    }
}

#[derive(Clone)]
#[repr(C)]
pub struct ShadowBackground {
    pub tag: ShadowBackgroundTag,
    pad0: u32,
    pub solid: Hsla,
    pub gradient_angle_or_pattern_height: f32,
    pub colors: [LinearColorStop; 2],
    pad1: u32,
}

#[derive(Clone)]
#[repr(C)]
pub enum ShadowBackgroundTag {
    #[allow(dead_code)]
    Solid = 0,
    LinearGradient = 1,
    #[allow(dead_code)]
    PatternSlash = 2,
}

impl ShadowBackground {
    pub fn from(bg: &Background) -> &Self {
        unsafe { &*(bg as *const Background as *const Self) }
    }

    fn get_effective_colors(&self) -> [LinearColorStop; 2] {
        if self.colors[0].eq_none() && self.colors[1].eq_none() {
            [
                LinearColorStop {
                    color: self.solid,
                    percentage: 0.,
                },
                LinearColorStop {
                    color: self.solid,
                    percentage: 1.,
                },
            ]
        } else {
            self.colors.clone()
        }
    }
}

impl Interpolatable for LinearColorStop {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            color: refine_interp!(self, other, color, t),
            percentage: refine_interp!(self, other, percentage, t),
        }
    }
}

pub trait LinearColorEqNone {
    fn eq_none(&self) -> bool;
}

impl LinearColorEqNone for LinearColorStop {
    fn eq_none(&self) -> bool {
        self.color.h.eq(&0.) && self.color.s.eq(&0.) && self.color.l.eq(&0.) && self.color.a.eq(&0.)
    }
}

impl Interpolatable for ShadowBackground {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        let self_colors = self.get_effective_colors();
        let other_colors = other.get_effective_colors();

        Self {
            tag: ShadowBackgroundTag::LinearGradient,
            pad0: other.pad0.clone(),
            solid: self.solid.interpolate(&other.solid, t),
            gradient_angle_or_pattern_height: refine_interp!(
                self,
                other,
                gradient_angle_or_pattern_height,
                t
            ),
            colors: [
                self_colors[0].interpolate(&other_colors[0], t),
                self_colors[1].interpolate(&other_colors[1], t),
            ],
            pad1: other.pad1,
        }
    }
}

impl From<ShadowBackground> for Background {
    fn from(shadow: ShadowBackground) -> Self {
        unsafe { std::mem::transmute(shadow) }
    }
}

impl From<ShadowBackground> for Fill {
    fn from(shadow: ShadowBackground) -> Self {
        Fill::from(Background::from(shadow))
    }
}

impl Interpolatable for Fill {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        let Fill::Color(bg_start) = self;
        let Fill::Color(bg_end) = other;

        ShadowBackground::from(bg_start)
            .interpolate(ShadowBackground::from(bg_end), t)
            .into()
    }
}

impl Interpolatable for TextStyleRefinement {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            color: optional_refine_interp!(self, other, color, t),
            background_color: optional_refine_interp!(self, other, background_color, t),
            font_size: optional_refine_interp!(self, other, font_size, t),
            font_weight: optional_refine_interp!(self, other, font_weight, t),

            ..other.clone()
        }
    }
}

impl FastInterpolatable for TextStyleRefinement {
    #[inline]
    fn fast_interpolate(&self, other: &Self, t: f32, out: &mut Self) {
        fast_optional_refine_interp!(self, other, color, t, out);
        fast_optional_refine_interp!(self, other, background_color, t, out);
        // memory leak due to hashmap cache
        fast_optional_refine_interp!(self, other, font_size, t, out);
        fast_optional_refine_interp!(self, other, font_weight, t, out);
    }
}

impl Interpolatable for DefiniteLength {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        match (self, other) {
            (Self::Absolute(from), Self::Absolute(to)) => Self::Absolute(from.interpolate(to, t)),
            (Self::Fraction(from), Self::Fraction(to)) => Self::Fraction(from.interpolate(to, t)),
            _ => *other,
        }
    }
}

impl Interpolatable for Length {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        match (self, other) {
            (Self::Definite(from), Self::Definite(to)) => Self::Definite(from.interpolate(&to, t)),
            _ => *other,
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable for Size<T> {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            width: refine_interp!(self, other, width, t),
            height: refine_interp!(self, other, height, t),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable for SizeRefinement<T> {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            width: optional_refine_interp!(self, other, width, t),
            height: optional_refine_interp!(self, other, height, t),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable for Edges<T> {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            top: refine_interp!(self, other, top, t),
            right: refine_interp!(self, other, right, t),
            bottom: refine_interp!(self, other, bottom, t),
            left: refine_interp!(self, other, left, t),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable
    for EdgesRefinement<T>
{
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            top: optional_refine_interp!(self, other, top, t),
            right: optional_refine_interp!(self, other, right, t),
            bottom: optional_refine_interp!(self, other, bottom, t),
            left: optional_refine_interp!(self, other, left, t),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable for Corners<T> {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            top_left: refine_interp!(self, other, top_left, t),
            top_right: refine_interp!(self, other, top_right, t),
            bottom_right: refine_interp!(self, other, bottom_right, t),
            bottom_left: refine_interp!(self, other, bottom_left, t),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable
    for CornersRefinement<T>
{
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            top_left: optional_refine_interp!(self, other, top_left, t),
            top_right: optional_refine_interp!(self, other, top_right, t),
            bottom_right: optional_refine_interp!(self, other, bottom_right, t),
            bottom_left: optional_refine_interp!(self, other, bottom_left, t),
        }
    }
}

impl<T: Clone + Debug + Default + PartialEq + Interpolatable> Interpolatable for Point<T> {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            x: refine_interp!(self, other, x, t),
            y: refine_interp!(self, other, y, t),
        }
    }
}

impl Interpolatable for BoxShadow {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        Self {
            color: refine_interp!(self, other, color, t),
            offset: refine_interp!(self, other, offset, t),
            blur_radius: refine_interp!(self, other, blur_radius, t),
            spread_radius: refine_interp!(self, other, spread_radius, t),
        }
    }
}

impl<T: Interpolatable> Interpolatable for Vec<T> {
    #[inline]
    fn interpolate(&self, other: &Self, t: f32) -> Self {
        let max_len = self.len().max(other.len());
        let mut result = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let from = self.get(i);
            let to = other.get(i);

            match (from, to) {
                (Some(f), Some(_t)) => result.push(f.interpolate(_t, t)),
                (_, Some(t)) => result.push(t.clone()),
                _ => {}
            }
        }

        result
    }
}

impl FastInterpolatable for StyleRefinement {
    #[inline]
    fn fast_interpolate(&self, other: &Self, t: f32, out: &mut Self) {
        fast_optional_refine_interp!(self, other, scrollbar_width, t, out);
        fast_optional_refine_interp!(self, other, aspect_ratio, t, out);
        fast_refine_interp!(self, other, size, t, out);
        fast_refine_interp!(self, other, max_size, t, out);
        fast_refine_interp!(self, other, min_size, t, out);
        fast_refine_interp!(self, other, margin, t, out);
        fast_refine_interp!(self, other, padding, t, out);
        fast_refine_interp!(self, other, border_widths, t, out);
        fast_refine_interp!(self, other, gap, t, out);
        fast_optional_refine_interp!(self, other, flex_basis, t, out);
        fast_optional_refine_interp!(self, other, flex_grow, t, out);
        fast_optional_refine_interp!(self, other, flex_shrink, t, out);
        fast_optional_refine_interp!(self, other, background, t, out);
        fast_optional_refine_interp!(self, other, border_color, t, out);
        fast_refine_interp!(self, other, corner_radii, t, out);
        fast_optional_refine_interp!(self, other, box_shadow, t, out);
        fast_optional_refine_interp!(self, other, opacity, t, out);
        fast_refine_interp!(self, other, inset, t, out);

        match (&self.text, &other.text) {
            (Some(from), Some(to)) => {
                if from.ne(&to) {
                    from.fast_interpolate(to, t, out.text.as_mut().unwrap());
                }
            }
            (None, Some(to)) => {
                out.text = Some(to.clone());
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct State<T: FastInterpolatable + Default + PartialEq> {
    #[allow(dead_code)]
    pub(crate) origin: T,
    pub(crate) from: T,
    pub(crate) to: T,
    pub(crate) cur: T,
    pub(crate) progress: f32,
    pub(crate) start_at: Instant,
    pub(crate) version: usize,
    pub(crate) priority: AnimationPriority,
}

impl<T: FastInterpolatable + Default + PartialEq> PartialEq for State<T> {
    fn eq(&self, other: &Self) -> bool {
        self.to.eq(&other.to)
    }

    fn ne(&self, other: &Self) -> bool {
        self.to.ne(&other.to)
    }
}

impl<T: FastInterpolatable + Default + PartialEq> Default for State<T> {
    fn default() -> Self {
        Self {
            origin: T::default(),
            from: T::default(),
            to: T::default(),
            cur: T::default(),
            progress: 1.,
            start_at: Instant::now(),
            version: 0,
            priority: AnimationPriority::Lowest,
        }
    }
}

impl Styled for State<StyleRefinement> {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.to
    }
}

impl<T: FastInterpolatable + Default + PartialEq> State<T> {
    pub fn origin(mut self) -> Self {
        self.to = self.origin.clone();

        self
    }

    pub(crate) fn new(init: T) -> Self {
        Self {
            origin: init.clone(),
            cur: init.clone(),
            from: init.clone(),
            to: init,
            ..Default::default()
        }
    }

    pub(crate) fn pre_animated(&mut self, dt: Duration) -> (usize, Duration) {
        self.version = self.version.wrapping_add(1);

        let is_reversing = self.to == self.from;

        let actual_duration = if is_reversing {
            dt.mul_f32(self.progress)
        } else {
            dt
        };

        self.from = self.cur.clone();
        self.start_at = Instant::now();

        self.progress = 0.;

        (self.version, actual_duration)
    }

    pub(crate) fn animated(
        &mut self,
        ss_ver: usize,
        dt: Duration,
        transition: &Arc<dyn Transition>,
        persistent: bool,
    ) -> bool {
        if ss_ver != self.version {
            return true;
        }

        self.progress = transition.run(self.start_at, dt);

        if self.progress >= 1.0 {
            self.cur = self.to.clone();
            if persistent {
                return false;
            }

            return true;
        }

        self.from
            .fast_interpolate(&self.to, self.progress, &mut self.cur);

        false
    }
}
