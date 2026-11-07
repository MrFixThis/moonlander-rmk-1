use embassy_time::Instant;

use rmk_palettefx::{
    color::{self, Hsv, Rgb},
    effects::{FlowState, FrameParams, Lcg, ReactiveState, RippleState, SparkleState, VortexState},
};

const REACTIVE_HITS: usize = 8;

pub(super) enum Effect {
    Gradient,
    Plain(PlainState),
    Flow(FlowState),
    Vortex(VortexState),
    Sparkle(SparkleState),
    Ripple(RippleState, Lcg),
    Reactive(ReactiveState<REACTIVE_HITS>),
}

impl Effect {
    const RIPPLE_BASE_SEED: u64 = 0xDEAD_C0DE_CAFE_F00D;

    #[inline]
    pub(super) const fn plain() -> Self {
        Self::Plain(PlainState {
            last_sat: u8::MAX,
            last_val: u8::MAX,
            dirty: true,
        })
    }

    #[inline]
    pub(super) const fn flow() -> Self {
        Self::Flow(FlowState::new())
    }

    #[inline]
    pub(super) const fn vortex() -> Self {
        Self::Vortex(VortexState::new())
    }

    #[inline]
    pub(super) const fn sparkle() -> Self {
        Self::Sparkle(SparkleState::new())
    }

    #[inline]
    pub(super) fn ripple() -> Self {
        Self::Ripple(
            RippleState::new(),
            Lcg::new((Self::RIPPLE_BASE_SEED ^ Instant::now().as_millis()) as _),
        )
    }

    #[inline]
    pub(super) const fn reactive() -> Self {
        Self::Reactive(ReactiveState::new())
    }
}

pub(super) struct PlainState {
    last_sat: u8,
    last_val: u8,
    dirty: bool,
}

impl PlainState {
    #[inline(always)]
    pub(super) fn dirty(&mut self, val: bool) {
        self.dirty = val;
    }

    pub(super) fn tick(&mut self, params: FrameParams<'_>) -> Option<Rgb> {
        // For first flush
        if self.dirty {
            self.dirty = false;
            return Some(Self::determine_rgb(params));
        }

        if self.last_sat != params.sat || self.last_val != params.val {
            self.last_sat = params.sat;
            self.last_val = params.val;
            self.dirty = true;
            return Some(Self::determine_rgb(params));
        }
        None
    }

    #[inline(always)]
    fn determine_rgb(params: FrameParams<'_>) -> Rgb {
        color::hsv_to_rgb(Hsv {
            h: params.sat,
            s: u8::MAX,
            v: params.val,
        })
    }
}
