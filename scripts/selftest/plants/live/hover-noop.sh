# R18-F3 (M42/T42.0): check (1), "it landed", passed on ANY non-empty
# readout, and `changed` was the whole frame's set. With the hover shade
# removed from the showcase's sheet, a hover changes nothing on the
# button -- the drive stayed green on the readout alone. Once fixed, the
# check is filtered to the widget's subtree and compares the readout
# before and after.
CHECK='./scripts/widget_drive.sh Button'
EXPECT='Button/hovering is visible.*changed in subtree 0'
plant() {
    sub examples/showcase/src/main.rs \
        '(Selector::Id\("d_button_a"\),\n\s*btn\(\)\.background_role\(Role::Accent\)\.color_role\(Role::OnAccent\))' \
        '\1.hover_dim(0.0)'
}
