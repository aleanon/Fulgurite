# R18-F2, the exported half (M42/T42.0): nothing tied the widgets
# lumen-static-widgets exports to an inventory. Once fixed, every
# exported widget type has an inventory or an entry in
# scripts/drive/undriven.allow, and removing an entry is named.
CHECK='./scripts/check_drive_coverage.sh'
EXPECT='Glass.*(no inventory|undriven)'
plant() {
    # Select was the victim until M44/T44.10 gave it an inventory; Glass
    # is exported and owed only by its allow line, so dropping it must be named.
    drop_lines scripts/drive/undriven.allow 'Glass |'
}
