"""M54/T54.3 — the role column: what `ui.getTree` must report for each
driven widget's node (the inventory entry's own node, or its fourth
element when the selector names a part).

A drive that clicks a checkbox and reads the flag back proves the flag;
it says nothing about what a screen reader is told the control is. The
runner asserts the node's role against this table before the first
action, prints both, and `run.py --manifest-only` refuses an inventory
entry with no row here, so a new widget cannot be driven without saying
what it is. `GenericContainer` is a claim like any other: `PullToRefresh`
is a gesture wrapper around content that has its own role.
"""

ROLES = {
    # A tag that reads as its text; pressable since M57/B2.
    "Chip": "Label",
    "PickList": "ComboBox",
    "Combobox": "ComboBox",
    # A menu, not a combo box, since M57/B23: it runs a command and has
    # no value.
    "Menu": "Menu",
    "Pagination": "ListBox",
    "Tree": "Tree",
    "DataGrid": "Grid",
    "NumberInput": "SpinButton",
    "RangeSlider": "Slider",
    "SearchField": "TextInput",
    "Wrap": "Label",
    "Grid": "Label",
    "SplitPane": "Splitter",
    "Accordion": "DisclosureTriangle",
    "PaneGrid": "Group",
    "Tabs": "TabList",
    "BottomNav": "TabList",
    "NavigationRail": "TabList",
    "AppBar": "Banner",
    "PullToRefresh": "GenericContainer",
    "ColorPicker": "ComboBox",
    "DatePicker": "ComboBox",
    "TimePicker": "ComboBox",
    "FilePicker": "ComboBox",
    "RichText": "Paragraph",
    "Markdown": "Paragraph",
    "RichTextEditor": "TextInput",
    "PaintBox": "Canvas",
    "Select": "ComboBox",
    "ScrollView": "ScrollView",
    "Card": "Group",
    "Toast": "Status",
    "Tooltip": "Group",
    "TextInput": "TextInput",
    "TextArea": "MultilineTextInput",
    # A label takes no input; its demo's switch restyles it (M80/T80.2).
    "Label": "Label",
    "Slider": "Slider",
    "Button": "Button",
    "CheckBox": "CheckBox",
    "Switch": "Switch",
    "Radio": "RadioButton",
    "Modal": "Dialog",
    "Sheet": "Dialog",
    "Drawer": "Dialog",
}
