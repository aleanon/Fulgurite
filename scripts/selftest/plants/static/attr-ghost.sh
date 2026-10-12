# R17-F4: the attribute-table check compared two hand-written tables and
# never read the derives' attributes(...) lists, so an attribute the parser
# accepts and neither table documents passed.
CHECK='./scripts/check_attr_table.sh'
EXPECT='planted_ghost.*declared, not documented|declared, not documented.*planted_ghost'
plant() {
    replace crates/fulgurite-macros/src/lib.rs \
        'attributes(reload))]' 'attributes(reload, planted_ghost))]'
}
