"""M57/D3 -- the keyboard-only drive (A12's gate).

Every demo with a focusable node is reached by **Tab alone** and driven
by keys alone: no pointer after the sidebar has put the demo on the stage.
The widgets had keyboard arms that nothing could reach (A12: SplitPane,
Collapsible, Tree, a picker's pad could not take focus at all), and the
pointer drives could not see it -- they never pressed Tab.

Coverage is data, as the pointer inventories' is: the runner counts the
focusable nodes inside each demo, and a demo that has one and no entry
here fails the run.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Callable


@dataclass
class KeyAction:
    name: str
    #: What to press once the target has focus: key names, (key, [mods])
    #: pairs, or "text:..." for typed text (a field inserts on text, not on
    #: a key-down; the agent's `input.type`).
    keys: list
    #: Optional custom check; the default is "the readout or the target's
    #: published value changed".
    expect: Callable | None = None


#: Keys pressed after the demo is on stage and before Tabbing to it: a demo
#: that opens with its popup up traps Tab inside the popup, as an open
#: popup should, and Escape is how a keyboard user leaves it.
START: dict[str, list] = {"DatePicker": ["Escape"], "Menu": ["Escape"]}

# widget -> (the selector Tab must reach, [actions])
KEYS: dict[str, tuple[str, list[KeyAction]]] = {
    "Chip": ("#d_chip_a", [KeyAction("Enter selects the chip", ["Enter"])]),
    "RichTextEditor": ("#d_editor_a > #editor-input",
                       [KeyAction("typing edits the source", ["text:x"])]),
    "Button": ("#d_button_a", [KeyAction("Enter presses it", ["Enter"])]),
    "CheckBox": ("#d_checkbox_a > #labelled-control", [KeyAction("Space ticks it", ["Space"])]),
    "Switch": ("#d_switch_a > #labelled-control", [KeyAction("Space flips it", ["Space"])]),
    "Radio": ("#d_radio_a > #labelled-control", [KeyAction("Down moves the choice", ["Down"])]),
    "Slider": ("#d_slider_a", [KeyAction("Right steps the value", ["Right"])]),
    "TextInput": ("#d_textinput_a", [KeyAction("typing edits it", ["text:x"])]),
    # M81/T81.6: the right-to-left field; typing edits it as it does the first.
    "TextInput RTL": ("#d_textinput_rtl", [KeyAction("typing edits it", ["text:x"])]),
    # M80/T80.2: the Label demo's switch, which sets the paragraph's line height.
    "Label": ("#d_label_spacing > #labelled-control", [KeyAction("Space sets the line height", ["Space"])]),
    # M80/T80.4: Tab reaches the text itself. The demo opens with all of it
    # selected, so Escape clears it first and Ctrl+A then selects it all
    # again -- each a change the readout (selected characters) reports.
    "Selectable": ("#d_selectable_a", [KeyAction("Escape clears the selection", ["Escape"]),
                                        KeyAction("Ctrl+A selects all", [("a", ["ctrl"])])]),
    "ColorPicker": ("#d_colorpicker_a > #picker-trigger", [KeyAction("Space opens it", ["Space"], "open")]),
    "DatePicker": ("#d_datepicker_a > #picker-trigger", [KeyAction("Space opens it", ["Space"], "open")]),
    "TimePicker": ("#d_timepicker_a > #picker-trigger", [KeyAction("Space opens it", ["Space"], "open")]),
    "FilePicker": ("#d_filepicker_a > #picker-trigger", [KeyAction("Space opens it", ["Space"], "open")]),
    "PickList": ("#d_picklist_a > #chooser-trigger",
                 [KeyAction("Down, Down, Enter chooses", ["Down", "Down", "Enter"])]),
    "Select": ("#d_select_a > #select-trigger",
               [KeyAction("Enter, Down, Down, Enter chooses", ["Enter", "Down", "Down", "Enter"])]),
    "Combobox": ("#d_combobox_a > #chooser-trigger",
                 [KeyAction("Down, Down, Enter chooses", ["Down", "Down", "Enter"])]),
    "Menu": ("#d_menu_open", [KeyAction("Enter opens the menu", ["Enter"], "menu")]),
    "NumberInput": ("#d_number_a", [KeyAction("Up steps the number", ["Up"])]),
    "RangeSlider": ("#d_range_a", [KeyAction("Right moves a handle", ["Right"])]),
    "SearchField": ("#d_search_a > #search-field",
                    [KeyAction("Escape clears the query", ["Escape"])]),
    "Card": ("#d_card_go", [KeyAction("Enter presses the card's button", ["Enter"])]),
    "Toast": ("#d_toast_show", [KeyAction("Enter raises a toast", ["Enter"])]),
    "Tooltip": ("#tooltip-content", [KeyAction("focus alone shows the tip", [], "tip")]),
    "Tabs": ("#d_tabs_a", [KeyAction("Right moves the tab", ["Right"])]),
    "BottomNav": ("#d_bottomnav_a", [KeyAction("Right moves the item", ["Right"])]),
    "NavigationRail": ("#d_rail_a", [KeyAction("Down moves the item", ["Down"])]),
    "AppBar": ("appbar-first-button", [KeyAction("Enter presses Back", ["Enter"])]),
    "Modal": ("#d_modal_go", [KeyAction("Enter confirms", ["Enter"])]),
    "Sheet": ("#d_sheet_go", [KeyAction("Enter confirms", ["Enter"])]),
    "Drawer": ("#d_drawer_go", [KeyAction("Enter confirms", ["Enter"])]),
    "Wrap": ("#d_wrap_a", [KeyAction("Enter selects a chip", ["Enter"], "focus-only")]),
    "SplitPane": ("#d_split_a", [KeyAction("Right moves the divider", ["Right"])]),
    "Accordion": ("#d_acc_b", [KeyAction("Space opens the section", ["Space"])]),
    "PaneGrid": ("#d_panegrid_a", [KeyAction("Right moves the vertical divider", ["Right"])]),
    "Pagination": ("#d_pagination_a", [KeyAction("Right goes to the next page", ["Right"])]),
    "Tree": ("#d_tree_a", [KeyAction("Down, Left folds a branch", ["Down", "Left"])]),
    # D3 recorded the grid's rows unreachable by keyboard; the sheet takes
    # focus itself and the arrows move its active cell.
    "DataGrid": ("#d_datagrid_a", [KeyAction("Down, Right moves the active cell", ["Down", "Right"])]),
    # Both came from main in the M56 merge (M44/T44.10's demos), after
    # D3's table was written.
    "TextArea": ("#d_textarea_a", [KeyAction("typing edits it", ["text:x"])]),
    "ScrollView": ("#d_scroll_first", [KeyAction("PageDown scrolls the view", ["PageDown"])]),
    # M84/T84.3: the Image demo's pause button; the readout says playing or paused.
    "Image": ("#d_image_pause", [KeyAction("Enter pauses or resumes it", ["Enter"])]),
}
