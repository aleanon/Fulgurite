# R11-F1 (M52/T52.2): one `f32::clamp` once linked 26.5 KB of float
# formatting, and nothing counted the symbols -- only the ceiling, which a
# paint-only firmware stays under. The review's `2189c21`: the count must
# fire by name in the cascade-only binary, whose .text stays under the
# ceiling (80 KB since D5).
CHECK='./scripts/check_text.sh'
EXPECT='firmware_cascade: float-fmt symbols [1-9][0-9]* \(bound 0\)'
plant() {
    replace crates/lumen-static/src/desc.rs \
        'w: crate::num::clamp(s.w, self.min_w, self.max_w),' \
        'w: s.w.clamp(self.min_w, self.max_w),'
}
