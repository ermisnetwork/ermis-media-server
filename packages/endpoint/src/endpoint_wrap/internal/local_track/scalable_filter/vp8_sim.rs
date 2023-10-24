//! VP8 simulcast filter
//! - Rewrite VP8 PictureId and TTL0IDX.
//!
//! For PictureId: it will rewrite the PictureId to avoid PictureId conflict.
//! For TL0IDX:
//!
//! Ref: https://github.com/versatica/mediasoup/issues/40

use transport::PayloadCodec;
use utils::SeqRewrite;

use super::{FilterResult, ScalableFilter};

const PIC_ID_MAX: u64 = 1 << 15;
const TL0IDX_MAX: u64 = 1 << 8;

struct Selection {
    spatial: u8,
    temporal: u8,
    key_only: bool,
}

impl Selection {
    pub fn new(spatial: u8, temporal: u8, key_only: bool) -> Self {
        Self { spatial, temporal, key_only }
    }

    pub fn allow(&self, pkt: &mut transport::MediaPacket, pic_id_rewrite: &mut SeqRewrite<PIC_ID_MAX, 60>, tl0idx_rewrite: &mut SeqRewrite<TL0IDX_MAX, 60>) -> FilterResult {
        match &mut pkt.codec {
            PayloadCodec::Vp8(is_key, Some(sim)) => {
                if sim.spatial == self.spatial {
                    if sim.temporal <= self.temporal && (*is_key || !self.key_only) {
                        if let Some(tl0idx) = sim.tl0_pic_idx {
                            if let Some(new_tl0idx) = tl0idx_rewrite.generate(tl0idx as u64) {
                                sim.tl0_pic_idx = Some(new_tl0idx as u8);
                            } else {
                                return FilterResult::Drop;
                            }
                        }
                        if let Some(pic_id) = sim.picture_id {
                            if let Some(new_pic_id) = pic_id_rewrite.generate(pic_id as u64) {
                                sim.picture_id = Some(new_pic_id as u16);
                            } else {
                                return FilterResult::Drop;
                            }
                        }

                        FilterResult::Send
                    } else {
                        if let Some(pic_id) = sim.picture_id {
                            pic_id_rewrite.drop_value(pic_id as u64);
                        }
                        if let Some(tl0idx) = sim.tl0_pic_idx {
                            tl0idx_rewrite.drop_value(tl0idx as u64);
                        }
                        FilterResult::Drop
                    }
                } else {
                    FilterResult::Reject
                }
            }
            _ => FilterResult::Reject,
        }
    }

    pub fn should_switch(&self, current: &Option<Self>, pkt: &transport::MediaPacket) -> bool {
        match (current, &pkt.codec) {
            (None, PayloadCodec::Vp8(is_key, Some(sim))) => sim.spatial == self.spatial && sim.temporal <= self.temporal && *is_key,
            (Some(current), PayloadCodec::Vp8(is_key, Some(sim))) => {
                if current.spatial == self.spatial {
                    if self.temporal > current.temporal {
                        //Up sample
                        sim.temporal == self.temporal && sim.layer_sync
                    } else {
                        //Down sample => should apply now
                        true
                    }
                } else {
                    sim.spatial == self.spatial && sim.temporal <= self.temporal && *is_key
                }
            }
            _ => false,
        }
    }
}

#[derive(Default)]
pub struct Vp8SimulcastFilter {
    current: Option<Selection>,
    target: Option<Selection>,
    pic_id_rewrite: SeqRewrite<PIC_ID_MAX, 60>,
    tl0idx_rewrite: SeqRewrite<TL0IDX_MAX, 60>,
}

impl ScalableFilter for Vp8SimulcastFilter {
    fn pause(&mut self) {
        self.current = None;
        self.target = None;
        self.pic_id_rewrite.reinit();
        self.tl0idx_rewrite.reinit();
    }

    fn resume(&mut self) {}

    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool {
        let (key_frame, changed) = match &self.current {
            Some(current) => (current.spatial != spatial, current.spatial != spatial || current.temporal != temporal),
            None => (true, true),
        };
        if changed {
            self.target = Some(Selection::new(spatial, temporal, key_only));
        }
        key_frame
    }

    fn should_send(&mut self, pkt: &mut transport::MediaPacket) -> FilterResult {
        if let Some(target) = &self.target {
            if target.should_switch(&self.current, pkt) {
                self.pic_id_rewrite.reinit();
                self.tl0idx_rewrite.reinit();
                self.current = self.target.take();
            }
        }

        if let Some(current) = &self.current {
            current.allow(pkt, &mut self.pic_id_rewrite, &mut self.tl0idx_rewrite)
        } else {
            FilterResult::Reject
        }
    }
}

#[cfg(test)]
mod test {
    use transport::{MediaPacket, PayloadCodec, Vp8Simulcast};

    use crate::endpoint_wrap::internal::local_track::scalable_filter::{FilterResult, ScalableFilter};

    enum Input {
        // input (spatial, temporal, key_only) => need out request key
        SetTarget(u8, u8, bool, bool),
        // input (is_key, spatial, temporal, layer_sync, seq, time) => should send
        Packet(bool, u8, u8, bool, u16, u32, FilterResult),
    }

    fn test(data: Vec<Input>) {
        let mut filter = super::Vp8SimulcastFilter::default();

        for row in data {
            match row {
                Input::SetTarget(spatial, temporal, key_only, need_key) => {
                    assert_eq!(filter.set_target_layer(spatial, temporal, key_only), need_key);
                }
                Input::Packet(is_key, spatial, temporal, layer_sync, seq, time, send_expected) => {
                    let mut pkt = MediaPacket::simple_video(PayloadCodec::Vp8(is_key, Some(Vp8Simulcast::new(spatial, temporal, layer_sync))), seq, time, vec![1, 2, 3]);
                    assert_eq!(filter.should_send(&mut pkt), send_expected);
                }
            }
        }
    }

    #[test]
    fn simple() {
        test(vec![
            Input::SetTarget(0, 1, false, true),
            Input::Packet(false, 0, 0, false, 0, 100, FilterResult::Reject),
            Input::Packet(true, 0, 0, true, 1, 200, FilterResult::Send),
            Input::Packet(true, 0, 2, true, 2, 200, FilterResult::Drop),
        ])
    }
}
