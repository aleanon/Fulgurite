# R11-F2 (M52/T52.2): "f64 in the frame path: zero" was prose. The
# review's `12e6d3d`, an f64 round-trip in Constraints::clamp, must fire
# the soft-double count by name.
CHECK='./scripts/check_text.sh'
EXPECT='firmware: soft-f64 symbols [1-9][0-9]* \(bound 0\)'
plant() {
    replace crates/lumen-static/src/desc.rs \
        'w: crate::num::clamp(s.w, self.min_w, self.max_w),' \
        'w: crate::num::clamp(((s.w as f64) / 3.0f64 * 3.0f64) as f32, self.min_w, self.max_w),'
}
