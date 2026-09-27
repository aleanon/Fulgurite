# R18-F3 (M42/T42.0): check (1), "it landed", passed on ANY non-empty
# readout, and `changed` was the whole frame's set. With the hover shade
# removed from the showcase's sheet, a hover changes nothing on the
# button -- the drive stayed green on the readout alone. Once fixed, the
# check is filtered to the widget's subtree and compares the readout
# before and after.
CHECK='./scripts/widget_drive.sh Button'
# hover_dim(0.0) also removes the press shade, and since M48/T48.4 the
# drive checks that a click paints damage over the widget. That action
# runs before the hover action, so the same planted fault is now named
# one action earlier. Either name is this plant's mechanism.
EXPECT='Button/(hovering is visible.*changed in subtree 0|a click reaches the application: FAIL no damage)'
plant() {
    sub examples/showcase/src/main.rs \
        '(Selector::Id\("d_button_a"\),\n\s*btn\(\)\.background_role\(Role::Accent\)\.color_role\(Role::OnAccent\))' \
        '\1.hover_dim(0.0)'
}
